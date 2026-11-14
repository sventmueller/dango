//! The open windows as the rest of the app reaches them: by id for answers from other threads, by folder, the one in
//! front, and what all of them are told when files, settings or the session change.

use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker};
use objc2_app_kit::NSWindow;

use super::{Panel, WindowController, activity_bar, layout, windows};
use crate::context::AppContext;
use crate::util::main_soon;

/// Runs `f` with the controller of the window with `id` once the main queue gets to it, unless the window closed
/// in the meantime; the way back for answers from other threads (Review C11).
pub fn deliver(id: u64, f: impl FnOnce(&WindowController) + Send + 'static) {
    main_soon(move || {
        if let Some(controller) = crate::context::app().windows.get(id) {
            f(&controller);
        }
    });
}

/// Shows the changed folders `dirs` in the tree of the window that has `root` open, and lets editors of files there
/// take what changed on disk.
pub fn tree_changed(root: &Path, dirs: &[PathBuf]) {
    crate::editor::files_changed(dirs);
    for window in windows() {
        let ivars = window.ivars();
        let same = ivars.workspace.borrow().as_ref().is_some_and(|p| p.root() == root);
        if same && let Some(files) = ivars.files.borrow().as_ref() {
            files.reload(dirs);
        }
    }
}

/// Hides the symbol of `panel` in every window's activity bar, or shows it again, and remembers that; the last symbol
/// that shows stays.
pub fn toggle_activity_symbol(panel: Panel) {
    let mut hidden = activity_bar::hidden();
    if hidden.contains(&panel) {
        hidden.retain(|p| *p != panel);
    } else if hidden.len() + 1 < Panel::ALL.len() {
        hidden.push(panel);
    } else {
        return;
    }
    activity_bar::store_hidden(&hidden);
    for window in windows() {
        window.leave_out_symbols(&hidden);
    }
}

/// Lets the tree of every window list what Git ignores and the system hides, or leave it out again.
pub fn show_ignored_files(on: bool) {
    for window in windows() {
        let ivars = window.ivars();
        let Some(project) = ivars.workspace.borrow().clone() else {
            continue;
        };
        let changed = project
            .tree()
            .set_options(ide_project::TreeOptions { show_ignored: on });
        if let Some(files) = ivars.files.borrow().as_ref() {
            files.reload(&changed);
        }
    }
}

/// Hands quick-open hits to the window whose folder is `root`.
pub(super) fn quick_open_answered(root: &Path, asked: u64, hits: Vec<ide_project::Hit>) {
    for window in windows() {
        if let Some(panel) = window.ivars().quick_open.borrow().as_ref()
            && panel.root() == root
        {
            panel.answered(asked, hits);
            return;
        }
    }
}

/// Saves the window tabs of every window for its folder, and where the window is, as the app quits.
pub fn save_layouts() {
    for window in windows() {
        if !window.ivars().closed.get() {
            layout::save(&window);
            window.save_frame();
        }
    }
}

/// The folders open in windows, front to back.
pub fn open_projects() -> Vec<PathBuf> {
    windows().iter().filter_map(|w| w.project()).collect()
}

/// The project window in front: the main window, else the first on screen.
pub fn frontmost(mtm: MainThreadMarker) -> Option<Retained<WindowController>> {
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    let mut controllers = AppContext::get(mtm).windows.all();
    controllers.retain(|w| !w.ivars().closed.get());
    let mut order: Vec<Retained<NSWindow>> = app.mainWindow().into_iter().collect();
    order.extend(app.orderedWindows().iter());
    order
        .iter()
        .find_map(|window| controllers.iter().find(|c| c.owns_window(window)).cloned())
}

/// The window that shows `root`.
pub fn for_project(root: &Path) -> Option<Retained<WindowController>> {
    crate::context::app()
        .windows
        .find(|w| w.project_root().as_deref() == Some(root))
}

/// What each open window shows, in the order they opened, for the next launch.
pub fn records() -> Vec<crate::session::WindowRecord> {
    windows()
        .iter()
        .filter(|w| !w.ivars().closed.get())
        .map(|w| w.record())
        .collect()
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
