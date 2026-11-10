//! What the view draws: the visible cells with their final colours, the cursor, the selection and what changed.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell as TermCell, Flags};
use alacritty_terminal::term::color::{COUNT, Colors};
use alacritty_terminal::term::{RenderableCursor, Term, TermDamage};
use alacritty_terminal::vte::ansi::{Color, CursorShape as TermCursorShape, NamedColor};

use crate::input::InputModes;

/// A colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[expect(missing_docs, reason = "the channels")]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// The colour from its channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    /// Two thirds as bright, as faint text is drawn.
    #[must_use]
    pub fn dim(self) -> Rgb {
        let third = |c: u8| u8::try_from(u16::from(c) * 2 / 3).unwrap_or(c);
        Rgb::new(third(self.r), third(self.g), third(self.b))
    }
}

/// The colours a session starts from; programs may change them (OSC 4, 10, 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// The default text colour.
    pub foreground: Rgb,
    /// The default background.
    pub background: Rgb,
    /// The cursor; `None` draws it in the text colour.
    pub cursor: Option<Rgb>,
    /// The 16 ANSI colours: black, red, green, yellow, blue, magenta, cyan, white, then their bright forms.
    pub ansi: [Rgb; 16],
    /// Bold text in one of the first eight colours uses the bright form, as most terminals do.
    pub bold_is_bright: bool,
}

impl Default for Palette {
    fn default() -> Palette {
        Palette {
            foreground: Rgb::new(0xd8, 0xd8, 0xd8),
            background: Rgb::new(0x18, 0x18, 0x18),
            cursor: None,
            ansi: [
                Rgb::new(0x00, 0x00, 0x00),
                Rgb::new(0xcd, 0x31, 0x31),
                Rgb::new(0x0d, 0xbc, 0x79),
                Rgb::new(0xe5, 0xe5, 0x10),
                Rgb::new(0x24, 0x72, 0xc8),
                Rgb::new(0xbc, 0x3f, 0xbc),
                Rgb::new(0x11, 0xa8, 0xcd),
                Rgb::new(0xe5, 0xe5, 0xe5),
                Rgb::new(0x66, 0x66, 0x66),
                Rgb::new(0xf1, 0x4c, 0x4c),
                Rgb::new(0x23, 0xd1, 0x8b),
                Rgb::new(0xf5, 0xf5, 0x43),
                Rgb::new(0x3b, 0x8e, 0xea),
                Rgb::new(0xd6, 0x70, 0xd6),
                Rgb::new(0x29, 0xb8, 0xdb),
                Rgb::new(0xff, 0xff, 0xff),
            ],
            bold_is_bright: true,
        }
    }
}

impl Palette {
    /// Colour `index` of the 256-colour table: the ANSI colours, a 6×6×6 cube, then 24 greys.
    pub fn indexed(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi[usize::from(index)],
            16..=231 => {
                let level = |n: u8| if n == 0 { 0 } else { 55 + 40 * n };
                let n = index - 16;
                Rgb::new(level(n / 36), level(n / 6 % 6), level(n % 6))
            }
            _ => {
                let grey = 8 + 10 * (index - 232);
                Rgb::new(grey, grey, grey)
            }
        }
    }

    /// Every colour a cell can name, in the emulator's numbering, with the program's changes applied.
    pub(crate) fn table(&self, changed: &Colors) -> Vec<Rgb> {
        let mut table: Vec<Rgb> = (0..=255u8).map(|index| self.indexed(index)).collect();
        table.resize(COUNT, self.foreground);
        table[NamedColor::Foreground as usize] = self.foreground;
        table[NamedColor::Background as usize] = self.background;
        table[NamedColor::Cursor as usize] = self.cursor.unwrap_or(self.foreground);
        table[NamedColor::BrightForeground as usize] = self.foreground;
        table[NamedColor::DimForeground as usize] = self.foreground.dim();
        for (n, slot) in (NamedColor::DimBlack as usize..=NamedColor::DimWhite as usize).enumerate() {
            table[slot] = self.ansi[n].dim();
        }
        for (index, slot) in table.iter_mut().enumerate() {
            if let Some(rgb) = changed[index] {
                *slot = Rgb::new(rgb.r, rgb.g, rgb.b);
            }
        }
        table
    }
}

/// How a cell is drawn besides its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CellFlags(u16);

#[expect(missing_docs, reason = "the names are the attributes")]
impl CellFlags {
    pub const BOLD: CellFlags = CellFlags(1);
    pub const ITALIC: CellFlags = CellFlags(1 << 1);
    pub const DIM: CellFlags = CellFlags(1 << 2);
    pub const UNDERLINE: CellFlags = CellFlags(1 << 3);
    pub const DOUBLE_UNDERLINE: CellFlags = CellFlags(1 << 4);
    pub const UNDERCURL: CellFlags = CellFlags(1 << 5);
    pub const DOTTED_UNDERLINE: CellFlags = CellFlags(1 << 6);
    pub const DASHED_UNDERLINE: CellFlags = CellFlags(1 << 7);
    pub const STRIKETHROUGH: CellFlags = CellFlags(1 << 8);
    /// Foreground and background are swapped; `Cell::fg` and `Cell::bg` already are.
    pub const INVERSE: CellFlags = CellFlags(1 << 9);
    /// The text is not drawn.
    pub const HIDDEN: CellFlags = CellFlags(1 << 10);
    /// A double-width character, drawn across this cell and the next.
    pub const WIDE: CellFlags = CellFlags(1 << 11);
    /// The second half of a double-width character; nothing to draw.
    pub const WIDE_SPACER: CellFlags = CellFlags(1 << 12);
    /// The line continues on the next row because it wrapped here.
    pub const WRAPPED: CellFlags = CellFlags(1 << 13);
}

impl CellFlags {
    /// Whether all of `flags` are set.
    pub fn contains(self, flags: CellFlags) -> bool {
        self.0 & flags.0 == flags.0
    }

    /// Whether no flag is set.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn from_term(flags: Flags) -> CellFlags {
        const PAIRS: [(Flags, CellFlags); 14] = [
            (Flags::BOLD, CellFlags::BOLD),
            (Flags::ITALIC, CellFlags::ITALIC),
            (Flags::DIM, CellFlags::DIM),
            (Flags::UNDERLINE, CellFlags::UNDERLINE),
            (Flags::DOUBLE_UNDERLINE, CellFlags::DOUBLE_UNDERLINE),
            (Flags::UNDERCURL, CellFlags::UNDERCURL),
            (Flags::DOTTED_UNDERLINE, CellFlags::DOTTED_UNDERLINE),
            (Flags::DASHED_UNDERLINE, CellFlags::DASHED_UNDERLINE),
            (Flags::STRIKEOUT, CellFlags::STRIKETHROUGH),
            (Flags::INVERSE, CellFlags::INVERSE),
            (Flags::HIDDEN, CellFlags::HIDDEN),
            (Flags::WIDE_CHAR, CellFlags::WIDE),
            (Flags::WIDE_CHAR_SPACER, CellFlags::WIDE_SPACER),
            (Flags::WRAPLINE, CellFlags::WRAPPED),
        ];
        let mut bits = 0;
        for (term, own) in PAIRS {
            if flags.contains(term) {
                bits |= own.0;
            }
        }
        // A wide character that did not fit at the end of a line leaves a placeholder there.
        if flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
            bits |= CellFlags::WIDE_SPACER.0;
        }
        CellFlags(bits)
    }
}

/// One cell of the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character; a space when empty.
    pub ch: char,
    /// Combining characters drawn on top of it.
    pub zero_width: Vec<char>,
    /// The text colour, inverse and faint already applied.
    pub fg: Rgb,
    /// The background, inverse already applied.
    pub bg: Rgb,
    /// A colour of its own for the underline (SGR 58).
    pub underline_color: Option<Rgb>,
    /// Attributes.
    pub flags: CellFlags,
    /// The target of an OSC 8 hyperlink over this cell.
    pub hyperlink: Option<String>,
}

/// A cell position on the visible screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Point {
    /// The line, from 0 at the top.
    pub line: usize,
    /// The column, from 0 at the left.
    pub column: usize,
}

/// The cursor's look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(missing_docs, reason = "the names are the shapes")]
pub enum CursorShape {
    Block,
    Underline,
    Beam,
    /// A frame, as for a terminal without focus.
    HollowBlock,
}

/// The cursor as drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cursor {
    /// Where it is.
    pub point: Point,
    /// Its shape.
    pub shape: CursorShape,
    /// Whether to draw it: the program has not hidden it and it is not scrolled out of view.
    pub visible: bool,
    /// The program asked for a blinking cursor.
    pub blinking: bool,
    /// It stands on a double-width character.
    pub wide: bool,
}

/// The selected part of the visible screen, from `start` to `end` inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Highlight {
    /// The first selected cell.
    pub start: Point,
    /// The last selected cell.
    pub end: Point,
    /// Only the columns between `start` and `end` on each line, as in a block selection.
    pub block: bool,
}

/// What changed since the previous snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Damage {
    /// Draw everything.
    Full,
    /// Draw these parts of lines; the rest is unchanged.
    Lines(Vec<LineDamage>),
}

/// The changed columns of a line, `left` to `right` inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(missing_docs, reason = "the struct says it")]
pub struct LineDamage {
    pub line: usize,
    pub left: usize,
    pub right: usize,
}

/// The visible screen at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Columns per line.
    pub columns: usize,
    /// Visible lines.
    pub lines: usize,
    /// The visible lines, top first, each `columns` cells long.
    pub rows: Vec<Vec<Cell>>,
    /// The cursor.
    pub cursor: Cursor,
    /// The selection where it is visible.
    pub selection: Option<Highlight>,
    /// How many lines the view is scrolled back into the history.
    pub display_offset: usize,
    /// How many lines the history holds.
    pub history: usize,
    /// What changed since the previous snapshot.
    pub damage: Damage,
    /// The background the screen is cleared to.
    pub background: Rgb,
    /// The input modes, such as whether the program takes the mouse.
    pub modes: InputModes,
}

pub(crate) fn take<T: EventListener>(term: &mut Term<T>, palette: &Palette) -> Snapshot {
    let table = palette.table(term.colors());
    let content = term.renderable_content();
    let display_offset = content.display_offset;
    let cursor = cursor(term, &content.cursor, display_offset);
    let selection = content.selection.and_then(|range| {
        let lines = term.screen_lines();
        let to_view = |line: Line| i64::from(line.0) + i64::try_from(display_offset).unwrap_or(0);
        let (start, end) = (to_view(range.start.line), to_view(range.end.line));
        let last = i64::try_from(lines).ok()? - 1;
        if end < 0 || start > last {
            return None;
        }
        let point = |line: i64, column: usize| Point {
            line: usize::try_from(line).unwrap_or(0),
            column,
        };
        Some(Highlight {
            start: if start < 0 {
                point(0, 0)
            } else {
                point(start, range.start.column.0)
            },
            end: if end > last {
                point(last, term.columns() - 1)
            } else {
                point(end, range.end.column.0)
            },
            block: range.is_block,
        })
    });
    let modes = InputModes::from_term(content.mode);
    let rows = visible_rows(term, &table, palette.bold_is_bright);
    let damage = match term.damage() {
        TermDamage::Full => Damage::Full,
        TermDamage::Partial(lines) => Damage::Lines(
            lines
                .map(|bounds| LineDamage {
                    line: bounds.line,
                    left: bounds.left,
                    right: bounds.right,
                })
                .collect(),
        ),
    };
    term.reset_damage();
    Snapshot {
        columns: term.columns(),
        lines: term.screen_lines(),
        rows,
        cursor,
        selection,
        display_offset,
        history: term.history_size(),
        damage,
        background: table[NamedColor::Background as usize],
        modes,
    }
}

fn cursor<T>(term: &Term<T>, cursor: &RenderableCursor, display_offset: usize) -> Cursor {
    let line = i64::from(cursor.point.line.0) + i64::try_from(display_offset).unwrap_or(0);
    let on_screen = usize::try_from(line).ok().filter(|&line| line < term.screen_lines());
    let style = term.cursor_style();
    let (shape, shown) = match cursor.shape {
        TermCursorShape::Block => (CursorShape::Block, true),
        TermCursorShape::Underline => (CursorShape::Underline, true),
        TermCursorShape::Beam => (CursorShape::Beam, true),
        TermCursorShape::HollowBlock => (CursorShape::HollowBlock, true),
        TermCursorShape::Hidden => (CursorShape::Block, false),
    };
    let wide = term.grid()[cursor.point].flags.contains(Flags::WIDE_CHAR);
    Cursor {
        point: Point {
            line: on_screen.unwrap_or(0),
            column: cursor.point.column.0,
        },
        shape,
        visible: shown && on_screen.is_some(),
        blinking: style.blinking,
        wide,
    }
}

fn visible_rows<T>(term: &Term<T>, table: &[Rgb], bold_is_bright: bool) -> Vec<Vec<Cell>> {
    let grid = term.grid();
    let offset = i32::try_from(grid.display_offset()).unwrap_or(0);
    (0..term.screen_lines())
        .map(|line| {
            let line = Line(i32::try_from(line).unwrap_or(0) - offset);
            let row = &grid[line];
            (0..term.columns())
                .map(|column| cell(&row[Column(column)], table, bold_is_bright))
                .collect()
        })
        .collect()
}

fn cell(cell: &TermCell, table: &[Rgb], bold_is_bright: bool) -> Cell {
    let mut fg = foreground(cell.fg, cell.flags, table, bold_is_bright);
    let mut bg = background(cell.bg, table);
    if cell.flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    Cell {
        ch: cell.c,
        zero_width: cell.zerowidth().map(<[char]>::to_vec).unwrap_or_default(),
        fg,
        bg,
        underline_color: cell.underline_color().map(|color| background(color, table)),
        flags: CellFlags::from_term(cell.flags),
        hyperlink: cell.hyperlink().map(|link| link.uri().to_string()),
    }
}

/// A text colour, following alacritty's rules for bold and faint text.
pub(crate) fn foreground(color: Color, flags: Flags, table: &[Rgb], bold_is_bright: bool) -> Rgb {
    let bold = flags.contains(Flags::BOLD);
    let dim = flags.contains(Flags::DIM);
    match color {
        Color::Spec(rgb) => {
            let rgb = Rgb::new(rgb.r, rgb.g, rgb.b);
            if dim { rgb.dim() } else { rgb }
        }
        Color::Named(named) => {
            let named = if dim {
                named.to_dim()
            } else if bold && bold_is_bright {
                named.to_bright()
            } else {
                named
            };
            table[named as usize]
        }
        Color::Indexed(index) => {
            let index = usize::from(index);
            let index = match index {
                0..=7 if dim => NamedColor::DimBlack as usize + index,
                8..=15 if dim => index - 8,
                0..=7 if bold && bold_is_bright => index + 8,
                _ => index,
            };
            table[index]
        }
    }
}

pub(crate) fn background(color: Color, table: &[Rgb]) -> Rgb {
    match color {
        Color::Spec(rgb) => Rgb::new(rgb.r, rgb.g, rgb.b),
        Color::Named(named) => table[named as usize],
        Color::Indexed(index) => table[usize::from(index)],
    }
}

/// The visible lines as plain text without trailing blanks, for VoiceOver.
pub(crate) fn visible_text<T>(term: &Term<T>) -> Vec<String> {
    let grid = term.grid();
    let offset = i32::try_from(grid.display_offset()).unwrap_or(0);
    (0..term.screen_lines())
        .map(|line| {
            let row = &grid[Line(i32::try_from(line).unwrap_or(0) - offset)];
            let mut text = String::with_capacity(term.columns());
            for column in 0..term.columns() {
                let cell = &row[Column(column)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                text.push(cell.c);
                text.extend(cell.zerowidth().into_iter().flatten());
            }
            text.truncate(text.trim_end().len());
            text
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::vte::ansi::Rgb as TermRgb;

    use super::*;

    #[test]
    fn the_256_colour_table_has_the_cube_and_the_grey_ramp() {
        let palette = Palette::default();
        assert_eq!(palette.indexed(1), palette.ansi[1]);
        assert_eq!(palette.indexed(16), Rgb::new(0, 0, 0));
        assert_eq!(palette.indexed(21), Rgb::new(0, 0, 255));
        assert_eq!(palette.indexed(196), Rgb::new(255, 0, 0));
        assert_eq!(palette.indexed(231), Rgb::new(255, 255, 255));
        assert_eq!(palette.indexed(232), Rgb::new(8, 8, 8));
        assert_eq!(palette.indexed(255), Rgb::new(238, 238, 238));
    }

    #[test]
    fn colours_resolve_through_palette_program_changes_bold_and_faint() {
        let palette = Palette::default();
        let mut changed = Colors::default();
        changed[2] = Some(TermRgb { r: 1, g: 2, b: 3 });
        let table = palette.table(&changed);
        let plain = Flags::empty();
        let red = Color::Named(NamedColor::Red);
        assert_eq!(foreground(red, plain, &table, true), palette.ansi[1]);
        assert_eq!(foreground(red, Flags::BOLD, &table, true), palette.ansi[9]);
        assert_eq!(foreground(red, Flags::BOLD, &table, false), palette.ansi[1]);
        assert_eq!(foreground(red, Flags::DIM, &table, true), palette.ansi[1].dim());
        assert_eq!(
            foreground(Color::Indexed(1), Flags::BOLD, &table, true),
            palette.ansi[9]
        );
        assert_eq!(foreground(Color::Indexed(9), Flags::DIM, &table, true), palette.ansi[1]);
        assert_eq!(foreground(Color::Indexed(2), plain, &table, true), Rgb::new(1, 2, 3));
        let spec = Color::Spec(TermRgb { r: 90, g: 60, b: 30 });
        assert_eq!(foreground(spec, Flags::DIM, &table, true), Rgb::new(60, 40, 20));
        assert_eq!(
            background(Color::Named(NamedColor::Background), &table),
            palette.background
        );
        assert_eq!(
            foreground(Color::Named(NamedColor::Foreground), plain, &table, true),
            palette.foreground
        );
    }
}
