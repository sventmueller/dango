//! The editor as the language servers reach it (`crate::lsp::editor::Editor`), and their inlay hints as its insets.

use std::cell::{Ref, RefCell, RefMut};
use std::ops::{Deref, Range};
use std::path::PathBuf;
use std::rc::Rc;

use ide_document::{Change, Document};
use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2_app_kit::{NSStackView, NSTextStorage, NSTextView};

use super::EditorTextView;
use super::insets::{self, Inset, Layer, Place};
use crate::lsp::ViewState;
use crate::lsp::editor::{Editor, Hint};

/// `view` as a language server's editor that it holds on to.
pub fn lsp_editor(view: Retained<EditorTextView>) -> Box<dyn Editor> {
    Box::new(Held(view))
}

/// An editor held for a language server.
struct Held(Retained<EditorTextView>);

impl Deref for Held {
    type Target = NSTextView;

    fn deref(&self) -> &NSTextView {
        &self.0
    }
}

impl Editor for EditorTextView {
    fn serial(&self) -> u64 {
        self.ivars().serial
    }

    fn lsp(&self) -> &RefCell<ViewState> {
        &self.ivars().lsp
    }

    fn document(&self) -> Ref<'_, Document> {
        EditorTextView::document(self)
    }

    fn document_mut(&self) -> RefMut<'_, Document> {
        EditorTextView::document_mut(self)
    }

    fn project_root(&self) -> Option<PathBuf> {
        self.controller().and_then(|c| c.project_root())
    }

    fn window_id(&self) -> Option<u64> {
        self.controller().map(|c| c.id())
    }

    fn later(&self, selector: Sel, delay: f64) {
        EditorTextView::later(self, selector, delay);
    }

    fn storage(&self) -> Option<Retained<NSTextStorage>> {
        EditorTextView::storage(self)
    }

    fn apply_model_changes(&self, changes: &[Change]) {
        EditorTextView::apply_model_changes(self, changes);
    }

    fn paint_found(&self) {
        EditorTextView::paint_found(self);
    }

    fn redraw_gutter(&self) {
        super::redraw_gutter(self);
    }

    fn diagnostics_changed(&self) {
        super::diagnostics_changed(self);
    }

    fn redraw_minimap(&self) {
        super::redraw_minimap(self);
    }

    fn visible_utf16(&self) -> Range<usize> {
        super::visible_utf16(self)
    }

    fn hidden_by_fold(&self, offset: usize) -> bool {
        super::hidden_by_fold(self, offset)
    }

    fn set_server_folds(&self, ranges: Option<Vec<(usize, usize)>>) {
        super::set_server_folds(self, ranges);
    }

    fn save_soon(&self) {
        super::save_soon(self, false);
    }

    fn path_changed(&self) {
        super::path_changed(self);
    }

    fn formatting_options(&self) -> (u32, bool) {
        super::formatting_options(self)
    }

    fn follow_overlay(&self) {
        super::follow_overlay(self);
    }

    fn status_items(&self, stack: &NSStackView) {
        super::status_items(objc2::MainThreadOnly::mtm(self), self, stack);
    }

    fn set_inlay_hints(&self, hints: Vec<Hint>, kept: Option<Range<usize>>) {
        let mut shown: Vec<Inset> = hints.into_iter().map(inset).collect();
        if let Some(kept) = kept {
            shown.extend(
                insets::of(self, Layer::Inlay)
                    .into_iter()
                    .filter(|old| !kept.contains(&old.at)),
            );
        }
        insets::set(self, Layer::Inlay, shown);
    }

    fn clear_inlay_hints(&self) {
        insets::clear(self, Layer::Inlay);
    }

    fn debugger_paused(&self) -> bool {
        crate::debug::paused_in(self)
    }

    fn debugger_hover(&self, index: usize) -> bool {
        crate::debug::hover(self, index)
    }
}

/// What one hint shows, says on hover and does on a click.
fn inset(hint: Hint) -> Inset {
    let mut inset = Inset::new(hint.at, hint.text, Place::Inline);
    inset.tooltip = hint.tooltip;
    inset.pad_left = hint.pad_left;
    inset.pad_right = hint.pad_right;
    if let Some((path, range)) = hint.target {
        inset.action = Some(Rc::new(move |view: &EditorTextView| {
            if let Some(window) = Editor::window_id(view) {
                crate::lsp::open_at(window, &path, range);
            }
        }));
    }
    inset
}

impl Editor for Held {
    fn serial(&self) -> u64 {
        Editor::serial(&*self.0)
    }

    fn lsp(&self) -> &RefCell<ViewState> {
        Editor::lsp(&*self.0)
    }

    fn document(&self) -> Ref<'_, Document> {
        self.0.document()
    }

    fn document_mut(&self) -> RefMut<'_, Document> {
        self.0.document_mut()
    }

    fn project_root(&self) -> Option<PathBuf> {
        Editor::project_root(&*self.0)
    }

    fn window_id(&self) -> Option<u64> {
        Editor::window_id(&*self.0)
    }

    fn later(&self, selector: Sel, delay: f64) {
        self.0.later(selector, delay);
    }

    fn storage(&self) -> Option<Retained<NSTextStorage>> {
        self.0.storage()
    }

    fn apply_model_changes(&self, changes: &[Change]) {
        self.0.apply_model_changes(changes);
    }

    fn paint_found(&self) {
        self.0.paint_found();
    }

    fn redraw_gutter(&self) {
        Editor::redraw_gutter(&*self.0);
    }

    fn diagnostics_changed(&self) {
        Editor::diagnostics_changed(&*self.0);
    }

    fn redraw_minimap(&self) {
        Editor::redraw_minimap(&*self.0);
    }

    fn visible_utf16(&self) -> Range<usize> {
        Editor::visible_utf16(&*self.0)
    }

    fn hidden_by_fold(&self, offset: usize) -> bool {
        Editor::hidden_by_fold(&*self.0, offset)
    }

    fn set_server_folds(&self, ranges: Option<Vec<(usize, usize)>>) {
        Editor::set_server_folds(&*self.0, ranges);
    }

    fn save_soon(&self) {
        Editor::save_soon(&*self.0);
    }

    fn path_changed(&self) {
        Editor::path_changed(&*self.0);
    }

    fn formatting_options(&self) -> (u32, bool) {
        Editor::formatting_options(&*self.0)
    }

    fn follow_overlay(&self) {
        Editor::follow_overlay(&*self.0);
    }

    fn status_items(&self, stack: &NSStackView) {
        Editor::status_items(&*self.0, stack);
    }

    fn set_inlay_hints(&self, hints: Vec<Hint>, kept: Option<Range<usize>>) {
        Editor::set_inlay_hints(&*self.0, hints, kept);
    }

    fn clear_inlay_hints(&self) {
        Editor::clear_inlay_hints(&*self.0);
    }

    fn debugger_paused(&self) -> bool {
        Editor::debugger_paused(&*self.0)
    }

    fn debugger_hover(&self, index: usize) -> bool {
        Editor::debugger_hover(&*self.0, index)
    }
}
