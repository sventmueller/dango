//! Colours of the text, lexed in slices that leave the main thread free. Code gets its colours as rendering
//! attributes of the layout manager, painted for the lines on screen and the rest once they come into view: an
//! attribute of the storage would make TextKit 2 lay its paragraph out again from estimated heights, which moves the
//! lines on screen and the places a jump lands on. The language server's colours paint over them. Markdown, whose
//! bold and italic text changes the layout anyway, keeps its colours and font traits in the storage.

mod unpainted;

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSTextElementProvider;
use std::ops::Range;
use std::time::{Duration, Instant};

use ide_document::syntax::{Language, Styled};
use ide_document::{Change, Document, Text, Unit};
use objc2::sel;
use objc2_app_kit::{
    NSColor, NSFontAttributeName, NSForegroundColorAttributeName, NSStrikethroughStyleAttributeName,
    NSTextContentStorage, NSTextLayoutManager,
};
use objc2_foundation::{NSMutableAttributedString, NSNumber, NSPoint, NSRange};

pub use self::unpainted::Unpainted;
use super::style::{palette_color, styled_code_font, token_color};
use super::view::EditorTextView;
use crate::util::{constant, with_editing};

/// How long one slice of background colouring may hold the main thread.
const SLICE: Duration = Duration::from_millis(4);

/// Lines beyond the screen coloured with it, so a short scroll shows colour at once.
const MARGIN: usize = 40;

/// Marks the lines `change` touched for painting, which `schedule` then does.
pub fn edited(view: &EditorTextView, change: &Change) {
    view.ivars()
        .unpainted
        .borrow_mut()
        .edited(change.start.line, change.old_end.line, change.new_end.line);
}

/// Colours what an edit changed once the run loop comes round.
pub fn schedule(view: &EditorTextView) {
    view.later(sel!(ideColorStep), 0.0);
}

fn is_markdown(view: &EditorTextView) -> bool {
    view.document().language().is_some_and(Language::is_markdown)
}

/// Lexes and paints the visible lines and, in a step that had nothing to paint on screen, lexes a slice of the rest
/// while the document is not lexed through. The step after a keystroke thus shares its turn of the run loop with
/// little more than the keystroke's own drawing, and catching up above the screen stops when the slice is over.
pub fn step(view: &EditorTextView) {
    let start = Instant::now();
    let visible = visible_lines(view);
    let changed = view.document_mut().highlight_within(visible.clone(), SLICE);
    let lexed = if is_markdown(view) {
        step_markdown(view, start, changed)
    } else {
        step_code(view, start, &visible, changed)
    };
    if !view.document().is_highlighted() {
        view.later(sel!(ideColorStep), 0.01);
    }
    if lexed {
        super::minimap::redraw(view);
    }
}

/// The rest of `step` for code; whether lexing changed colours.
fn step_code(view: &EditorTextView, start: Instant, visible: &Range<usize>, changed: Option<Range<usize>>) -> bool {
    let mut lexed = changed.is_some();
    if let Some(lines) = changed {
        view.ivars().unpainted.borrow_mut().add(lines);
    }
    if !paint_due(view, visible) {
        let more = view
            .document_mut()
            .highlight_more(SLICE.saturating_sub(start.elapsed()));
        lexed |= more.is_some();
        if let Some(lines) = more {
            view.ivars().unpainted.borrow_mut().add(lines);
            paint_due(view, visible);
        }
    }
    lexed
}

/// The rest of `step` for Markdown, which paints every line whose colours changed at once; whether lexing changed
/// colours.
fn step_markdown(view: &EditorTextView, start: Instant, changed: Option<Range<usize>>) -> bool {
    let edited = view.ivars().unpainted.take().span();
    let lexed = changed.is_some();
    if let Some(lines) = union(changed, edited) {
        paint_storage(view, lines);
    }
    let more = view
        .document_mut()
        .highlight_more(SLICE.saturating_sub(start.elapsed()));
    if let Some(lines) = more.clone() {
        paint_storage(view, lines);
    }
    lexed || more.is_some()
}

fn union(a: Option<Range<usize>>, b: Option<Range<usize>>) -> Option<Range<usize>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.start.min(b.start)..a.end.max(b.end)),
        (a, b) => a.or(b),
    }
}

/// After TextKit laid out the lines on screen: those whose colours wait get them before they are drawn.
pub fn laid_out(view: &EditorTextView) {
    if view.ivars().unpainted.borrow().is_empty() || is_markdown(view) {
        return;
    }
    let visible = visible_lines(view);
    if view.ivars().unpainted.borrow().meets(&visible) {
        paint_due(view, &visible);
    }
}

/// Colours everything again, after the whole text was replaced or its language changed.
pub fn recolor_all(view: &EditorTextView) {
    let markdown = is_markdown(view);
    if view.ivars().styled.get() && !markdown {
        view.ivars().styled.set(false);
        if let Some(storage) = view.storage() {
            let whole = NSRange::new(0, storage.length());
            with_editing(&storage, || plain_traits(&storage, whole, view.ivars().font_size.get()));
        }
    }
    let lines = view.document().text().line_count();
    let visible = visible_lines(view);
    let _ = view.document_mut().highlight_within(visible.clone(), SLICE);
    {
        let mut unpainted = view.ivars().unpainted.borrow_mut();
        *unpainted = Unpainted::default();
        if !markdown {
            unpainted.add(0..lines.max(1));
        }
    }
    if markdown {
        if let Some((layout, content)) = layout_of(view) {
            layout.removeRenderingAttribute_forTextRange(
                constant!(NSForegroundColorAttributeName),
                &content.documentRange(),
            );
            view.paint_found();
        }
        paint_storage(view, 0..lines.min(visible.end.max(1)));
    } else {
        paint_due(view, &visible);
    }
    schedule(view);
}

/// Paints the lines of `lines` whose colours wait; whether there were any.
fn paint_due(view: &EditorTextView, lines: &Range<usize>) -> bool {
    let due = view.ivars().unpainted.borrow_mut().take_in(lines);
    let Some(due) = due else { return false };
    if let Some(utf16) = paint_shown(view, due.clone()) {
        ide_ui_lsp::paint_tokens(view, &utf16);
        view.paint_found();
    } else {
        view.ivars().unpainted.borrow_mut().add(due);
    }
    true
}

/// Gives the UTF-16 range `range` the colours of the lexing again, which the language server's then paint over.
pub(super) fn paint_under_server(view: &EditorTextView, range: &Range<usize>) {
    if is_markdown(view) {
        if let Some((layout, content)) = layout_of(view)
            && let Some(range) = ide_ui_lsp::text_range(&content, range)
        {
            layout.removeRenderingAttribute_forTextRange(constant!(NSForegroundColorAttributeName), &range);
        }
        return;
    }
    let lines = {
        let document = view.document();
        let text = document.text();
        let line = |utf16: usize| {
            text.utf16_to_byte(utf16.min(text.len_utf16()))
                .map_or(0, |byte| text.line_of(byte))
        };
        line(range.start)..line(range.end) + 1
    };
    let _ = view.ivars().unpainted.borrow_mut().take_in(&lines);
    let _ = paint_shown(view, lines);
}

fn layout_of(view: &EditorTextView) -> Option<(Retained<NSTextLayoutManager>, Retained<NSTextContentStorage>)> {
    Some((view.textLayoutManager()?, view.textContentStorage()?))
}

/// Sets the colours of `lines` as rendering attributes from the document's spans; the UTF-16 range painted, unless
/// the view holds other text than the document.
fn paint_shown(view: &EditorTextView, lines: Range<usize>) -> Option<Range<usize>> {
    let (layout, content) = layout_of(view)?;
    let document = view.document();
    let text = document.text();
    let length = text.len_utf16();
    if view.storage()?.length() != length {
        return None;
    }
    let key = constant!(NSForegroundColorAttributeName);
    let plain: Retained<AnyObject> = NSColor::textColor().into();
    let mut painted: Option<Range<usize>> = None;
    for line in lines {
        let Some(whole) = line_range(text, line, length) else {
            break;
        };
        let utf16 = whole.location..whole.location + whole.length;
        let set = |range: &Range<usize>, color: &AnyObject| {
            if let Some(range) = ide_ui_lsp::text_range(&content, range) {
                // SAFETY: The value of the foreground colour attribute is a colour.
                unsafe { layout.addRenderingAttribute_value_forTextRange(key, Some(color), &range) };
            }
        };
        set(&utf16, &plain);
        for span in document.spans(line) {
            if let Some(part) = part(whole, span.start, span.end) {
                let color: Retained<AnyObject> = token_color(span.kind).into();
                set(&(part.location..part.location + part.length), &color);
            }
        }
        painted = Some(painted.map_or(utf16.start, |p| p.start)..utf16.end);
    }
    painted
}

/// Sets the colours of `lines` in the view's storage from the document's spans, and Markdown's font traits.
fn paint_storage(view: &EditorTextView, lines: Range<usize>) {
    let Some(storage) = view.storage() else { return };
    let document = view.document();
    let text = document.text();
    let length = text.len_utf16();
    if storage.length() != length {
        return;
    }
    let markdown = document.language().is_some_and(Language::is_markdown);
    view.ivars().styled.set(view.ivars().styled.get() || markdown);
    let size = view.ivars().font_size.get();
    with_editing(&storage, || {
        for line in lines {
            let Some(whole) = line_range(text, line, length) else {
                break;
            };
            paint_line(&storage, whole, &document, line, markdown, size);
        }
    });
}

/// Gives `string`, which holds line `line` of `view` from its start, the colours and Markdown's font traits the
/// view shows for that line once it is on screen.
pub(super) fn paint_copy(view: &EditorTextView, line: usize, string: &NSMutableAttributedString) {
    let document = view.document();
    let markdown = document.language().is_some_and(Language::is_markdown);
    let whole = NSRange::new(0, string.length());
    paint_line(string, whole, &document, line, markdown, view.ivars().font_size.get());
}

/// The colours of line `line` of `document` over `whole` of `string`, and in Markdown its font traits.
fn paint_line(
    string: &NSMutableAttributedString,
    whole: NSRange,
    document: &Document,
    line: usize,
    markdown: bool,
    size: f64,
) {
    let key = constant!(NSForegroundColorAttributeName);
    // SAFETY: The key is AppKit's foreground colour key and the value a colour.
    unsafe { string.addAttribute_value_range(key, &NSColor::textColor(), whole) };
    for span in document.spans(line) {
        if let Some(range) = part(whole, span.start, span.end) {
            // SAFETY: AppKit's foreground colour key with a colour value, over a range the span keeps inside the string.
            unsafe { string.addAttribute_value_range(key, &token_color(span.kind), range) };
        }
    }
    if markdown {
        paint_styles(string, whole, document.styles(line), size);
    }
}

/// The lines on screen with a margin on both sides.
pub(super) fn visible_lines(view: &EditorTextView) -> Range<usize> {
    lines_in(view, view.visibleRect())
}

/// The lines laid out in `rect` of the view, with a margin on both sides.
pub(super) fn lines_in(view: &EditorTextView, rect: objc2_foundation::NSRect) -> Range<usize> {
    let top = line_at(view, NSPoint::new(0.0, rect.origin.y)).unwrap_or(0);
    let bottom = line_at(view, NSPoint::new(0.0, rect.origin.y + rect.size.height)).unwrap_or(top + 80);
    let count = view.document().text().line_count();
    top.saturating_sub(MARGIN)..(bottom + MARGIN).min(count)
}

/// The line laid out at `point` of the view.
pub fn line_at(view: &EditorTextView, point: NSPoint) -> Option<usize> {
    let layout = view.textLayoutManager()?;
    let content = view.textContentStorage()?;
    let origin = view.textContainerOrigin();
    let fragment = layout.textLayoutFragmentForPosition(NSPoint::new(point.x, point.y - origin.y))?;
    let start = content.documentRange().location();
    let offset = content.offsetFromLocation_toLocation(&start, &fragment.rangeInElement().location());
    let text = view.document();
    let byte = text.text().utf16_to_byte(usize::try_from(offset).ok()?).ok()?;
    Some(text.text().line_of(byte))
}

/// The UTF-16 range of line `line` with its line break.
fn line_range(text: &Text, line: usize, length: usize) -> Option<NSRange> {
    let utf16 = |byte: usize| text.byte_to_utf16(byte).ok();
    let start = text.offset(line, 0, Unit::Utf16).ok().and_then(utf16)?;
    let end = text
        .offset(line + 1, 0, Unit::Utf16)
        .ok()
        .and_then(utf16)
        .unwrap_or(length);
    Some(NSRange::new(start, end - start))
}

/// Columns `start..end` of a line laid out at `line`, where any of them are inside it.
fn part(line: NSRange, start: u32, end: u32) -> Option<NSRange> {
    let from = line.location + start as usize;
    let to = (line.location + end as usize).min(line.location + line.length);
    (from < to).then(|| NSRange::new(from, to - from))
}

/// The plain code font and no strike-through over `range`.
fn plain_traits(string: &NSMutableAttributedString, range: NSRange, size: f64) {
    let (font, strike) = (
        constant!(NSFontAttributeName),
        constant!(NSStrikethroughStyleAttributeName),
    );
    // SAFETY: AppKit's font and strike-through keys, with a font as the value of the first.
    unsafe {
        string.addAttribute_value_range(font, &styled_code_font(size, false, false), range);
        string.removeAttribute_range(strike, range);
    }
}

/// Markdown's colours and font traits of the line laid out at `line`.
fn paint_styles(string: &NSMutableAttributedString, line: NSRange, styles: &[Styled], size: f64) {
    plain_traits(string, line, size);
    let (color, font, strike) = (
        constant!(NSForegroundColorAttributeName),
        constant!(NSFontAttributeName),
        constant!(NSStrikethroughStyleAttributeName),
    );
    for styled in styles {
        let Some(range) = part(line, styled.start, styled.end) else {
            continue;
        };
        let style = styled.style;
        // SAFETY: AppKit's colour, font and strike-through keys, with a colour, a font and a number as their values.
        unsafe {
            if let Some(token) = style.token {
                string.addAttribute_value_range(color, &palette_color(token), range);
            }
            if style.bold || style.italic {
                string.addAttribute_value_range(font, &styled_code_font(size, style.bold, style.italic), range);
            }
            if style.strike {
                string.addAttribute_value_range(strike, &NSNumber::new_isize(1), range);
            }
        }
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
