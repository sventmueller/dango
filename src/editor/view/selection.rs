//! What follows a move of the selection (SN-T6): what shows at the caret at once, what only reads where it is once per
//! frame, however often it moved in between.

use ide_process::signpost::name;
use objc2::DefinedClass;

use super::EditorTextView;
use super::system::step;
use crate::editor::{blame, insets, multi, predict};

impl EditorTextView {
    /// The selection moved and settled.
    pub(super) fn selection_moved(&self) {
        step(name!("selection: typing insets"), || insets::strip_typing(self));
        step(name!("selection: found match"), || self.clear_found());
        step(name!("selection: carets"), || multi::selection_changed(self));
        step(name!("selection: prediction"), || predict::moved(self));
        step(name!("selection: navigation"), || crate::navigation::moved(self, false));
        if !self.ivars().selection_due.replace(true) {
            self.soon(objc2::sel!(ideSelectionSettled));
        }
    }

    /// What reads the selection once per frame: the blame at the line's end and the language server's breadcrumbs,
    /// code actions and signature, each of which waits for a pause anyway; a command that asks the server about the
    /// cursor calls it first, so the late move does not cancel its question.
    pub(super) fn selection_settled(&self) {
        if !self.ivars().selection_due.replace(false) {
            return;
        }
        step(name!("selection: blame"), || blame::cursor_moved(self));
        step(name!("selection: language server"), || {
            ide_ui_lsp::selection_moved(self)
        });
    }
}
