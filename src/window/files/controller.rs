//! The controller of the files panel: hands the view's events to the model and carries out the effects it returns, on
//! the file system, the pasteboard, in the Finder and in the window.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ide_project::Entry;
use ide_ui_common::alert::{Alert, Role, first, warn};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSOutlineView, NSPasteboard, NSPasteboardTypeString, NSTableViewAnimationOptions, NSTextField, NSWorkspace,
};
use objc2_foundation::{NSArray, NSFileManager, NSMutableIndexSet, NSPoint, NSRect, NSSize, NSURL};
use splat_i18n::t;

use super::model::{Changes, Command, Effect, Kind, MenuEntry, Model, Naming, changes, valid_name};
use super::view::{FileTreeSource, Pending, path_of};
use crate::util::{FlagGuard, constant, main_after, ns};

/// How long a folder's entries may take to read before the folder shows a spinner.
const SPINNER_AFTER: Duration = Duration::from_millis(100);

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

    pub(super) fn create_from_header(&self, new: Kind) {
        let view = self.ivars().controller.load().and_then(|c| c.file_tree_view());
        let selected = view
            .and_then(|view| view.itemAtRow(view.selectedRow()))
            .and_then(|item| path_of(&item));
        let kind = selected.as_deref().and_then(kind_of);
        let effect = self
            .ivars()
            .model
            .borrow()
            .create_from_header(selected.as_deref(), kind, new);
        self.run(effect);
    }

    /// The folder `path` opens: its entries are read on the disk lane, with a spinner in the folder once that takes
    /// `SPINNER_AFTER`; `deep` opens the folders inside as well once they are known.
    pub(super) fn folder_opens(&self, path: &Path, deep: bool) {
        let ivars = self.ivars();
        if ivars.shown.borrow().contains_key(path) {
            return;
        }
        let known = ivars
            .pending
            .borrow_mut()
            .insert(path.to_path_buf(), Pending { spinner: None, deep })
            .is_some();
        if known {
            return;
        }
        ivars.project.expand(path);
        self.spin_later(path.to_path_buf());
    }

    /// Waits `SPINNER_AFTER`, then shows a spinner in `dir` if its entries are still being read.
    pub(super) fn spin_later(&self, dir: PathBuf) {
        let Some(window) = self.ivars().controller.load().map(|c| c.id()) else {
            return;
        };
        main_after(SPINNER_AFTER, move || {
            crate::window::deliver(window, move |controller| {
                controller.with_file_tree(|files| files.spin(&dir));
            });
        });
    }

    pub(super) fn folder_closed(&self, path: &Path) {
        let ivars = self.ivars();
        ivars.shown.borrow_mut().retain(|dir, _| !dir.starts_with(path));
        ivars.pending.borrow_mut().retain(|dir, _| !dir.starts_with(path));
        ivars.project.collapse(path);
    }

    /// Shows a spinner in `dir` while its entries are still being read.
    pub(super) fn spin(&self, view: &NSOutlineView, dir: &Path) {
        {
            let mut pending = self.ivars().pending.borrow_mut();
            let Some(waiting) = pending.get_mut(dir).filter(|p| p.spinner.is_none()) else {
                return;
            };
            waiting.spinner = Some(NSObject::new());
        }
        self.reload_folder(view, dir);
    }

    /// Shows what the tree read of `dir`: all its rows where it just opened, else only the rows that came, went or
    /// look different, which leaves the folders inside as they are.
    pub(super) fn listed(&self, view: &NSOutlineView, dir: &Path, entries: Option<&Arc<[Entry]>>) {
        let ivars = self.ivars();
        let waiting = ivars.pending.borrow_mut().remove(dir);
        let old = ivars.shown.borrow().get(dir).cloned();
        let new = entries.cloned().unwrap_or_else(|| Arc::from([]));
        match (old, &waiting) {
            (Some(old), None) => {
                let changes = changes(&old, &new);
                ivars.shown.borrow_mut().insert(dir.to_path_buf(), new.clone());
                match changes {
                    Some(changes) => self.move_rows(view, dir, &new, &changes),
                    None => self.reload_folder(view, dir),
                }
            }
            (_, Some(waiting)) => {
                ivars.shown.borrow_mut().insert(dir.to_path_buf(), new.clone());
                self.reload_folder(view, dir);
                if waiting.deep {
                    self.open_inside(view, &new);
                }
            }
            (None, None) => return,
        }
        if entries.is_none() {
            ivars.shown.borrow_mut().remove(dir);
        }
    }

    fn move_rows(&self, view: &NSOutlineView, dir: &Path, entries: &[Entry], changes: &Changes) {
        let parent = self.item_of(dir);
        if parent.is_none() && dir != self.ivars().project.root() {
            return;
        }
        let parent: Option<&AnyObject> = parent.as_deref().map(|p| p.as_ref());
        let set = |indexes: &[usize]| {
            let set = NSMutableIndexSet::new();
            for index in indexes {
                set.addIndex(*index);
            }
            set
        };
        let none = NSTableViewAnimationOptions::empty();
        view.beginUpdates();
        // SAFETY: The parent is nil for the root or an item the data source handed this view, and the indexes are
        // those of the rows it showed before and shows now.
        unsafe {
            if !changes.removed.is_empty() {
                view.removeItemsAtIndexes_inParent_withAnimation(&set(&changes.removed), parent, none);
            }
            if !changes.inserted.is_empty() {
                view.insertItemsAtIndexes_inParent_withAnimation(&set(&changes.inserted), parent, none);
            }
        }
        view.endUpdates();
        for entry in changes.redrawn.iter().filter_map(|i| entries.get(*i)) {
            let item = self.item_for(entry);
            // SAFETY: The item is one the data source handed this view.
            unsafe { view.reloadItem(Some(&item)) };
        }
    }

    /// Opens the folders among `entries`, and the folders inside them as they are read.
    fn open_inside(&self, view: &NSOutlineView, entries: &[Entry]) {
        let _cascade = FlagGuard::raise(&self.ivars().cascade);
        for entry in entries.iter().filter(|e| e.can_expand()) {
            let item = self.item_for(entry);
            // SAFETY: The item is one the data source handed this view for a row of the folder just shown.
            unsafe { view.expandItem_expandChildren(Some(&item), true) };
        }
    }

    /// Asks the view for the rows of `dir` again, and of the folders inside.
    fn reload_folder(&self, view: &NSOutlineView, dir: &Path) {
        if dir == self.ivars().project.root() {
            // SAFETY: Nil stands for the root, whose children the data source answers for.
            unsafe { view.reloadItem_reloadChildren(None, true) };
        } else if let Some(item) = self.item_of(dir) {
            // SAFETY: The item is one the data source handed to this view, kept alive by the source.
            unsafe { view.reloadItem_reloadChildren(Some(&item), true) };
        }
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
                    controller.open_file_beside(&path, crate::window::panes::Side::Right);
                }
            }
            Effect::OpenBelow(path) => {
                if let Some(controller) = controller {
                    controller.open_file_beside(&path, crate::window::panes::Side::Below);
                }
            }
            Effect::OpenAsPreview(path) => {
                if let Some(controller) = controller {
                    crate::preview::open_drawing(&controller, &path);
                }
            }
            Effect::OpenInBrowserTab(path) => {
                if let Some(controller) = controller {
                    crate::site_preview::open(&controller, &path);
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
            Effect::Create { path, kind } => {
                let result = create(&path, kind);
                if let Some(effect) = Model::created(&path, kind, result) {
                    self.run(effect);
                }
            }
            Effect::Rename { from, to } => {
                let result = std::fs::rename(&from, &to);
                if let Some(effect) = Model::renamed(&from, result) {
                    self.run(effect);
                }
            }
            Effect::Fail { message, detail } => warn(self.mtm(), &message, &detail),
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

fn create(path: &Path, kind: Kind) -> std::io::Result<()> {
    match kind {
        Kind::Folder => std::fs::create_dir(path),
        Kind::File => std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(|_| ()),
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
    let field = NSTextField::textFieldWithString(&ns(current), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(260.0, 24.0)));
    let response = Alert::new(mtm, title)
        .button(t("OK"), Role::Plain)
        .button(t("Abbrechen"), Role::Plain)
        .accessory(&field)
        .focus(&field)
        .run_modal();
    if !first(response) {
        return None;
    }
    valid_name(&field.stringValue().to_string())
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
        create(&file, Kind::File).unwrap();
        create(&folder, Kind::Folder).unwrap();
        assert_eq!(kind_of(&file), Some(Kind::File));
        assert_eq!(kind_of(&folder), Some(Kind::Folder));
    }

    #[test]
    fn creating_never_overwrites_what_is_there() {
        let scratch = Scratch::new("files-exists");
        let file = scratch.path().join("a.txt");
        std::fs::write(&file, "kept").unwrap();
        assert!(create(&file, Kind::File).is_err());
        assert!(create(scratch.path(), Kind::Folder).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "kept");
    }
}
