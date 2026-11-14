//! Colours of the text: the lines on screen at once, the rest in slices that leave the main thread free. Markdown
//! also gets the bold, italic and struck-through text of splat's source view.

use objc2::DefinedClass;
use objc2_app_kit::NSTextElementProvider;
use std::ops::Range;
use std::time::{Duration, Instant};

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

/// Lines painted between two looks at the clock.
const LINES_PER_LOOK: usize = 16;

/// Lines whose colours wait to be painted into the storage, as sorted ranges that neither overlap nor touch.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Unpainted(Vec<Range<usize>>);

impl Unpainted {
    fn add(&mut self, lines: Range<usize>) {
        if lines.is_empty() {
            return;
        }
        let at = self.0.partition_point(|r| r.end < lines.start);
        let mut merged = lines;
        while let Some(next) = self.0.get(at).filter(|r| r.start <= merged.end) {
            merged = merged.start.min(next.start)..merged.end.max(next.end);
            self.0.remove(at);
        }
        self.0.insert(at, merged);
    }

    /// Lines `first..=old_end` became `first..=new_end`: the ranges below move with them, the edited lines join.
    fn edited(&mut self, first: usize, old_end: usize, new_end: usize) {
        let moved = |line: usize, edited: usize| {
            if line < first {
                line
            } else if line > old_end {
                line - old_end + new_end
            } else {
                edited
            }
        };
        let ranges = std::mem::take(&mut self.0);
        for range in ranges {
            self.add(moved(range.start, first)..moved(range.end - 1, new_end) + 1);
        }
        self.add(first..new_end + 1);
    }

    /// Takes out the lines of `lines` and returns the stretch they span.
    fn take_in(&mut self, lines: &Range<usize>) -> Option<Range<usize>> {
        let mut taken: Option<Range<usize>> = None;
        let mut kept = Vec::with_capacity(self.0.len() + 1);
        for range in std::mem::take(&mut self.0) {
            let (start, end) = (range.start.max(lines.start), range.end.min(lines.end));
            if start >= end {
                kept.push(range);
                continue;
            }
            taken = Some(taken.map_or(start..end, |t| t.start.min(start)..t.end.max(end)));
            kept.extend(
                [range.start..start, end..range.end]
                    .into_iter()
                    .filter(|r| !r.is_empty()),
            );
        }
        self.0 = kept;
        taken
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Colours what an edit changed once the run loop comes round.
pub fn edited(view: &EditorTextView, change: &ide_document::Change) {
    view.ivars()
        .unpainted
        .borrow_mut()
        .edited(change.start.line, change.old_end.line, change.new_end.line);
    schedule(view);
}

fn schedule(view: &EditorTextView) {
    view.later(sel!(ideColorStep), 0.0);
}

/// Colours the visible lines and, while the document is not coloured through, a slice of the rest: the lexing above
/// the screen, the lexing below and the painting of what lies off screen each stop when the slice is over.
pub fn step(view: &EditorTextView) {
    let start = Instant::now();
    let visible = visible_lines(view);
    let mut unpainted = view.ivars().unpainted.take();
    let changed = view.document_mut().highlight_within(visible.clone(), SLICE / 2);
    let more = view
        .document_mut()
        .highlight_more((SLICE / 2).saturating_sub(start.elapsed()));
    for lines in [changed, more].into_iter().flatten() {
        unpainted.add(lines);
    }
    if let Some(on_screen) = unpainted.take_in(&visible) {
        paint(view, on_screen, None);
    }
    while let Some(lines) = unpainted.0.first().cloned() {
        let reached = paint(view, lines.clone(), Some(start + SLICE));
        unpainted.0[0] = reached..lines.end;
        if reached < lines.end {
            break;
        }
        unpainted.0.remove(0);
    }
    let rest = !unpainted.is_empty();
    let mut kept = view.ivars().unpainted.borrow_mut();
    for lines in unpainted.0 {
        kept.add(lines);
    }
    drop(kept);
    if rest || !view.document().is_highlighted() {
        view.later(sel!(ideColorStep), 0.01);
    }
    super::minimap::redraw(view);
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
    let _ = view.document_mut().highlight_within(visible.clone(), SLICE);
    let on_screen = {
        let mut unpainted = view.ivars().unpainted.borrow_mut();
        *unpainted = Unpainted::default();
        unpainted.add(0..lines.max(1));
        unpainted.take_in(&visible)
    };
    if let Some(on_screen) = on_screen {
        paint(view, on_screen, None);
    }
    schedule(view);
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

/// Sets the colours of `lines` in the view's storage from the document's spans, and Markdown's font traits, until
/// `deadline`; returns the line it stopped before.
fn paint(view: &EditorTextView, lines: Range<usize>, deadline: Option<Instant>) -> usize {
    let Some(storage) = view.storage() else {
        return lines.start;
    };
    let document = view.document();
    let text = document.text();
    let length = text.len_utf16();
    if storage.length() != length {
        return lines.start;
    }
    let markdown = document.language().is_some_and(Language::is_markdown);
    view.ivars().styled.set(view.ivars().styled.get() || markdown);
    let size = view.ivars().font_size.get();
    let key = constant!(NSForegroundColorAttributeName);
    let mut reached = lines.start;
    with_editing(&storage, || {
        for line in lines.clone() {
            if (line - lines.start) % LINES_PER_LOOK == LINES_PER_LOOK - 1
                && deadline.is_some_and(|deadline| Instant::now() >= deadline)
            {
                return;
            }
            reached = line + 1;
            let Some(whole) = line_range(text, line, length) else {
                reached = lines.end;
                return;
            };
            // SAFETY: The key is AppKit's foreground colour key and the value a colour.
            unsafe { storage.addAttribute_value_range(key, &NSColor::textColor(), whole) };
            for span in document.spans(line) {
                if let Some(range) = part(whole, span.start, span.end) {
                    // SAFETY: AppKit's foreground colour key with a colour value, over a range the span keeps inside the storage.
                    unsafe { storage.addAttribute_value_range(key, &token_color(span.kind), range) };
                }
            }
            if markdown {
                paint_styles(&storage, whole, document.styles(line), size);
            }
        }
    });
    reached
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

#[cfg(test)]
mod tests {
    use super::Unpainted;

    fn one(lines: std::ops::Range<usize>) -> Vec<std::ops::Range<usize>> {
        std::iter::once(lines).collect()
    }

    #[test]
    fn unpainted_lines_merge_where_they_meet_and_give_up_what_is_taken() {
        let mut lines = Unpainted::default();
        lines.add(10..20);
        lines.add(30..40);
        lines.add(20..25);
        assert_eq!(lines.0, [10..25, 30..40]);
        lines.add(5..31);
        assert_eq!(lines.0, one(5..40));
        assert_eq!(lines.take_in(&(12..18)), Some(12..18));
        assert_eq!(lines.0, [5..12, 18..40]);
        assert_eq!(lines.take_in(&(0..100)), Some(5..40));
        assert!(lines.is_empty());
    }

    #[test]
    fn unpainted_lines_follow_an_edit_and_take_in_its_lines() {
        let mut lines = Unpainted(vec![2..4, 50..60]);
        lines.edited(10, 10, 12);
        assert_eq!(lines.0, [2..4, 10..13, 52..62], "two lines more below line 10");
        let mut lines = Unpainted(one(50..60));
        lines.edited(10, 15, 10);
        assert_eq!(lines.0, [10..11, 45..55], "five lines fewer");
        let mut lines = Unpainted(one(8..14));
        lines.edited(10, 12, 10);
        assert_eq!(
            lines.0,
            one(8..12),
            "a range through the edit keeps its start and ends after what is left"
        );
    }
}
