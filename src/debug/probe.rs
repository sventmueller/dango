//! What the self-test reads from and does with a window's debugger: `ide_ui_debug`'s probe, and the marks, values
//! and rotor the app's editors carry.

use ide_ui_debug::model::breakpoints::Look;
pub use ide_ui_debug::selftest::*;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::NSAccessibilityCustomRotor;
use objc2_foundation::NSArray;

use crate::editor::EditorTextView;

/// The breakpoint of `line` of `editor` and whether the program is paused there, as the gutter draws them.
pub fn gutter(editor: &EditorTextView, line: usize) -> (Option<Look>, bool) {
    super::editor::gutter_mark(editor, line)
}

/// The values at the line ends of `editor`.
pub fn inline_values(editor: &EditorTextView) -> Vec<String> {
    crate::editor::insets::of(editor, crate::editor::insets::Layer::Debug)
        .into_iter()
        .map(|inset| inset.text)
        .collect()
}

/// The labels of `editor`'s rotors and the entries of its "Haltepunkte" rotor.
pub fn rotors(editor: &EditorTextView) -> (Vec<String>, Vec<String>) {
    // SAFETY: The getter takes no arguments and may answer nil, which the `Option` result type allows for.
    let own: Option<Retained<NSArray<NSAccessibilityCustomRotor>>> =
        unsafe { msg_send![editor, accessibilityCustomRotors] };
    let labels = own
        .iter()
        .flat_map(|r| r.to_vec())
        .map(|r| r.label().to_string())
        .collect();
    let items = super::rotor::items(editor)
        .into_iter()
        .map(|(_, label)| label)
        .collect();
    (labels, items)
}

/// What the hover popover would say about the expression at UTF-16 offset `index` of `editor`.
pub fn expression_at(editor: &EditorTextView, index: usize) -> Option<String> {
    super::editor::expression_at(editor, index).map(|(text, _)| text)
}

/// Asks the hover for `index` of `editor` as a resting mouse does; whether the debugger took it.
pub fn hover(editor: &EditorTextView, index: usize) -> bool {
    super::editor::hover(editor, index)
}
