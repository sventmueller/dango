//! What the server says about the symbol under the resting mouse, or at the cursor on request, in a popover; the
//! same popover shows a diagnostic's message when the cursor jumps to it.

use std::ops::Range;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSEvent, NSLineBreakMode, NSPopover, NSPopoverBehavior, NSScrollView, NSTextField, NSTextInputClient, NSView,
    NSViewController,
};
use objc2_foundation::{NSPoint, NSRange, NSRect, NSRectEdge, NSSize};

use super::{server_of, text};
use crate::lsp::editor::Editor;
use crate::lsp::host as editor;
use ide_ui_common::util::main_soon;

/// How long the mouse rests before the server is asked.
const REST: f64 = 0.6;
/// The widest and tallest a popover grows before its text scrolls.
const MAX_WIDTH: f64 = 520.0;
const MAX_HEIGHT: f64 = 340.0;
const PADDING: f64 = 10.0;

#[derive(Default)]
pub struct State {
    /// Where the mouse rests, in the view's coordinates.
    rest: Option<NSPoint>,
    /// Counts questions, so that an answer to an older one is dropped.
    asked: u64,
    popover: Option<Retained<NSPopover>>,
    /// The area the shown popover is about, in the view's coordinates; leaving it closes the popover.
    about: Option<NSRect>,
    /// The text shown, for the self-test.
    shown: String,
}

/// The rectangle of `range`'s first line in `view`'s coordinates; TextKit 2 answers this for input methods.
pub fn rect_of(view: &dyn Editor, range: Range<usize>) -> Option<NSRect> {
    let window = view.window()?;
    // SAFETY: A range within the text; no actual range is asked for.
    let screen = unsafe {
        view.firstRectForCharacterRange_actualRange(
            NSRange::new(range.start, range.end - range.start),
            std::ptr::null_mut(),
        )
    };
    let in_window = window.convertRectFromScreen(screen);
    Some(view.convertRect_fromView(in_window, None))
}

/// The character under `point` of the view, if the point is on one.
fn character_at(view: &dyn Editor, point: NSPoint) -> Option<usize> {
    let length = view.document().text().len_utf16();
    let index = view.characterIndexForInsertionAtPoint(point);
    [index, index.wrapping_sub(1)].into_iter().find(|&candidate| {
        candidate < length
            && rect_of(view, candidate..candidate + 1).is_some_and(|r| {
                point.x >= r.origin.x - 1.0
                    && point.x <= r.origin.x + r.size.width + 1.0
                    && point.y >= r.origin.y
                    && point.y <= r.origin.y + r.size.height
            })
    })
}

/// The mouse moved over `view`: a popover it left closes, and a pause will ask the server.
pub fn mouse_moved(view: &dyn Editor, event: &NSEvent) {
    let point = view.convertPoint_fromView(event.locationInWindow(), None);
    let leave = {
        let mut state = view.lsp().borrow_mut();
        let moved = state
            .hover
            .rest
            .is_none_or(|rest| (rest.x - point.x).abs() > 2.0 || (rest.y - point.y).abs() > 2.0);
        if moved {
            state.hover.asked += 1;
            state.hover.rest = Some(point);
        }
        state.hover.about.is_some_and(|about| {
            let near = NSRect::new(
                NSPoint::new(about.origin.x - 6.0, about.origin.y - 6.0),
                NSSize::new(about.size.width + 12.0, about.size.height + 12.0),
            );
            !(point.x >= near.origin.x
                && point.x <= near.origin.x + near.size.width
                && point.y >= near.origin.y
                && point.y <= near.origin.y + near.size.height)
        })
    };
    if leave {
        dismiss(view);
    }
    if server_of(view).is_some() || view.debugger_paused() {
        view.later(sel!(ideHoverRest), REST);
    }
}

/// The mouse rested: asks about the character under it.
pub fn hover_rest(view: &dyn Editor) {
    let Some(point) = view.lsp().borrow().hover.rest else {
        return;
    };
    let shown = view.lsp().borrow().hover.popover.as_ref().is_some_and(|p| p.isShown());
    if shown {
        return;
    }
    if let Some(index) = character_at(view, point)
        && !view.debugger_hover(index)
    {
        ask(view, index);
    }
}

/// Asks about the symbol at the cursor, as the menu command does.
pub fn hover_at_cursor(view: &dyn Editor) {
    let index = view.selectedRange().location;
    view.lsp().borrow_mut().hover.rest = None;
    ask(view, index);
}

fn ask(view: &dyn Editor, index: usize) {
    let Some((server, uri)) = server_of(view) else { return };
    let Some(position) = text::position(view.document().text(), index) else {
        return;
    };
    let (serial, asked) = {
        let mut state = view.lsp().borrow_mut();
        state.hover.asked += 1;
        (state.serial, state.hover.asked)
    };
    let _ = server.hover(&uri, position, move |answer| {
        let Ok(Some(hover)) = answer else { return };
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            if view.lsp().borrow().hover.asked != asked || hover.markdown.trim().is_empty() {
                return;
            }
            let range = hover
                .range
                .map_or(index..index + 1, |r| text::range(view.document().text(), r));
            show(&*view, range, &hover.markdown);
        });
    });
}

/// Shows `markdown` in a popover at `range`.
pub fn show(view: &dyn Editor, range: Range<usize>, markdown: &str) {
    dismiss(view);
    let Some(rect) = rect_of(view, range.start..range.end.max(range.start + 1)) else {
        return;
    };
    let mtm = view.mtm();
    let content = content(mtm, markdown);
    let controller = NSViewController::new(mtm);
    let size = content.frame().size;
    controller.setView(&content);
    let popover = NSPopover::new(mtm);
    popover.setBehavior(NSPopoverBehavior::Transient);
    popover.setAnimates(false);
    popover.setContentViewController(Some(&controller));
    popover.setContentSize(size);
    popover.showRelativeToRect_ofView_preferredEdge(rect, view, NSRectEdge::MaxY);
    let mut state = view.lsp().borrow_mut();
    state.hover.popover = Some(popover);
    state.hover.about = Some(rect);
    markdown.clone_into(&mut state.hover.shown);
}

/// The popover's content: the styled text, scrolling when it is long.
fn content(mtm: MainThreadMarker, markdown: &str) -> Retained<NSView> {
    let field = NSTextField::labelWithAttributedString(&super::markdown::attributed(markdown, 12.0), mtm);
    field.setSelectable(true);
    field.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    field.setPreferredMaxLayoutWidth(MAX_WIDTH);
    let fitted = field.cell().map_or(NSSize::new(MAX_WIDTH, 40.0), |cell| {
        cell.cellSizeForBounds(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(MAX_WIDTH, 1.0e6)))
    });
    let width = fitted.width.min(MAX_WIDTH).ceil();
    let height = fitted.height.ceil();
    let shown = height.min(MAX_HEIGHT);
    let container = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width + 2.0 * PADDING, shown + 2.0 * PADDING),
        ),
    );
    let inner = NSRect::new(NSPoint::new(PADDING, PADDING), NSSize::new(width, shown));
    if height > MAX_HEIGHT {
        field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)));
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), inner);
        scroll.setHasVerticalScroller(true);
        scroll.setDrawsBackground(false);
        scroll.setDocumentView(Some(&field));
        container.addSubview(&scroll);
        if let Some(document) = scroll.documentView() {
            document.scrollPoint(NSPoint::new(0.0, height));
        }
    } else {
        field.setFrame(inner);
        container.addSubview(&field);
    }
    container
}

/// Closes the popover of `view`, if one is shown.
pub fn dismiss(view: &dyn Editor) {
    let popover = {
        let mut state = view.lsp().borrow_mut();
        state.hover.about = None;
        state.hover.popover.take()
    };
    if let Some(popover) = popover {
        popover.close();
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
