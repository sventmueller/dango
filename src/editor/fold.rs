//! Folding: the ranges come from the language server where one runs, else from the indentation. A folded range's
//! lines are left out of TextKit 2's enumeration of paragraphs, so they take no room and line numbers skip them; its
//! first line stays and ends in a placeholder that unfolds it. Editing inside a fold unfolds it.

use std::collections::BTreeMap;
use std::ops::Range;

use ide_document::Change;
use objc2::DefinedClass;
use objc2_app_kit::{NSTextContentManager, NSTextElement, NSTextElementProvider, NSView};
use objc2_foundation::{NSPoint, NSRange, NSRect, NSSize};

use super::view::EditorTextView;

/// The width of a fold's placeholder after its first line.
pub const PLACEHOLDER_WIDTH: f64 = 26.0;

/// What a view knows about folds.
#[derive(Debug, Default)]
pub struct Folds {
    /// The ranges the language server offers, as the first line and the last hidden line.
    server: Option<Vec<(usize, usize)>>,
    /// The indentation's ranges for a revision of the document.
    indented: Option<(u64, Vec<(usize, usize)>)>,
    /// The folded ranges: UTF-16 code units from the start of the second line to the start of the line after the last
    /// hidden one, or the end of the text; sorted by start.
    hidden: Vec<Range<usize>>,
}

/// The ranges that can fold, as first line and last hidden line, ordered by first line.
fn available(view: &EditorTextView) -> Vec<(usize, usize)> {
    let revision = view.document().revision();
    let mut folds = view.ivars().folds.borrow_mut();
    if let Some(server) = &folds.server {
        return server.clone();
    }
    if let Some((seen, ranges)) = &folds.indented
        && *seen == revision
    {
        return ranges.clone();
    }
    let tab_width = if view.ivars().indent.borrow().as_str() == "\t" {
        4
    } else {
        view.ivars().indent.borrow().len()
    };
    let ranges: Vec<(usize, usize)> = view
        .document()
        .folds(tab_width.max(1))
        .into_iter()
        .filter(|f| f.end > f.start)
        .map(|f| (f.start, f.end))
        .collect();
    folds.indented = Some((revision, ranges.clone()));
    ranges
}

/// Takes the language server's folding ranges, or `None` to go back to the indentation.
pub fn set_server_ranges(view: &EditorTextView, ranges: Option<Vec<(usize, usize)>>) {
    view.ivars().folds.borrow_mut().server = ranges.map(|mut r| {
        r.retain(|(start, end)| end > start);
        r.sort_unstable();
        r
    });
    super::redraw_gutter(view);
}

/// The UTF-16 offset where line `line` starts, or the end of the text past its last line.
fn line_offset(view: &EditorTextView, line: usize) -> usize {
    view.line_start(line)
        .unwrap_or_else(|| view.document().text().len_utf16())
}

/// The line UTF-16 offset `offset` lies on.
fn line_of(view: &EditorTextView, offset: usize) -> usize {
    let document = view.document();
    let text = document.text();
    text.utf16_to_byte(offset.min(text.len_utf16()))
        .map_or(0, |byte| text.line_of(byte))
}

/// Folds lines `start + 1 ..= end` under line `start`.
fn fold(view: &EditorTextView, start: usize, end: usize) {
    let hidden = line_offset(view, start + 1)..line_offset(view, end + 1);
    if hidden.is_empty() {
        return;
    }
    {
        let mut folds = view.ivars().folds.borrow_mut();
        if folds.hidden.contains(&hidden) {
            return;
        }
        folds.hidden.push(hidden.clone());
        folds.hidden.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
    }
    let selection = view.selectedRange();
    if selection.location >= hidden.start && selection.location < hidden.end {
        let header_end = hidden.start.saturating_sub(newline_before(view, hidden.start));
        view.setSelectedRange(NSRange::new(header_end, 0));
    }
    relayout(view);
}

/// The length of the line break that ends just before UTF-16 offset `at`.
fn newline_before(view: &EditorTextView, at: usize) -> usize {
    let string = view.string();
    match (
        at.checked_sub(2).map(|i| string.characterAtIndex(i)),
        at.checked_sub(1).map(|i| string.characterAtIndex(i)),
    ) {
        (Some(0x0D), Some(0x0A)) => 2,
        (_, Some(0x0A | 0x0D)) => 1,
        _ => 0,
    }
}

/// Unfolds the folds `remove` picks; whether any went.
fn unfold_where(view: &EditorTextView, remove: impl Fn(&Range<usize>) -> bool) -> bool {
    let removed = {
        let mut folds = view.ivars().folds.borrow_mut();
        let before = folds.hidden.len();
        folds.hidden.retain(|r| !remove(r));
        folds.hidden.len() != before
    };
    if removed {
        relayout(view);
    }
    removed
}

/// Lays out the text again, so folded lines go or come back.
pub fn relayout(view: &EditorTextView) {
    if let (Some(layout), Some(content)) = (view.textLayoutManager(), view.textContentStorage()) {
        layout.invalidateLayoutForRange(&content.documentRange());
        layout.textViewportLayoutController().layoutViewport();
    }
    view.setNeedsDisplay(true);
    redraw_fragments(view);
    super::redraw_gutter(view);
    super::overlay::redraw(view);
    super::whitespace::redraw(view);
    crate::lsp::paint(view);
}

/// Has every view that draws a paragraph draw again: TextKit keeps the drawing of a paragraph whose own text did not
/// change, and its insets at the line's end move when the fold's placeholder after it comes or goes.
fn redraw_fragments(view: &NSView) {
    for child in view.subviews().iter() {
        child.setNeedsDisplay(true);
        redraw_fragments(&child);
    }
}

/// Folds the innermost range around the cursor's line, or the one starting on it.
pub fn fold_at_cursor(view: &EditorTextView) {
    let line = line_of(view, view.selectedRange().location);
    let ranges = available(view);
    let found = ranges
        .iter()
        .filter(|(start, end)| *start <= line && line <= *end)
        .min_by_key(|(start, end)| (usize::from(*start != line), end - start));
    if let Some((start, end)) = found {
        fold(view, *start, *end);
    }
}

/// Unfolds what hides the cursor's line or starts on it.
pub fn unfold_at_cursor(view: &EditorTextView) {
    let at = view.selectedRange().location;
    let line = line_of(view, at);
    let next = line_offset(view, line + 1);
    unfold_where(view, |r| r.start == next || r.contains(&at));
}

pub fn fold_all(view: &EditorTextView) {
    for (start, end) in available(view) {
        fold(view, start, end);
    }
}

pub fn unfold_all(view: &EditorTextView) {
    unfold_where(view, |_| true);
}

/// Folds or unfolds the range that starts on `line`, as a click on its triangle does.
pub fn toggle(view: &EditorTextView, line: usize) {
    let next = line_offset(view, line + 1);
    if unfold_where(view, |r| r.start == next) {
        return;
    }
    if let Some((start, end)) = available(view).into_iter().find(|(start, _)| *start == line) {
        fold(view, start, end);
    }
}

/// Keeps folds on their lines while the text changes, and unfolds one that an edit reaches into.
pub fn shift(view: &EditorTextView, change: &Change) {
    let touched = {
        let mut folds = view.ivars().folds.borrow_mut();
        let before = folds.hidden.len();
        folds.hidden.retain_mut(|r| {
            let inside = change.utf16.start > r.start && change.utf16.start < r.end
                || change.utf16.end > r.start && change.utf16.start < r.end;
            !inside && crate::lsp::follow(r, change)
        });
        folds.hidden.len() != before
    };
    if touched {
        // The storage is still processing the edit, and TextKit must not enumerate its paragraphs meanwhile.
        view.later(objc2::sel!(ideRelayoutFolds), 0.0);
    }
}

/// Whether TextKit should lay out `element`: not when a fold hides it. A view that is busy shows everything.
pub fn shows(view: &EditorTextView, manager: &NSTextContentManager, element: &NSTextElement) -> bool {
    let Ok(folds) = view.ivars().folds.try_borrow() else {
        return true;
    };
    if folds.hidden.is_empty() {
        return true;
    }
    let Some(range) = element.elementRange() else {
        return true;
    };
    let offset = manager.offsetFromLocation_toLocation(&manager.documentRange().location(), &range.location());
    let Ok(offset) = usize::try_from(offset) else {
        return true;
    };
    !folds.hidden.iter().any(|r| r.contains(&offset))
}

/// Whether a fold hides UTF-16 offset `offset`.
pub fn hidden_by_fold(view: &EditorTextView, offset: usize) -> bool {
    view.ivars()
        .folds
        .try_borrow()
        .is_ok_and(|folds| folds.hidden.iter().any(|r| r.contains(&offset)))
}

/// The folds that are not inside another fold, as their hidden ranges.
fn outermost(view: &EditorTextView) -> Vec<Range<usize>> {
    let folds = view.ivars().folds.borrow();
    let mut out: Vec<Range<usize>> = vec![];
    for r in &folds.hidden {
        if !out.iter().any(|o| o.start <= r.start && r.end <= o.end) {
            out.push(r.clone());
        }
    }
    out
}

/// The placeholders of the folds on screen: where each is drawn, in the view's coordinates, with its hidden range.
pub fn placeholders(view: &EditorTextView) -> Vec<(NSRect, Range<usize>)> {
    let visible = super::visible_utf16(view);
    outermost(view)
        .into_iter()
        .filter(|r| r.start >= visible.start.saturating_sub(1) && r.start <= visible.end)
        .filter_map(|hidden| {
            let header_end = hidden.start.saturating_sub(newline_before(view, hidden.start));
            let rect = crate::lsp::rect_of(view, header_end..header_end)?;
            let at = NSRect::new(
                NSPoint::new(rect.origin.x + 4.0, rect.origin.y + 1.0),
                NSSize::new(PLACEHOLDER_WIDTH, (rect.size.height - 2.0).max(8.0)),
            );
            Some((at, hidden))
        })
        .collect()
}

/// Unfolds the fold whose placeholder lies under `point` of the view; whether there was one.
pub fn click(view: &EditorTextView, point: NSPoint) -> bool {
    let hit = placeholders(view).into_iter().find(|(rect, _)| {
        point.x >= rect.origin.x
            && point.x <= rect.origin.x + rect.size.width
            && point.y >= rect.origin.y
            && point.y <= rect.origin.y + rect.size.height
    });
    match hit {
        Some((_, hidden)) => unfold_where(view, |r| *r == hidden),
        None => false,
    }
}

/// The lines that can fold, with whether they are folded, for the line number column.
pub fn gutter(view: &EditorTextView) -> BTreeMap<usize, bool> {
    let mut lines: BTreeMap<usize, bool> = available(view).into_iter().map(|(start, _)| (start, false)).collect();
    let hidden: Vec<Range<usize>> = view.ivars().folds.borrow().hidden.clone();
    for range in hidden {
        lines.insert(line_of(view, range.start).saturating_sub(1), true);
    }
    lines
}

/// Whether the folds come from the language server, for the self-test.
#[cfg(feature = "selftest")]
pub fn from_server(view: &EditorTextView) -> bool {
    view.ivars().folds.borrow().server.is_some()
}

/// The lines TextKit lays out, in order, for the self-test: folded ones are missing.
#[cfg(feature = "selftest")]
pub fn laid_out_lines(view: &EditorTextView) -> Vec<usize> {
    use objc2_app_kit::{NSTextLayoutFragment, NSTextLayoutFragmentEnumerationOptions};
    let (Some(layout), Some(content)) = (view.textLayoutManager(), view.textContentStorage()) else {
        return vec![];
    };
    let start = content.documentRange().location();
    let lines = std::cell::RefCell::new(vec![]);
    let block = block2::RcBlock::new(
        |fragment: std::ptr::NonNull<NSTextLayoutFragment>| -> objc2::runtime::Bool {
            // SAFETY: TextKit keeps the fragment alive for the call.
            let fragment = unsafe { fragment.as_ref() };
            let offset = content.offsetFromLocation_toLocation(&start, &fragment.rangeInElement().location());
            if let Ok(offset) = usize::try_from(offset) {
                lines.borrow_mut().push(line_of(view, offset));
            }
            objc2::runtime::Bool::YES
        },
    );
    let _ = layout.enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
        Some(&start),
        NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
        &block,
    );
    drop(block);
    lines.into_inner()
}

/// How many folds are folded, for the self-test.
#[cfg(feature = "selftest")]
pub fn folded(view: &EditorTextView) -> usize {
    view.ivars().folds.borrow().hidden.len()
}
