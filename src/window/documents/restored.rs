//! Clean file tabs restored from their paths. Only the documents in front need an editor.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dispatch2::MainThreadBound;
use ide_document::{Document, recovery::RecoveryId};
use objc2::rc::Weak;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSTextField, NSView};
use splat_i18n::tf;

use crate::editor::EditorTextView;
use crate::util::{FlagGuard, activate, main_after, main_soon, ns};
use crate::window::{TabId, TabKind, WindowController, tabs::TabWindow};

type Ready = Box<dyn FnOnce(&EditorTextView)>;

pub(super) struct File {
    path: PathBuf,
    recovery: RecoveryId,
    font_size: f64,
    loading: bool,
    preview: bool,
    ready: Vec<Ready>,
}

#[derive(Default)]
pub(super) struct Editors {
    restoring: Cell<bool>,
    pub(super) pending: RefCell<HashMap<TabId, File>>,
}

impl WindowController {
    /// Brings back the complete layout before selecting which clean files to load.
    pub(crate) fn restore_editor_tabs(&self, restore: impl FnOnce()) {
        let before = self.current_tab();
        {
            let _restoring = FlagGuard::raise(&self.ivars().documents.restored.restoring);
            restore();
        }
        if let Some(tab) = self.current_tab() {
            *self.ivars().documents.current.borrow_mut() = before;
            self.select(&tab);
        }
    }

    /// Whether the window is putting back the documents of its folder's layout or of the last run.
    pub(crate) fn restoring_editor_tabs(&self) -> bool {
        self.ivars().documents.restored.restoring.get()
    }

    pub(crate) fn restore_editor(&self, path: &Path, font_size: f64) -> Option<TabId> {
        if !path.is_file() {
            return None;
        }
        let title = crate::editor::tab_title(path);
        let view = NSView::new(self.mtm());
        let id = self.add_tab(TabKind::Editor, Some(path.to_string_lossy().into_owned()), &title, view);
        self.ivars().documents.restored.pending.borrow_mut().insert(
            id,
            File {
                path: path.to_owned(),
                recovery: RecoveryId::new(),
                font_size,
                loading: false,
                preview: false,
                ready: Vec::new(),
            },
        );
        Some(id)
    }

    pub(crate) fn restored_editor_record(&self, id: TabId) -> Option<(Option<PathBuf>, RecoveryId)> {
        let pending = self.ivars().documents.restored.pending.borrow();
        let file = pending.get(&id)?;
        Some((Some(file.path.clone()), file.recovery.clone()))
    }

    pub(in crate::window) fn allow_restored_preview(&self, id: TabId) {
        if let Some(file) = self.ivars().documents.restored.pending.borrow_mut().get_mut(&id) {
            file.preview = true;
        }
    }

    pub(crate) fn when_restored_editor<F>(&self, path: &Path, ready: F) -> Result<(), F>
    where
        F: FnOnce(&EditorTextView) + 'static,
    {
        let Some(id) = self.tab_with_key(&path.to_string_lossy()) else {
            return Err(ready);
        };
        let mut pending = self.ivars().documents.restored.pending.borrow_mut();
        let Some(file) = pending.get_mut(&id) else {
            return Err(ready);
        };
        file.ready.push(Box::new(ready));
        Ok(())
    }

    pub(in crate::window) fn load_restored_editors(&self, tab: &TabWindow) {
        let restored = &self.ivars().documents.restored;
        if restored.restoring.get() || restored.pending.borrow().is_empty() {
            return;
        }
        let active = tab.panes.active();
        for pane in std::iter::once(active).chain(tab.panes.all().into_iter().filter(|&pane| pane != active)) {
            if let Some(document) = tab.panes.front(pane) {
                self.load_restored_editor(document.id);
            }
        }
    }

    fn load_restored_editor(&self, id: TabId) {
        let path = {
            let mut pending = self.ivars().documents.restored.pending.borrow_mut();
            let Some(file) = pending.get_mut(&id).filter(|file| !file.loading) else {
                return;
            };
            file.loading = true;
            file.path.clone()
        };
        let window = MainThreadBound::new(Weak::from(self), self.mtm());
        let queued = ide_core::lane::DISK.ask(
            move || Document::open(&path),
            move |result| {
                main_soon(move || {
                    if let Some(controller) = MainThreadMarker::new().and_then(|mtm| window.get(mtm).load()) {
                        controller.finish_restored_editor(id, result);
                    }
                });
            },
        );
        if queued.is_err() {
            self.finish_restored_editor(id, None);
        }
    }

    fn finish_restored_editor(&self, id: TabId, result: Option<std::io::Result<Document>>) {
        let Some(result) = result else {
            if let Some(file) = self.ivars().documents.restored.pending.borrow_mut().get_mut(&id) {
                file.loading = false;
            }
            let window = MainThreadBound::new(Weak::from(self), self.mtm());
            main_after(Duration::from_millis(50), move || {
                if let Some(controller) = MainThreadMarker::new().and_then(|mtm| window.get(mtm).load())
                    && let Some(tab) = controller.current_tab()
                {
                    controller.load_restored_editors(&tab);
                }
            });
            return;
        };
        let Some((tab, document)) = self.find_document(id) else {
            return;
        };
        let front = self
            .current_tab()
            .is_some_and(|current| std::rc::Rc::ptr_eq(&current, &tab))
            && tab
                .panes
                .of_document(id)
                .and_then(|pane| tab.panes.front(pane))
                .is_some_and(|doc| doc.id == id);
        let file = {
            let mut pending = self.ivars().documents.restored.pending.borrow_mut();
            if !front && let Some(file) = pending.get_mut(&id).filter(|file| file.ready.is_empty()) {
                file.loading = false;
                return;
            }
            pending.remove(&id)
        };
        let Some(file) = file else { return };
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(error) => {
                crate::diag::report("reopening a file", &error);
                let label = NSTextField::labelWithString(
                    &ns(&tf(
                        "„{}“ lässt sich nicht öffnen.",
                        &[&crate::editor::display_name(&file.path)],
                    )),
                    self.mtm(),
                );
                label.setTranslatesAutoresizingMaskIntoConstraints(false);
                document.view.addSubview(&label);
                activate([
                    label
                        .centerXAnchor()
                        .constraintEqualToAnchor(&document.view.centerXAnchor()),
                    label
                        .centerYAnchor()
                        .constraintEqualToAnchor(&document.view.centerYAnchor()),
                ]);
                return;
            }
        };
        let view = crate::editor::attach_restored(self, id, &document.view, loaded, file.font_size, file.recovery);
        let focused = self.selected_tab() == Some(id);
        if focused
            && let Some(window) = view.window()
            && window
                .firstResponder()
                .and_then(|responder| responder.downcast::<NSView>().ok())
                .is_none_or(|focus| !focus.acceptsFirstResponder() || focus.isDescendantOf(&document.view))
        {
            window.makeFirstResponder(Some(&view));
        }
        if focused && file.preview && tab.documents().len() == 1 {
            crate::markdown::opened(&view);
        }
        for ready in file.ready {
            ready(&view);
        }
    }
}
