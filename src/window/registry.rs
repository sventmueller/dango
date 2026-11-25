//! The open windows as the rest of the app reaches them: by id for answers from other threads, by folder, the one in
//! front, and what all of them are told when files, settings or the session change.

use ide_ui_common::window::WindowId;
use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker};
use objc2_app_kit::NSWindow;

use super::{Panel, WindowController, activity_bar, layout};
use crate::context::AppContext;
use crate::project::Listing;
use crate::util::main_soon;

/// Runs `f` with the controller of the window with `id` once the main queue gets to it, unless the window closed
/// in the meantime; the way back for answers from other threads (Review C11).
pub(crate) fn deliver(id: WindowId, f: impl FnOnce(&WindowController) + Send + 'static) {
    main_soon(move || {
        if let Some(controller) = crate::context::app().windows().get(id.get()) {
            f(&controller);
        }
    });
}

/// Shows the folders that changed on disk in the tree of the windows that have `root` open, and lets editors of files
/// there take what changed.
pub(crate) fn tree_changed(root: &Path, listings: &[Listing]) {
    let dirs: Vec<PathBuf> = listings.iter().map(|(dir, _)| dir.clone()).collect();
    crate::editor::files_changed(&dirs);
    tree_listed(root, listings);
}

/// Shows the folders the tree of `root` read in the windows that have it open.
pub(crate) fn tree_listed(root: &Path, listings: &[Listing]) {
    for window in crate::context::app().windows().all() {
        window.tree_listed(root, listings);
    }
}

/// Hides the symbol of `panel` in every window's activity bar, or shows it again, and remembers that; the last symbol
/// that shows stays.
pub(crate) fn toggle_activity_symbol(cx: &AppContext, panel: Panel) {
    let mut hidden = activity_bar::hidden();
    if hidden.contains(&panel) {
        hidden.retain(|p| *p != panel);
    } else if hidden.len() + 1 < Panel::ALL.len() {
        hidden.push(panel);
    } else {
        return;
    }
    activity_bar::store_hidden(&hidden);
    for window in cx.windows().all() {
        window.leave_out_symbols(&hidden);
        if !hidden.contains(&Panel::Ci) {
            crate::ci_panel::shown(&window);
        }
    }
}

/// Lets the tree of every window list what Git ignores and the system hides, or leave it out again.
pub(crate) fn show_ignored_files(cx: &AppContext, on: bool) {
    for window in cx.windows().all() {
        window.show_ignored_files(on);
    }
}

/// Hands quick-open hits to the window whose folder is `root`.
pub(super) fn quick_open_answered(root: &Path, asked: u64, hits: Vec<ide_project::Hit>) {
    for window in crate::context::app().windows().all() {
        if let Some(panel) = window.quick_open_panel()
            && panel.root() == root
        {
            panel.answered(asked, hits);
            return;
        }
    }
}

/// Saves the window tabs of every window for its folder, and where the window is, as the app quits.
pub(crate) fn save_layouts(cx: &AppContext) {
    for window in cx.windows().all() {
        if !window.ivars().closed.get() {
            layout::save(&window);
            window.save_frame();
        }
    }
}

/// The folders open in windows, front to back.
pub(crate) fn open_projects(cx: &AppContext) -> Vec<PathBuf> {
    cx.windows().all().iter().filter_map(|w| w.project()).collect()
}

/// The project window in front: the main window, else the first on screen.
pub(crate) fn frontmost(mtm: MainThreadMarker) -> Option<Retained<WindowController>> {
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    let mut controllers = AppContext::get(mtm).windows().all();
    controllers.retain(|w| !w.ivars().closed.get());
    let mut order: Vec<Retained<NSWindow>> = app.mainWindow().into_iter().collect();
    order.extend(app.orderedWindows().iter());
    order
        .iter()
        .find_map(|window| controllers.iter().find(|c| c.owns_window(window)).cloned())
}

/// The window that shows `root`.
pub(crate) fn for_project(root: &Path) -> Option<Retained<WindowController>> {
    crate::context::app()
        .windows()
        .find(|w| w.project_root().as_deref() == Some(root))
}

/// What each open window shows, in the order they opened, for the next launch.
pub(crate) fn records(cx: &AppContext) -> Vec<crate::session::WindowRecord> {
    cx.windows()
        .all()
        .iter()
        .filter(|w| !w.ivars().closed.get())
        .map(|w| w.record())
        .collect()
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
