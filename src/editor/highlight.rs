//! Colours of the text: the lines on screen at once, the rest in slices that leave the main thread free. Markdown
//! also gets the bold, italic and struck-through text of splat's source view.

use objc2::DefinedClass;
use objc2_app_kit::NSTextElementProvider;
use std::ops::Range;
use std::time::Duration;

use ide_document::syntax::{Language, Styled};
use ide_document::{Text, Unit};
use objc2::sel;
use objc2_app_kit::{
    NSColor, NSFontAttributeName, NSForegroundColorAttributeName, NSStrikethroughStyleAttributeName, NSTextStorage,
};
use objc2_foundation::{NSNumber, NSPoint, NSRange};

use super::style::{palette_color, styled_code_font, token_color};
use super::view::EditorTextView;
use crate::util::constant;

/// How long one slice of background colouring may hold the main thread.
const SLICE: Duration = Duration::from_millis(4);

/// Lines beyond the screen coloured with it, so a short scroll shows colour at once.
const MARGIN: usize = 40;

/// Colours what an edit changed once the run loop comes round.
pub fn schedule(view: &EditorTextView) {
    view.later(sel!(ideColorStep), 0.0);
}

/// Colours the visible lines and, while the document is not coloured through, a slice of the rest.
pub fn step(view: &EditorTextView) {
    let visible = visible_lines(view);
    let from = view.ivars().recolor_from.take();
    let wanted = match from {
        Some(line) => line.min(visible.start)..visible.end.max(line + 1),
        None => visible.clone(),
    };
    let changed = view.document_mut().highlight(wanted.clone());
    paint(view, union(changed, from.map(|line| line..line + 1)));
    let more = view.document_mut().highlight_more(SLICE);
    paint(view, more);
    if !view.document().is_highlighted() {
        view.later(sel!(ideColorStep), 0.01);
    }
}

/// Colours everything again, after the whole text was replaced or its language changed.
pub fn recolor_all(view: &EditorTextView) {
    let markdown = view.document().language().is_some_and(Language::is_markdown);
    if view.ivars().styled.get() && !markdown {
        view.ivars().styled.set(false);
        if let Some(storage) = view.storage() {
            let whole = NSRange::new(0, storage.length());
            with_editing(&storage, || plain_traits(&storage, whole, view.ivars().font_size.get()));
        }
    }
    let lines = view.document().text().line_count();
    let visible = visible_lines(view);
    let _ = view.document_mut().highlight(visible.clone());
    paint(view, Some(0..lines.min(visible.end.max(1))));
    schedule(view);
}

fn union(a: Option<Range<usize>>, b: Option<Range<usize>>) -> Option<Range<usize>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.start.min(b.start)..a.end.max(b.end)),
        (a, b) => a.or(b),
    }
}

/// The lines on screen with a margin on both sides.
pub(super) fn visible_lines(view: &EditorTextView) -> Range<usize> {
    let rect = view.visibleRect();
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

/// Sets the colours of `lines` in the view's storage from the document's spans, and Markdown's font traits.
fn paint(view: &EditorTextView, lines: Option<Range<usize>>) {
    let Some(lines) = lines else { return };
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
    let key = constant!(NSForegroundColorAttributeName);
    with_editing(&storage, || {
        for line in lines {
            let Some(whole) = line_range(text, line, length) else {
                break;
            };
            // SAFETY: The key is AppKit's foreground colour key and the value a colour.
            unsafe { storage.addAttribute_value_range(key, &NSColor::textColor(), whole) };
            for span in document.spans(line) {
                if let Some(range) = part(whole, span.start, span.end) {
                    // SAFETY: As above.
                    unsafe { storage.addAttribute_value_range(key, &token_color(span.kind), range) };
                }
            }
            if markdown {
                paint_styles(&storage, whole, document.styles(line), size);
            }
        }
    });
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
fn plain_traits(storage: &NSTextStorage, range: NSRange, size: f64) {
    let (font, strike) = (
        constant!(NSFontAttributeName),
        constant!(NSStrikethroughStyleAttributeName),
    );
    // SAFETY: AppKit's font and strike-through keys, with a font as the value of the first.
    unsafe {
        storage.addAttribute_value_range(font, &styled_code_font(size, false, false), range);
        storage.removeAttribute_range(strike, range);
    }
}

/// Markdown's colours and font traits of the line laid out at `line`.
fn paint_styles(storage: &NSTextStorage, line: NSRange, styles: &[Styled], size: f64) {
    plain_traits(storage, line, size);
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
                storage.addAttribute_value_range(color, &palette_color(token), range);
            }
            if style.bold || style.italic {
                storage.addAttribute_value_range(font, &styled_code_font(size, style.bold, style.italic), range);
            }
            if style.strike {
                storage.addAttribute_value_range(strike, &NSNumber::new_isize(1), range);
            }
        }
    }
}

/// Runs `f` as one edit of `storage`, ended also when `f` panics.
fn with_editing(storage: &NSTextStorage, f: impl FnOnce()) {
    struct End<'a>(&'a NSTextStorage);
    impl Drop for End<'_> {
        fn drop(&mut self) {
            self.0.endEditing();
        }
    }
    storage.beginEditing();
    let _end = End(storage);
    f();
}
