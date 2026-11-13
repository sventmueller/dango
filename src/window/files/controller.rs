//! The controller of the files panel: hands the view's events to the model and carries out the effects it returns, on
//! the file system, the pasteboard, in the Finder and in the window.

use std::path::Path;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSModalResponse, NSOutlineView, NSPasteboard, NSPasteboardTypeString, NSTextField, NSWorkspace,
};
use objc2_foundation::{NSArray, NSFileManager, NSPoint, NSRect, NSSize, NSURL};
use splat_i18n::t;

use super::model::{Command, Effect, Kind, MenuEntry, Model, Naming, valid_name};
use super::view::{FileTreeSource, path_of};
use crate::util::{constant, ns};

impl FileTreeSource {
    pub(super) fn row_clicked(&self, view: &NSOutlineView, row: isize) {
        let Some(item) = (row >= 0).then(|| view.itemAtRow(row)).flatten() else {
            return;
        };
        let Some(path) = path_of(&item) else { return };
        let kind = if path.is_dir() { Kind::Folder } else { Kind::File };
        match Model::click(&path, kind) {
            // SAFETY: The item came from this outline view, which knows it.
            Effect::Toggle => unsafe {
                if view.isItemExpanded(Some(&item)) {
                    view.collapseItem(Some(&item));
                } else {
                    view.expandItem(Some(&item));
                }
            },
            effect => self.run(effect),
        }
    }

    pub(super) fn row_double_clicked(&self, view: &NSOutlineView, row: isize) {
        let Some(path) = (row >= 0)
            .then(|| view.itemAtRow(row))
            .flatten()
            .and_then(|item| path_of(&item))
        else {
            return;
        };
        let kind = if path.is_dir() { Kind::Folder } else { Kind::File };
        if let Some(effect) = Model::double_click(&path, kind) {
            self.run(effect);
        }
    }

    /// What the context menu offers for `path`, the row it opens on.
    pub(super) fn menu_opens(&self, path: Option<&Path>) -> Vec<MenuEntry> {
        let kind = path.and_then(kind_of);
        self.ivars().model.borrow_mut().open_menu(path, kind)
    }

    pub(super) fn command(&self, command: Command) {
        let effect = {
            let model = self.ivars().model.borrow();
            let kind = model.clicked().and_then(kind_of);
            model.command(command, kind)
        };
        if let Some(effect) = effect {
            self.run(effect);
        }
    }

    pub(super) fn create_from_header(&self, folder: bool) {
        let view = self.ivars().controller.load().and_then(|c| c.file_tree_view());
        let selected = view
            .and_then(|view| view.itemAtRow(view.selectedRow()))
            .and_then(|item| path_of(&item));
        let kind = selected.as_deref().and_then(kind_of);
        let effect = self
            .ivars()
            .model
            .borrow()
            .create_from_header(selected.as_deref(), kind, folder);
        self.run(effect);
    }

    pub(super) fn folder_opens(&self, path: &Path) {
        let _ = self.ivars().project.tree().expand(path);
    }

    pub(super) fn folder_closed(&self, path: &Path) {
        self.ivars().project.tree().collapse(path);
    }

    fn run(&self, effect: Effect) {
        let controller = self.ivars().controller.load();
        match effect {
            Effect::Toggle => {}
            Effect::Preview(path) => {
                if let Some(controller) = controller {
                    controller.preview_file(&path);
                }
            }
            Effect::Open(path) => {
                if let Some(controller) = controller {
                    controller.open_file(&path);
                }
            }
            Effect::OpenBeside(path) => {
                if let Some(controller) = controller {
                    controller.open_file_beside(&path);
                }
            }
            Effect::OpenAsPreview(path) => {
                if let Some(controller) = controller {
                    crate::preview::open_drawing(&controller, &path);
                }
            }
            Effect::RevealInFinder(path) => {
                let urls = NSArray::from_retained_slice(&[file_url(&path)]);
                NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&urls);
            }
            Effect::Copy(text) => copy_text(&text),
            Effect::MoveToTrash(path) => {
                let moved =
                    NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&file_url(&path), None);
                if let Err(error) = moved {
                    crate::diag::report("moving to the Trash", error.localizedDescription());
                }
            }
            Effect::AskName { naming, title, current } => self.ask_name(&naming, title, &current),
            Effect::Create { path, folder } => {
                let result = create(&path, folder).map_err(|error| error.to_string());
                if let Some(effect) = Model::created(&path, folder, result) {
                    self.run(effect);
                }
            }
            Effect::Rename { from, to } => {
                let result = std::fs::rename(&from, &to).map_err(|error| error.to_string());
                if let Some(effect) = Model::renamed(&from, result) {
                    self.run(effect);
                }
            }
            Effect::Fail { message, detail } => show_error(self.mtm(), &message, &detail),
        }
    }

    fn ask_name(&self, naming: &Naming, title: &str, current: &str) {
        let Some(name) = ask_name(self.mtm(), title, current) else {
            return;
        };
        if let Some(effect) = Model::named(naming, &name, Path::exists) {
            self.run(effect);
        }
    }
}

/// What `path` is on disk, `None` if nothing is there.
fn kind_of(path: &Path) -> Option<Kind> {
    if path.is_dir() {
        Some(Kind::Folder)
    } else if path.is_file() {
        Some(Kind::File)
    } else {
        None
    }
}

fn create(path: &Path, folder: bool) -> std::io::Result<()> {
    if folder {
        std::fs::create_dir(path)
    } else {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(|_| ())
    }
}

fn file_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&ns(&path.to_string_lossy()))
}

fn copy_text(text: &str) {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.setString_forType(&ns(text), constant!(NSPasteboardTypeString));
}

/// Asks for a file name; `None` if the user cancels or leaves it empty, or it holds a slash.
fn ask_name(mtm: MainThreadMarker, title: &str, current: &str) -> Option<String> {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(title));
    alert.addButtonWithTitle(&ns(t("OK")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(current), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(260.0, 24.0)));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    let response: NSModalResponse = alert.runModal();
    if response != objc2_app_kit::NSAlertFirstButtonReturn {
        return None;
    }
    valid_name(&field.stringValue().to_string())
}

fn show_error(mtm: MainThreadMarker, message: &str, detail: &str) {
    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(objc2_app_kit::NSAlertStyle::Warning);
    alert.setMessageText(&ns(message));
    alert.setInformativeText(&ns(detail));
    alert.runModal();
}

#[cfg(test)]
mod tests {
    use ide_core::testing::Scratch;

    use super::*;

    #[test]
    fn a_new_file_or_folder_is_made_and_then_known_as_such() {
        let scratch = Scratch::new("files-create");
        let file = scratch.path().join("a.txt");
        let folder = scratch.path().join("sub");
        assert_eq!(kind_of(&file), None);
        create(&file, false).unwrap();
        create(&folder, true).unwrap();
        assert_eq!(kind_of(&file), Some(Kind::File));
        assert_eq!(kind_of(&folder), Some(Kind::Folder));
    }

    #[test]
    fn creating_never_overwrites_what_is_there() {
        let scratch = Scratch::new("files-exists");
        let file = scratch.path().join("a.txt");
        std::fs::write(&file, "kept").unwrap();
        assert!(create(&file, false).is_err());
        assert!(create(scratch.path(), true).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "kept");
    }
}
