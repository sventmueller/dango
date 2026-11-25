//! The document model and the view in step: every edit of the view goes into the model, every change of the model
//! into the view, and what follows the text hears of both; after a panic in a callback the editor puts all of it back
//! in step (Review B9).

use ide_document::{Change, Edit};
use objc2::DefinedClass;
use objc2::rc::Weak;
use objc2_app_kit::NSTextStorage;
use objc2_foundation::NSRange;

use ide_process::signpost::name;
use objc2::sel;

use super::EditorTextView;
use super::system::step;
use crate::editor::insets::Layer;
use crate::editor::resync::Step;
use crate::editor::{
    blame, changes, conflict, file, fold, highlight, insets, line_diagnostics, long_lines, minimap, multi,
    notify_change, predict, recovery, undo_step, vim, whitespace,
};
use crate::util::{FlagGuard, ns};

impl EditorTextView {
    /// What an AppKit callback of the editor does, with `fallback` if a bug in it panics; the editor then puts itself
    /// back in step on the next turn of the run loop.
    pub(super) fn guarded<R>(&self, fallback: R, f: impl FnOnce() -> R) -> R {
        self.ivars().recovering.guarded(fallback, f, || self.ask_resync())
    }

    fn ask_resync(&self) {
        self.later(objc2::sel!(ideResync), 0.0);
    }

    /// Puts the view, its document and everything that follows them back in step, if a panic asked for it.
    pub(super) fn resync_due(&self) {
        if crate::editor::find_editor(self.ivars().serial).is_none() {
            return;
        }
        let failed = self.ivars().recovering.resync(
            || self.string().to_string() != self.document().text().to_string(),
            |step| self.resync_step(step),
        );
        if !failed.is_empty() {
            crate::diag::report("putting an editor back in step", format!("{failed:?} panicked"));
        }
    }

    fn resync_step(&self, step: Step) {
        match step {
            Step::ShowDocument => self.show_document(),
            Step::ResendText => ide_ui_lsp::opened(self),
            Step::Recolor => highlight::recolor_all(self),
            Step::DropInsets => self.drop_insets(),
            Step::ShowDirty => file::show_dirty_anew(self),
            Step::Follow => self.follow_anew(),
        }
    }

    /// Puts the document's whole text into the view, without the folds and further carets that sat on the old one.
    fn show_document(&self) {
        multi::clear(self);
        fold::unfold_all(self);
        let text = self.document().text().to_string();
        let _applying = FlagGuard::raise(&self.ivars().applying);
        self.setString(&ns(&text));
    }

    /// Takes away the insets that may sit on the wrong text; each comes back from where it came from.
    fn drop_insets(&self) {
        predict::forget(self);
        for layer in [Layer::Inlay, Layer::Diagnostics, Layer::Blame] {
            insets::clear(self, layer);
        }
        insets::reset(self);
        line_diagnostics::refresh(self);
        blame::cursor_moved(self);
    }

    /// What follows the text takes it from the start: the crash copy, the conflict count, the change marks, Vim, the
    /// previews, a project search and the views beside the text.
    fn follow_anew(&self) {
        recovery::changed(self);
        conflict::text_changed(self);
        changes::schedule(self);
        vim::changed(self);
        crate::preview::edited(self);
        if let Some(path) = self.document().path() {
            crate::window::buffer_edited(path);
        }
        minimap::redraw(self);
        self.later(objc2::sel!(ideStickyRefresh), 0.0);
        if let Some(gutter) = self.ivars().around.gutter.borrow().as_ref().and_then(Weak::load) {
            gutter.fit();
        }
        fold::relayout(self);
    }

    pub(super) fn follow_edit(&self, storage: &NSTextStorage, range: NSRange, delta: isize) {
        let old_length = usize::try_from(isize::try_from(range.length).unwrap_or(0) - delta).unwrap_or(0);
        let text = storage.string().substringWithRange(range).to_string();
        let edit = Edit {
            range_utf16: range.location..range.location + old_length,
            text,
        };
        let change = self.ivars().document.borrow_mut().apply(edit);
        match change {
            Ok(change) => self.changed(&change),
            Err(error) => {
                crate::diag::report("editor out of step with its document", format!("{error:?}"));
                self.ivars().recovering.ask(|| self.ask_resync());
            }
        }
    }

    /// What follows an edit of the document, from the view or from the model.
    pub(in crate::editor) fn changed(&self, change: &Change) {
        self.changed_all(std::slice::from_ref(change));
    }

    /// What follows edits of the document: each moves what sits on the text and goes to the language servers, and the
    /// work they cause runs once for all of them.
    fn changed_all(&self, changes: &[Change]) {
        if changes.is_empty() {
            return;
        }
        let mut lines = false;
        for change in changes {
            step(name!("edit: highlight"), || highlight::edited(self, change));
            step(name!("edit: folds"), || fold::shift(self, change));
            step(name!("edit: carets"), || multi::shift(self, change));
            step(name!("edit: insets"), || insets::shift(self, change));
            step(name!("edit: notify"), || notify_change(self, change));
            lines |= change.start.line != change.new_end.line || change.start.line != change.old_end.line;
        }
        step(name!("edit: schedule highlight"), || highlight::schedule(self));
        step(name!("edit: recovery"), || recovery::changed(self));
        step(name!("edit: conflict"), || conflict::text_changed(self));
        step(name!("edit: dirty"), || file::update_dirty(self));
        step(name!("edit: change marks"), || changes::schedule(self));
        step(name!("edit: whitespace"), || whitespace::redraw(self));
        step(name!("edit: minimap"), || minimap::redraw(self));
        // It reads the layout, which TextKit forbids while the edit is still under way; an edit within lines keeps the
        // lines of the blocks it found.
        let sticky = if lines {
            sel!(ideStickyRefresh)
        } else {
            sel!(ideStickyEdited)
        };
        self.later(sticky, 0.0);
        step(name!("edit: line diagnostics"), || {
            line_diagnostics::edited(self, changes)
        });
        step(name!("edit: vim"), || vim::changed(self));
        step(name!("edit: long lines"), || long_lines::edited(self, changes));
        if lines && let Some(gutter) = self.ivars().around.gutter.borrow().as_ref().and_then(Weak::load) {
            step(name!("edit: gutter"), || {
                gutter.fit();
                gutter.redraw();
            });
        }
    }

    /// Makes `changes` of the model in the view, as one undoable step.
    pub(crate) fn apply_model_changes(&self, changes: &[Change]) {
        vim::before_model_change(self);
        self.apply_changes(changes);
    }

    /// Makes a change Vim made in the view, as an undo step of its own or of the insert pass that is open.
    pub(crate) fn apply_vim_change(&self, change: &Change) {
        self.apply_changes(std::slice::from_ref(change));
    }

    fn apply_changes(&self, changes: &[Change]) {
        let Some(storage) = self.storage() else { return };
        {
            let _applying = FlagGuard::raise(&self.ivars().applying);
            undo_step(self, || {
                for change in changes {
                    let range = NSRange::new(change.utf16.start, change.utf16.end - change.utf16.start);
                    if self.shouldChangeTextInRange_replacementString(range, Some(&ns(&change.text))) {
                        storage.replaceCharactersInRange_withString(range, &ns(&change.text));
                        self.didChangeText();
                    }
                }
            });
        }
        self.changed_all(changes);
    }
}
