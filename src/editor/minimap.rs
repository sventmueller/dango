//! The minimap (E107): a column right of the editor's scroll view with a block of colour per run of code of each
//! logical line, the visible stretch as a frame, and marks for Git changes, diagnostics and the paused line. It draws
//! straight from the document on every redraw, only the rows that fit its height, and keeps no picture of its own.

use std::cell::{Cell, OnceCell, RefCell};
use std::ops::Range;

use ide_core::settings::{Settings, keys};
use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSEvent, NSLayoutConstraint, NSMenuItem, NSScrollView, NSView,
    NSViewBoundsDidChangeNotification, NSViewFrameDidChangeNotification,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize};

use super::view::EditorTextView;
use crate::util::{constant, guarded};

/// The column's width while it shows.
pub const WIDTH: f64 = 90.0;
/// Below this width of the editor, as with split tabs, the column hides.
const NARROWEST_EDITOR: f64 = 400.0;
const ROW: f64 = 2.0;
const COLUMN: f64 = 1.0;
const LEFT: f64 = 6.0;
const MARK: f64 = 3.0;

/// Whether the minimap shows, as the user last chose; read from the settings once.
#[derive(Default)]
pub struct Shown(Cell<Option<bool>>);

impl Shown {
    /// Reads the setting again on next use.
    pub fn forget(&self) {
        self.0.set(None);
    }
}

/// Whether editors show the minimap.
pub fn shown() -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let cx = crate::context::AppContext::get(mtm);
    let cell = &cx.editor.minimap.0;
    cell.get().unwrap_or_else(|| {
        let value = Settings::load_user().get(&keys::MINIMAP);
        cell.set(Some(value));
        value
    })
}

/// Shows the minimap in every editor, or hides it, and remembers the choice.
pub fn toggle() {
    let value = !shown();
    if let Some(mtm) = MainThreadMarker::new() {
        crate::context::AppContext::get(mtm).editor.minimap.0.set(Some(value));
    }
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::MINIMAP, &value) {
        crate::diag::report("saving the minimap setting", error);
    }
    for view in super::open_editors() {
        if let Some(map) = of(&view) {
            map.fit();
        }
    }
}

/// Whether `item` is the menu item of the minimap, which then gets its check mark.
pub fn validate(item: &NSMenuItem) -> Option<bool> {
    if item.action() != Some(objc2::sel!(ideToggleMinimap:)) {
        return None;
    }
    item.setState(if shown() {
        objc2_app_kit::NSControlStateValueOn
    } else {
        objc2_app_kit::NSControlStateValueOff
    });
    Some(true)
}

fn of(view: &EditorTextView) -> Option<Retained<Minimap>> {
    view.ivars().minimap.borrow().as_ref().and_then(Weak::load)
}

/// Draws the minimap of `view` again, if it shows.
pub fn redraw(view: &EditorTextView) {
    if let Some(map) = of(view).filter(|map| map.frame().size.width > 0.0) {
        map.setNeedsDisplay(true);
    }
}

pub struct MinimapIvars {
    editor: Weak<EditorTextView>,
    scroll: Weak<NSScrollView>,
    width: RefCell<Option<Retained<NSLayoutConstraint>>>,
    /// The scrolling and resizing it follows, which end with the column.
    watching: OnceCell<[crate::util::Observation; 2]>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Minimap` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEMinimap"]
    #[ivars = MinimapIvars]
    pub struct Minimap;

    unsafe impl NSObjectProtocol for Minimap {}

    impl Minimap {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, rect: NSRect) {
            guarded((), || self.draw(rect));
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            guarded((), || self.jump(event));
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            guarded((), || self.jump(event));
        }
    }
);

/// What the column shows: the rows of the lines no fold hides and where it starts in them.
struct Rows {
    /// The logical line of each row.
    lines: Vec<usize>,
    /// How far the rows are moved up, in points, so that a file longer than the column follows the scrolling.
    offset: f64,
    /// The rows the editor shows.
    visible: Range<usize>,
}

impl Minimap {
    /// The column for `editor` beside `scroll`, zero wide until `fit` finds it should show.
    pub fn new(mtm: MainThreadMarker, scroll: &NSScrollView, editor: &EditorTextView) -> Retained<Minimap> {
        let this = Self::alloc(mtm).set_ivars(MinimapIvars {
            editor: Weak::from(editor),
            scroll: Weak::from(scroll),
            width: RefCell::new(None),
            watching: OnceCell::new(),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        this.setClipsToBounds(true);
        // VoiceOver reads the position from the scroll bar.
        this.setAccessibilityElement(false);
        let clip = scroll.contentView();
        clip.setPostsBoundsChangedNotifications(true);
        scroll.setPostsFrameChangedNotifications(true);
        let map = Weak::from(&*this);
        let scrolled = crate::util::observe(mtm, constant!(NSViewBoundsDidChangeNotification), Some(&clip), {
            let map = map.clone();
            move |_| {
                if let Some(map) = map.load()
                    && map.frame().size.width > 0.0
                {
                    map.setNeedsDisplay(true);
                }
            }
        });
        let resized = crate::util::observe(
            mtm,
            constant!(NSViewFrameDidChangeNotification),
            Some(scroll),
            move |_| {
                // A width set while AppKit lays the window out takes effect only with the next pass.
                if let Some(editor) = map.load().and_then(|m| m.ivars().editor.load()) {
                    editor.later(objc2::sel!(ideFitMinimap), 0.0);
                }
            },
        );
        let _ = this.ivars().watching.set([scrolled, resized]);
        *editor.ivars().minimap.borrow_mut() = Some(Weak::from(&*this));
        this
    }

    /// Holds the column at `constraint`'s width, zero while hidden.
    pub fn set_width_constraint(&self, constraint: Retained<NSLayoutConstraint>) {
        *self.ivars().width.borrow_mut() = Some(constraint);
        self.fit();
    }

    /// Shows the column when the user wants it and the editor is wide enough, else makes it zero wide.
    pub fn fit(&self) {
        let Some(constraint) = self.ivars().width.borrow().clone() else {
            return;
        };
        let editor_width = self
            .ivars()
            .scroll
            .load()
            .map_or(0.0, |scroll| scroll.frame().size.width + self.frame().size.width);
        let width = if shown() && editor_width >= NARROWEST_EDITOR {
            WIDTH
        } else {
            0.0
        };
        if (constraint.constant() - width).abs() > f64::EPSILON {
            constraint.setConstant(width);
        }
        self.setNeedsDisplay(true);
    }

    /// The rows of `editor` and how they sit in a column `height` high.
    fn rows(&self, editor: &EditorTextView, height: f64) -> Rows {
        let lines = shown_lines(editor);
        let visible_rect = editor.visibleRect();
        let top = super::highlight::line_at(editor, visible_rect.origin).unwrap_or(0);
        let bottom_point = NSPoint::new(0.0, visible_rect.origin.y + visible_rect.size.height - 1.0);
        let bottom = super::highlight::line_at(editor, bottom_point).unwrap_or(top);
        let row_of = |line: usize| lines.partition_point(|&l| l < line);
        let visible = row_of(top)..(row_of(bottom) + 1).min(lines.len()).max(row_of(top));
        let content = lines.len() as f64 * ROW;
        let offset = if content <= height {
            0.0
        } else {
            let document = editor.frame().size.height;
            let room = (document - visible_rect.size.height).max(1.0);
            (visible_rect.origin.y / room).clamp(0.0, 1.0) * (content - height)
        };
        Rows { lines, offset, visible }
    }

    fn draw(&self, rect: NSRect) {
        NSColor::textBackgroundColor().setFill();
        objc2_app_kit::NSRectFill(rect);
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let size = self.bounds().size;
        NSColor::separatorColor().setFill();
        objc2_app_kit::NSRectFill(NSRect::new(
            NSPoint::new(0.0, rect.origin.y),
            NSSize::new(1.0, rect.size.height),
        ));
        let rows = self.rows(&editor, size.height);
        let first = index(rows.offset / ROW);
        let last = index(((rows.offset + size.height) / ROW).ceil());
        let paused = crate::debug::paused_line(&editor);
        let diagnostics = crate::lsp::line_marks(&editor);
        let lsp = editor.ivars().lsp.borrow();
        let painted = crate::lsp::painted(&lsp);
        let document = editor.document();
        let columns = index((size.width - LEFT - MARK) / COLUMN);
        for (row, &line) in rows
            .lines
            .iter()
            .enumerate()
            .take(last.min(rows.lines.len()))
            .skip(first)
        {
            let y = row as f64 * ROW - rows.offset;
            if paused == Some(line) {
                NSColor::systemGreenColor().colorWithAlphaComponent(0.35).setFill();
                objc2_app_kit::NSRectFill(NSRect::new(NSPoint::new(1.0, y), NSSize::new(size.width, ROW)));
            }
            let Some(text) = document.text().line(line) else {
                continue;
            };
            let start = editor_line_start(&document, line);
            for (range, color) in runs(&text, document.spans(line), painted, start, columns) {
                color.colorWithAlphaComponent(0.75).setFill();
                objc2_app_kit::NSRectFill(NSRect::new(
                    NSPoint::new(LEFT + range.start as f64 * COLUMN, y),
                    NSSize::new((range.end - range.start) as f64 * COLUMN, ROW - 0.5),
                ));
            }
            if let Some(mark) = super::changes::mark_at(&editor, line) {
                mark.color().setFill();
                objc2_app_kit::NSRectFill(NSRect::new(NSPoint::new(1.0, y), NSSize::new(MARK - 1.0, ROW)));
            }
            if let Some((severity, _)) = diagnostics.get(&line) {
                severity.color().setFill();
                objc2_app_kit::NSRectFill(NSRect::new(
                    NSPoint::new(size.width - MARK, y),
                    NSSize::new(MARK, ROW.max(2.0)),
                ));
            }
        }
        drop(document);
        drop(lsp);
        let frame = NSRect::new(
            NSPoint::new(1.0, rows.visible.start as f64 * ROW - rows.offset),
            NSSize::new(
                size.width - 1.0,
                (rows.visible.end - rows.visible.start).max(1) as f64 * ROW,
            ),
        );
        NSColor::labelColor().colorWithAlphaComponent(0.08).setFill();
        objc2_app_kit::NSRectFillUsingOperation(frame, objc2_app_kit::NSCompositingOperation::SourceOver);
        NSColor::tertiaryLabelColor().setStroke();
        objc2_app_kit::NSBezierPath::strokeRect(frame);
    }

    /// A click or drag scrolls the editor so that the line under the mouse is in the middle.
    fn jump(&self, event: &NSEvent) {
        let y = self.convertPoint_fromView(event.locationInWindow(), None).y;
        self.jump_to(y);
    }

    fn jump_to(&self, y: f64) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let rows = self.rows(&editor, self.bounds().size.height);
        let row = index((y + rows.offset) / ROW).min(rows.lines.len().saturating_sub(1));
        if let Some(&line) = rows.lines.get(row) {
            center_line(&editor, line);
        }
    }
}

/// The logical lines no fold hides, in order.
fn shown_lines(editor: &EditorTextView) -> Vec<usize> {
    let hidden = super::fold::hidden_ranges(editor);
    let document = editor.document();
    let count = document.text().line_count();
    if hidden.is_empty() {
        return (0..count).collect();
    }
    (0..count)
        .filter(|&line| {
            let start = editor_line_start(&document, line);
            !hidden.iter().any(|r| r.contains(&start))
        })
        .collect()
}

fn editor_line_start(document: &ide_document::Document, line: usize) -> usize {
    let text = document.text();
    text.offset(line, 0, ide_document::Unit::Utf16)
        .ok()
        .and_then(|byte| text.byte_to_utf16(byte).ok())
        .unwrap_or(0)
}

/// The runs of code of `text`, a line starting at UTF-16 offset `start`, as column ranges with their colour, at most
/// `columns` wide: whitespace stays empty, a semantic token's colour wins over the lexer's.
fn runs(
    text: &str,
    spans: &[ide_document::syntax::Span],
    painted: &[crate::lsp::Painted],
    start: usize,
    columns: usize,
) -> Vec<(Range<usize>, Retained<NSColor>)> {
    let first = painted.partition_point(|p| p.range.end <= start);
    let mut out: Vec<(Range<usize>, Kind)> = Vec::new();
    let mut column = 0usize;
    let mut unit = 0usize;
    for ch in text.chars() {
        if column >= columns || ch == '\n' || ch == '\r' {
            break;
        }
        let width = if ch == '\t' { 4 } else { 1 };
        if !ch.is_whitespace() {
            let at = start + unit;
            let kind = painted[first..]
                .iter()
                .take_while(|p| p.range.start <= at)
                .find(|p| p.range.contains(&at))
                .map(|p| Kind::Semantic(p.token))
                .or_else(|| {
                    spans
                        .iter()
                        .find(|s| (s.start as usize..s.end as usize).contains(&unit))
                        .map(|s| Kind::Lexer(s.kind))
                })
                .unwrap_or(Kind::Plain);
            match out.last_mut() {
                Some((range, last)) if range.end == column && *last == kind => range.end = column + 1,
                _ => out.push((column..column + 1, kind)),
            }
        }
        column += width;
        unit += ch.len_utf16();
    }
    out.into_iter().map(|(range, kind)| (range, kind.color())).collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    Lexer(ide_document::syntax::CodeToken),
    Semantic(splat_markdown::highlight::Token),
}

impl Kind {
    fn color(self) -> Retained<NSColor> {
        match self {
            Kind::Plain => NSColor::textColor(),
            Kind::Lexer(token) => super::style::token_color(token),
            Kind::Semantic(token) => super::style::palette_color(token),
        }
    }
}

/// Scrolls `editor` so that `line` sits in the middle of what it shows.
fn center_line(editor: &EditorTextView, line: usize) {
    let Some(start) = editor.line_start(line) else { return };
    editor.scrollRangeToVisible(NSRange::new(start, 0));
    let Some(rect) = super::line_rect(editor, line) else {
        return;
    };
    let Some(scroll) = editor.enclosingScrollView() else {
        return;
    };
    let clip = scroll.contentView();
    let visible = clip.bounds().size.height;
    let most = (editor.frame().size.height - visible).max(0.0);
    let y = (rect.origin.y + rect.size.height / 2.0 - visible / 2.0).clamp(0.0, most);
    clip.scrollToPoint(NSPoint::new(clip.bounds().origin.x, y));
    scroll.reflectScrolledClipView(&clip);
}

/// Fits the minimap of `view` to the editor's width.
pub fn fit(view: &EditorTextView) {
    if let Some(map) = of(view) {
        map.fit();
    }
}

/// The whole number of rows or columns in `value`, which is never negative or beyond a line count.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to a whole number within u32 first"
)]
fn index(value: f64) -> usize {
    value.floor().clamp(0.0, f64::from(u32::MAX)) as usize
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
