//! What the self-test reads from and does with editors, through the code their commands run.

use std::path::Path;

use ide_document::recovery::RecoveryId;
use objc2::DefinedClass;
use objc2::rc::{Retained, Weak};
use objc2_app_kit::{NSColor, NSFontAttributeName};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::super::{EditorTextView, changes, compare, conflict, editing, editor_for, open_editors, save, style};
use crate::util::{constant, ns};
use crate::window::WindowController;

pub use super::super::line_diagnostics::busy_lines;
pub use super::super::minimap::{click as click_minimap, probe as minimap};
pub use super::super::sticky::{click as click_sticky, height as sticky_height, shown as sticky_shown};
pub use super::super::vim::{mode as vim_mode, shown as vim_shown, toggle as toggle_vim};

/// The logical lines `view` shows, the first to the last on screen, for the self-test.
pub fn visible_line_range(view: &EditorTextView) -> std::ops::Range<usize> {
    let rect = view.visibleRect();
    let top = super::super::highlight::line_at(view, rect.origin).unwrap_or(0);
    let bottom = super::super::highlight::line_at(view, NSPoint::new(0.0, rect.origin.y + rect.size.height - 1.0))
        .unwrap_or(top);
    top..bottom + 1
}

/// The editors of `controller` with their tab ids, for the self-test.
pub fn editors_of(controller: &WindowController) -> Vec<(isize, Retained<EditorTextView>)> {
    open_editors()
        .into_iter()
        .filter(|v| v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
        .map(|v| (v.ivars().tab.get(), v))
        .collect()
}

/// What the bar above `view` says and offers while it shows, for the self-test.
pub fn conflict_bar(view: &EditorTextView) -> Option<(String, Vec<String>)> {
    conflict::inspect(view)
}

/// The message, informative text and buttons with their keys of the question before the text of a gone file's editor
/// goes, for the self-test.
pub fn discard_question(view: &EditorTextView) -> (String, String, Vec<(String, String)>) {
    conflict::inspect_question(view)
}

/// Clicks the button titled `title` of the bar above `view`, for the self-test; whether there was one.
pub fn press_conflict_button(view: &EditorTextView, title: &str) -> bool {
    conflict::press(view, title)
}

/// The text of the tab with the differences for `path`, for the self-test.
pub fn compared(path: &Path) -> Option<String> {
    compare::shown(path)
}

/// The copy against a crash of `view`, for the self-test.
pub fn recovery_of(view: &EditorTextView) -> RecoveryId {
    view.ivars().recovery.clone()
}

/// The text view of the front editor of `controller`, for the self-test.
pub fn editor_of(controller: &WindowController, path: &Path) -> Option<Retained<EditorTextView>> {
    editor_for(controller, path)
}

/// What the self-test does with folds and carets, through the code the commands run.
pub mod selftest {
    pub use super::super::super::fold::{
        click as click_fold, fold_all, fold_at_cursor, folded, from_server, gutter, laid_out_lines, placeholders,
        toggle as toggle_fold, unfold_all,
    };
    pub use super::super::super::multi::{
        add_next_occurrence, add_vertical, carets, column, copied, paste_text, set as set_carets,
    };
}

/// What the self-test does with blame, through the code the commands run.
pub mod blame_probe {
    pub use super::super::super::blame::{dismiss, line, popover_texts, show_for_cursor};
    pub use super::super::super::blame_column::of as column;
}

/// Types `text` as the keyboard would, through the view's input path.
pub fn type_text(view: &EditorTextView, text: &str) {
    use objc2_app_kit::NSTextInputClient;
    // SAFETY: The inserted object is an NSString, one of the two types this method accepts.
    unsafe {
        view.insertText_replacementRange(
            &ns(text),
            objc2_foundation::NSRange::new(objc2_foundation::NSNotFound.cast_unsigned(), 0),
        );
    }
}

/// Presses Return in `view`.
pub fn type_newline(view: &EditorTextView) {
    editing::newline(view);
}

/// Saves `view` and waits for the file to be written.
pub fn save_view(view: &EditorTextView) {
    save(view, false);
}

/// Whether a save of `view` still writes its file on the disk lane.
pub fn writing(view: &EditorTextView) -> bool {
    view.ivars().writing.get() != super::super::saving::Writing::Idle
}

pub use super::super::changes::{
    Mark as ChangeMark, Seen as ChangesSeen, popover_parts as change_popover, seen as changes_seen,
};
pub use super::super::indent::{menu as indent_menu, shown as indent_shown};
pub use super::super::whitespace::visible as whitespace_visible;

/// Clicks the change mark of `line`, counted from 0, in `view`'s line number column, as the mouse would.
pub fn click_change(view: &EditorTextView, line: usize) -> bool {
    let Some(gutter) = view.ivars().gutter.borrow().as_ref().and_then(Weak::load) else {
        return false;
    };
    let Some(start) = view.line_start(line) else {
        return false;
    };
    let Some(row) = ide_ui_lsp::rect_of(view, start..start + 1) else {
        return false;
    };
    let row = gutter.convertRect_fromView(row, Some(view));
    let width = gutter.bounds().size.width;
    let at = NSRect::new(
        NSPoint::new(width - 8.0, row.origin.y),
        NSSize::new(8.0, row.size.height),
    );
    changes::clicked(view, line, &gutter, at)
}

/// Whether the first `fn` of `text` is shown in a keyword colour.
pub fn keyword_coloured(view: &EditorTextView, text: &str) -> bool {
    let Some(at) = text.find("fn ") else { return false };
    coloured_as(
        view,
        text[..at].encode_utf16().count(),
        splat_markdown::code::CodeToken::Keyword,
    )
}

/// Whether the character at UTF-16 offset `at` is shown in the colour of `token`.
pub fn coloured_as(view: &EditorTextView, at: usize, token: splat_markdown::code::CodeToken) -> bool {
    if view.storage().is_none_or(|storage| at >= storage.length()) {
        return false;
    }
    let expected: Retained<objc2::runtime::AnyObject> = style::token_color(token).into();
    super::super::highlight::shown_color(view, at).is_some_and(|c| std::ptr::eq(&*c, &*expected))
}

/// The colour at UTF-16 offset `at` in sRGB as the view's appearance shows it, and whether the font there is bold.
pub fn styled_at(view: &EditorTextView, at: usize) -> Option<([f64; 3], bool)> {
    use objc2_app_kit::{NSAppearanceCustomization, NSColorSpace, NSFont, NSFontDescriptorSymbolicTraits};
    let storage = view.storage()?;
    let color = super::super::highlight::shown_color(view, at)?;
    // SAFETY: The index lies inside the text, and a null pointer for the effective range, which the method allows.
    let font =
        unsafe { storage.attribute_atIndex_effectiveRange(constant!(NSFontAttributeName), at, std::ptr::null_mut()) }?;
    let color = color.downcast::<NSColor>().ok()?;
    let rgb = std::cell::Cell::new(None);
    view.effectiveAppearance()
        .performAsCurrentDrawingAppearance(&block2::RcBlock::new(|| {
            crate::util::guarded((), || {
                rgb.set(
                    color
                        .colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())
                        .map(|c| [c.redComponent(), c.greenComponent(), c.blueComponent()]),
                );
            })
        }));
    let bold = font
        .downcast::<NSFont>()
        .ok()?
        .fontDescriptor()
        .symbolicTraits()
        .contains(NSFontDescriptorSymbolicTraits::TraitBold);
    Some((rgb.get()?, bold))
}

/// Sends a mouse event of `kind` to the line number column of `view` beside `line`, counted from 0, `from_right`
/// points left of the column's right edge, as the mouse would; for the self-test. Gives the titles of the menu a right
/// click gets, and whether the mouse is over the column then.
pub fn gutter_mouse(
    view: &EditorTextView,
    line: usize,
    from_right: f64,
    kind: objc2_app_kit::NSEventType,
) -> Option<(Vec<String>, bool)> {
    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
    let gutter = view.ivars().gutter.borrow().as_ref().and_then(Weak::load)?;
    let start = view.line_start(line)?;
    let row = gutter.convertRect_fromView(ide_ui_lsp::rect_of(view, start..start + 1)?, Some(view));
    let at = NSPoint::new(
        gutter.bounds().size.width - from_right,
        row.origin.y + row.size.height / 2.0,
    );
    gutter.displayIfNeeded();
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        if kind == NSEventType::RightMouseDown { kind } else { NSEventType::LeftMouseDown },
        gutter.convertPoint_toView(at, None),
        NSEventModifierFlags::empty(),
        0.0,
        view.window()?.windowNumber(),
        None,
        0,
        1,
        0.0,
    )?;
    let mut titles = Vec::new();
    match kind {
        NSEventType::RightMouseDown => {
            let menu = gutter.menuForEvent(&event);
            titles.extend(
                menu.iter()
                    .flat_map(|m| m.itemArray().iter().map(|i| i.title().to_string()).collect::<Vec<_>>()),
            );
        }
        // SAFETY: The column's own handlers of these events, each given a mouse event in its window.
        NSEventType::MouseEntered => unsafe { objc2::msg_send![&*gutter, mouseEntered: &*event] },
        // SAFETY: `mouseExited:` of the column itself, given a mouse event of its own window.
        NSEventType::MouseExited => unsafe { objc2::msg_send![&*gutter, mouseExited: &*event] },
        // SAFETY: `mouseDown:` of the column itself, given a mouse event of its own window.
        _ => unsafe { objc2::msg_send![&*gutter, mouseDown: &*event] },
    }
    gutter.displayIfNeeded();
    Some((titles, gutter.hovered()))
}
