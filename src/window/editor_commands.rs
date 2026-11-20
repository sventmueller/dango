//! The editor's menu commands while the keyboard focus is elsewhere in the window, such as in the sidebar or on the
//! window itself: the window answers them for the editor in front of the active pane, whose own validation decides.

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{ClassType, Message, ProtocolType};
use objc2_app_kit::{NSResponder, NSTextInputClient, NSView};
use objc2_foundation::NSObjectProtocol;

use super::WindowController;
use crate::editor::EditorTextView;

impl WindowController {
    /// The editor that takes `action` for the window: the one in front of the active pane, unless the window answers
    /// `action` itself or the keyboard focus is in another view that takes typing.
    pub(super) fn editor_for_action(&self, action: Sel) -> Option<Retained<EditorTextView>> {
        if !is_own_command(action) || Self::class().responds_to(action) {
            return None;
        }
        if self
            .window()?
            .firstResponder()
            .is_some_and(|focus| takes_typing(&focus))
        {
            return None;
        }
        let document = self.current_tab()?.focused()?;
        if !matches!(document.kind, super::TabKind::Editor) || !EditorTextView::class().responds_to(action) {
            return None;
        }
        editor_in(&document.view)
    }
}

/// The editor somewhere in `view`, the container of an editor tab.
fn editor_in(view: &NSView) -> Option<Retained<EditorTextView>> {
    if let Ok(editor) = Retained::downcast::<EditorTextView>(view.retain()) {
        return Some(editor);
    }
    view.subviews().iter().find_map(|child| editor_in(&child))
}

/// Whether `action` is one of the app's own commands, which AppKit's text views do not answer.
fn is_own_command(action: Sel) -> bool {
    action.name().to_bytes().starts_with(b"ide")
}

/// Whether `responder` takes typed text, whose shortcuts must not reach the editor instead.
fn takes_typing(responder: &NSResponder) -> bool {
    <dyn NSTextInputClient>::protocol().is_some_and(|protocol| responder.conformsToProtocol(protocol))
}

#[cfg(test)]
mod tests {
    use objc2::sel;

    use super::is_own_command;

    #[test]
    fn only_the_apps_own_commands_go_to_the_editor() {
        assert!(is_own_command(sel!(ideMarkdown:)));
        assert!(is_own_command(sel!(ideToggleWrap:)));
        assert!(!is_own_command(sel!(copy:)));
        assert!(!is_own_command(sel!(performTextFinderAction:)));
    }
}
