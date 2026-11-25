//! The session's screen on Tapa's VT core: what it shows as a `Snapshot`, the selection, scrolling and input modes.

use std::sync::Arc;
use std::time::Instant;

use term_vt::Terminal;
use term_vt::cell::{CellFlags as VtFlags, Link};
use term_vt::color::Color;
use term_vt::selection::{Anchor, Kind, Selection, Side};
use term_vt::style::Attrs;
use term_vt::term::{CursorShape as VtShape, Modes, colors};

use crate::input::{InputModes, KittyFlags};
use crate::session::{CellSide, Scroll, SelectionKind};
use crate::snapshot::{
    Cell, CellFlags, Cursor, CursorShape, Damage, Highlight, LineDamage, Palette, Point, Rgb, Snapshot,
};

/// The VT core with what the session adds around it.
#[derive(Debug)]
pub(crate) struct Screen {
    pub(crate) terminal: Terminal,
    pub(crate) selection: Option<Selection>,
    /// Programs may switch on the kitty keyboard protocol.
    kitty_keyboard: bool,
    /// When the program began a synchronized update (mode 2026) that has not ended.
    pub(crate) sync_since: Option<Instant>,
    /// The rows of the last snapshot, which the next one's damage is measured against.
    shown: Vec<Vec<Cell>>,
}

impl Screen {
    pub(crate) fn new(terminal: Terminal, kitty_keyboard: bool) -> Screen {
        Screen {
            terminal,
            selection: None,
            kitty_keyboard,
            sync_since: None,
            shown: Vec::new(),
        }
    }

    fn colour(&self, palette: &Palette, index: usize) -> Rgb {
        if let Some(Some((r, g, b))) = self.terminal.color_changes().get(index) {
            return Rgb::new(*r, *g, *b);
        }
        match index {
            0..=255 => palette.indexed(u8::try_from(index).unwrap_or(0)),
            colors::FOREGROUND => palette.foreground,
            colors::BACKGROUND => palette.background,
            _ => palette.cursor.unwrap_or(palette.foreground),
        }
    }

    fn foreground(&self, palette: &Palette, color: Color, attrs: Attrs) -> Rgb {
        let bold = attrs.contains(Attrs::BOLD) && palette.bold_is_bright;
        let dim = attrs.contains(Attrs::DIM);
        let ansi = |n: usize| match n {
            0..=7 if dim => self.colour(palette, n).dim(),
            8..=15 if dim => self.colour(palette, n - 8),
            0..=7 if bold => self.colour(palette, n + 8),
            _ => self.colour(palette, n),
        };
        match color {
            Color::Rgb(r, g, b) => {
                let rgb = Rgb::new(r, g, b);
                if dim { rgb.dim() } else { rgb }
            }
            Color::Named(n @ 0..=15) => ansi(usize::from(n)),
            Color::Indexed(n @ 0..=15) => ansi(usize::from(n)),
            Color::Indexed(n) => self.colour(palette, usize::from(n)),
            Color::Named(257) => self.colour(palette, colors::BACKGROUND),
            Color::Named(_) => {
                let rgb = self.colour(palette, colors::FOREGROUND);
                if dim { rgb.dim() } else { rgb }
            }
        }
    }

    fn background(&self, palette: &Palette, color: Color) -> Rgb {
        match color {
            Color::Rgb(r, g, b) => Rgb::new(r, g, b),
            Color::Named(257) => self.colour(palette, colors::BACKGROUND),
            Color::Named(256) => self.colour(palette, colors::FOREGROUND),
            Color::Named(n) => self.colour(palette, usize::from(n).min(255)),
            Color::Indexed(n) => self.colour(palette, usize::from(n)),
        }
    }

    /// The visible screen at this moment, with what changed since the last one.
    pub(crate) fn snapshot(&mut self, palette: &Palette) -> Snapshot {
        let rows = self.visible_rows(palette);
        let damage = damage(&self.shown, &rows);
        self.shown.clone_from(&rows);
        let terminal = &self.terminal;
        let grid = terminal.grid();
        let size = terminal.size();
        let cursor = terminal.cursor();
        let offset = grid.display_offset;
        let line = cursor.line + offset;
        let (shape, blinking) = match terminal.cursor_shape() {
            Some((VtShape::Block, blink)) => (CursorShape::Block, blink),
            Some((VtShape::Underline, blink)) => (CursorShape::Underline, blink),
            Some((VtShape::Bar, blink)) => (CursorShape::Beam, blink),
            None => (CursorShape::Block, false),
        };
        let wide = grid
            .rows
            .get(cursor.line)
            .and_then(|row| row.cells.get(cursor.column))
            .is_some_and(|cell| cell.flags.contains(VtFlags::WIDE));
        Snapshot {
            columns: size.columns,
            lines: size.lines,
            rows,
            cursor: Cursor {
                point: Point {
                    line: if line < size.lines { line } else { 0 },
                    column: cursor.column.min(size.columns - 1),
                },
                shape,
                visible: terminal.modes().contains(Modes::SHOW_CURSOR) && line < size.lines,
                blinking,
                wide,
            },
            selection: self.highlight(),
            display_offset: offset,
            history: grid.history.len(),
            damage,
            background: self.colour(palette, colors::BACKGROUND),
            modes: self.input_modes(),
        }
    }

    fn visible_rows(&self, palette: &Palette) -> Vec<Vec<Cell>> {
        let terminal = &self.terminal;
        let grid = terminal.grid();
        let size = terminal.size();
        let mut links = Links::default();
        (0..size.lines)
            .map(|line| {
                let Some(row) = grid.view_row(line) else {
                    let blank = Cell {
                        ch: ' ',
                        zero_width: Vec::new(),
                        fg: palette.foreground,
                        bg: palette.background,
                        underline_color: None,
                        flags: CellFlags::default(),
                        hyperlink: None,
                    };
                    return vec![blank; size.columns];
                };
                row.cells
                    .iter()
                    .enumerate()
                    .map(|(column, cell)| {
                        let style = terminal.style(cell.style);
                        let mut fg = self.foreground(palette, style.fg, style.attrs);
                        let mut bg = self.background(palette, style.bg);
                        if style.attrs.contains(Attrs::INVERSE) {
                            std::mem::swap(&mut fg, &mut bg);
                        }
                        let extra = cell.flags.contains(VtFlags::EXTRA).then(|| row.extra(column)).flatten();
                        Cell {
                            ch: cell.c,
                            zero_width: extra.map(|e| e.zero_width.chars().collect()).unwrap_or_default(),
                            fg,
                            bg,
                            underline_color: style.underline.map(|c| self.background(palette, c)),
                            flags: flags(cell.flags, style.attrs),
                            hyperlink: extra.and_then(|e| e.link.as_ref()).map(|link| links.target(link)),
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn highlight(&self) -> Option<Highlight> {
        let selection = self.selection.as_ref()?;
        let grid = self.terminal.grid();
        let (first, last) = selection.range(grid)?;
        let top = grid.global_of_view(0);
        let bottom = top + grid.lines() as u64 - 1;
        if last.line < top || first.line > bottom {
            return None;
        }
        let point = |anchor: Anchor| {
            if anchor.line < top {
                Point { line: 0, column: 0 }
            } else if anchor.line > bottom {
                Point {
                    line: grid.lines() - 1,
                    column: grid.columns() - 1,
                }
            } else {
                Point {
                    line: usize::try_from(anchor.line - top).unwrap_or(0),
                    column: anchor.column,
                }
            }
        };
        Some(Highlight {
            start: point(first),
            end: point(last),
            block: selection.kind == Kind::Block,
        })
    }

    pub(crate) fn input_modes(&self) -> InputModes {
        let kitty = if self.kitty_keyboard {
            KittyFlags::from_bits(self.terminal.keyboard_protocol())
        } else {
            KittyFlags::NONE
        };
        InputModes::from_modes(self.terminal.modes(), kitty)
    }

    /// The visible lines as plain text without trailing blanks, for VoiceOver.
    pub(crate) fn visible_text(&self) -> Vec<String> {
        let grid = self.terminal.grid();
        (0..grid.lines())
            .map(|line| {
                let mut text = grid.view_row(line).map(|row| row.text()).unwrap_or_default();
                text.truncate(text.trim_end().len());
                text
            })
            .collect()
    }

    pub(crate) fn scroll(&mut self, scroll: Scroll) {
        let lines = isize::try_from(self.terminal.size().lines).unwrap_or(1);
        let grid = self.terminal.grid_mut();
        match scroll {
            Scroll::Lines(delta) => grid.scroll_view(isize::try_from(delta).unwrap_or(0)),
            Scroll::PageUp => grid.scroll_view(lines),
            Scroll::PageDown => grid.scroll_view(-lines),
            Scroll::Top => grid.scroll_view(isize::MAX / 2),
            Scroll::Bottom => grid.display_offset = 0,
        }
    }

    fn anchor(&self, point: Point) -> Anchor {
        let grid = self.terminal.grid();
        Anchor {
            line: grid.global_of_view(point.line.min(grid.lines() - 1)),
            column: point.column.min(grid.columns() - 1),
        }
    }

    pub(crate) fn select(&mut self, kind: SelectionKind, point: Point, side: CellSide) {
        let kind = match kind {
            SelectionKind::Simple => Kind::Simple,
            SelectionKind::Block => Kind::Block,
            SelectionKind::Word => Kind::Word,
            SelectionKind::Line => Kind::Line,
        };
        let at = (self.anchor(point), side_of(side));
        self.selection = Some(Selection {
            kind,
            start: at,
            end: at,
        });
    }

    pub(crate) fn extend_selection(&mut self, point: Point, side: CellSide) {
        let at = (self.anchor(point), side_of(side));
        if let Some(selection) = self.selection.as_mut() {
            selection.end = at;
        }
    }

    pub(crate) fn selected_text(&self) -> Option<String> {
        self.selection.as_ref()?.text(self.terminal.grid())
    }
}

/// The link a snapshot met last with its target, so the cells of one link share one copy of its address.
#[derive(Default)]
struct Links(Option<(Arc<Link>, Arc<str>)>);

impl Links {
    fn target(&mut self, link: &Arc<Link>) -> Arc<str> {
        match &self.0 {
            Some((seen, target)) if Arc::ptr_eq(seen, link) => target.clone(),
            _ => {
                let target: Arc<str> = link.uri.as_str().into();
                self.0 = Some((Arc::clone(link), target.clone()));
                target
            }
        }
    }
}

/// The columns of each line that differ between `before` and `now`; everything when their shapes differ.
fn damage(before: &[Vec<Cell>], now: &[Vec<Cell>]) -> Damage {
    if before.len() != now.len() || before.iter().zip(now).any(|(a, b)| a.len() != b.len()) {
        return Damage::Full;
    }
    let lines = before
        .iter()
        .zip(now)
        .enumerate()
        .filter_map(|(line, (a, b))| {
            let left = a.iter().zip(b).position(|(x, y)| x != y)?;
            let right = a.iter().zip(b).rposition(|(x, y)| x != y)?;
            Some(LineDamage { line, left, right })
        })
        .collect();
    Damage::Lines(lines)
}

fn side_of(side: CellSide) -> Side {
    match side {
        CellSide::Left => Side::Left,
        CellSide::Right => Side::Right,
    }
}

fn flags(cell: VtFlags, attrs: Attrs) -> CellFlags {
    let pairs = [
        (attrs.contains(Attrs::BOLD), CellFlags::BOLD),
        (attrs.contains(Attrs::ITALIC), CellFlags::ITALIC),
        (attrs.contains(Attrs::DIM), CellFlags::DIM),
        (attrs.contains(Attrs::UNDERLINE), CellFlags::UNDERLINE),
        (attrs.contains(Attrs::DOUBLE_UNDERLINE), CellFlags::DOUBLE_UNDERLINE),
        (attrs.contains(Attrs::UNDERCURL), CellFlags::UNDERCURL),
        (attrs.contains(Attrs::DOTTED_UNDERLINE), CellFlags::DOTTED_UNDERLINE),
        (attrs.contains(Attrs::DASHED_UNDERLINE), CellFlags::DASHED_UNDERLINE),
        (attrs.contains(Attrs::STRIKE), CellFlags::STRIKETHROUGH),
        (attrs.contains(Attrs::INVERSE), CellFlags::INVERSE),
        (attrs.contains(Attrs::HIDDEN), CellFlags::HIDDEN),
        (cell.contains(VtFlags::WIDE), CellFlags::WIDE),
        (
            cell.intersects(VtFlags(VtFlags::WIDE_SPACER.0 | VtFlags::LEADING_WIDE_SPACER.0)),
            CellFlags::WIDE_SPACER,
        ),
        (cell.contains(VtFlags::WRAPPED), CellFlags::WRAPPED),
    ];
    pairs
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(CellFlags::default(), |bits, (_, flag)| bits.with(flag))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(columns: usize, lines: usize) -> Screen {
        Screen::new(Terminal::new(term_vt::Size { columns, lines }, 100), true)
    }

    #[test]
    fn the_cells_of_one_link_share_its_target() {
        let mut screen = screen(12, 2);
        let output = b"\x1b]8;;https://a.example/\x07link\x1b]8;;\x07 \x1b]8;;https://a.example/\x07x\x1b]8;;\x07";
        screen.terminal.advance(output);
        let snapshot = screen.snapshot(&Palette::default());
        let row = &snapshot.rows[0];
        let first = row[0].hyperlink.clone().expect("a link");
        assert_eq!(&*first, "https://a.example/");
        assert!(
            row[1..4]
                .iter()
                .all(|cell| cell.hyperlink.as_ref().is_some_and(|l| Arc::ptr_eq(l, &first)))
        );
        assert_eq!(row[4].hyperlink, None);
        assert_eq!(
            row[5].hyperlink.as_deref(),
            Some("https://a.example/"),
            "a link of its own"
        );
    }

    #[test]
    fn colours_resolve_through_palette_program_changes_bold_and_faint() {
        let palette = Palette::default();
        let mut screen = screen(20, 2);
        screen.terminal.advance(b"\x1b]4;2;rgb:01/02/03\x07");
        let fg = |color, attrs| screen.foreground(&palette, color, attrs);
        let plain = Attrs::default();
        assert_eq!(fg(Color::Named(1), plain), palette.ansi[1]);
        assert_eq!(fg(Color::Named(1), Attrs::BOLD), palette.ansi[9]);
        assert_eq!(fg(Color::Named(1), Attrs::DIM), palette.ansi[1].dim());
        assert_eq!(fg(Color::Indexed(1), Attrs::BOLD), palette.ansi[9]);
        assert_eq!(fg(Color::Indexed(9), Attrs::DIM), palette.ansi[1]);
        assert_eq!(fg(Color::Indexed(2), plain), Rgb::new(1, 2, 3));
        assert_eq!(fg(Color::Rgb(90, 60, 30), Attrs::DIM), Rgb::new(60, 40, 20));
        assert_eq!(fg(Color::FOREGROUND, plain), palette.foreground);
        assert_eq!(screen.background(&palette, Color::Named(257)), palette.background);
        let plain_bold = Palette {
            bold_is_bright: false,
            ..Palette::default()
        };
        assert_eq!(
            screen.foreground(&plain_bold, Color::Named(1), Attrs::BOLD),
            palette.ansi[1]
        );
    }

    #[test]
    fn damage_names_the_changed_columns_of_each_line() {
        let mut screen = screen(10, 3);
        let palette = Palette::default();
        assert_eq!(screen.snapshot(&palette).damage, Damage::Full);
        screen.terminal.advance(b"\x1b[2;3Hab");
        let Damage::Lines(lines) = screen.snapshot(&palette).damage else {
            panic!("lines")
        };
        assert_eq!(
            lines,
            [LineDamage {
                line: 1,
                left: 2,
                right: 3
            }]
        );
        assert_eq!(screen.snapshot(&palette).damage, Damage::Lines(Vec::new()));
    }

    #[test]
    fn a_selection_reads_its_text_and_shows_where_it_is_visible() {
        let mut screen = screen(10, 2);
        screen.terminal.advance(b"hello world");
        screen.select(SelectionKind::Word, Point { line: 0, column: 1 }, CellSide::Left);
        assert_eq!(screen.selected_text().as_deref(), Some("hello"));
        let shown = screen.snapshot(&Palette::default()).selection.expect("visible");
        assert_eq!((shown.start, shown.end.column), (Point { line: 0, column: 0 }, 4));
    }

    #[test]
    fn kitty_flags_reach_the_input_only_when_allowed() {
        let mut allowed = screen(10, 2);
        allowed.terminal.advance(b"\x1b[>1u");
        assert!(allowed.input_modes().kitty.contains(KittyFlags::DISAMBIGUATE));
        let mut refused = Screen::new(Terminal::new(term_vt::Size { columns: 10, lines: 2 }, 0), false);
        refused.terminal.advance(b"\x1b[>1u");
        assert_eq!(refused.input_modes().kitty, KittyFlags::NONE);
    }
}
