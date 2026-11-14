//! What the language servers ask of the app beyond one editor: the editors there are, the windows by id, fonts and
//! colours, trust and the state this module keeps.

use std::path::Path;
use std::rc::Rc;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSFont, NSWindow};

use super::editor::Editor;
use super::hierarchy::Hierarchy;

/// The open editor with `serial`.
pub fn find_editor(serial: u64) -> Option<Box<dyn Editor>> {
    crate::editor::find_editor(serial).map(crate::editor::lsp_editor)
}

/// Every open editor, in every window.
pub fn all_editors() -> Vec<Box<dyn Editor>> {
    crate::editor::all_editors()
        .into_iter()
        .map(crate::editor::lsp_editor)
        .collect()
}

/// The editor of `path` in the window `window`.
pub fn editor_in(window: u64, path: &Path) -> Option<Box<dyn Editor>> {
    let controller = crate::host::window(window)?;
    crate::editor::editor_for(&controller, path).map(crate::editor::lsp_editor)
}

/// Opens `path` in the window `window`, or brings its editor forward.
pub fn open_file(window: u64, path: &Path) {
    if let Some(controller) = crate::host::window(window) {
        controller.open_file(path);
    }
}

/// Shows `hierarchy` in the window `window`.
pub fn show_hierarchy(window: u64, hierarchy: Hierarchy) {
    if let Some(controller) = crate::host::window(window) {
        controller.show_hierarchy(hierarchy);
    }
}

/// The window that shows the folder `root`, by its id.
pub fn window_of(root: &Path) -> Option<u64> {
    crate::window::for_project(root).map(|controller| controller.id())
}

/// The AppKit window of the window `window`.
pub fn window(window: u64) -> Option<Retained<NSWindow>> {
    crate::host::window(window)?.window()
}

/// Shows a warning with `message` and `detail` and waits for OK.
pub fn show_alert(mtm: MainThreadMarker, message: &str, detail: &str) {
    crate::editor::show_alert(mtm, message, detail);
}

/// The editor's font of code at `size` points.
pub fn code_font(size: f64) -> Retained<NSFont> {
    crate::editor::code_font(size)
}

/// The colour of tokens of kind `token`.
pub fn palette_color(token: splat_markdown::highlight::Token) -> Retained<NSColor> {
    crate::editor::palette_color(token)
}

/// Whether the user trusts `folder` (E22).
pub fn is_trusted(folder: &Path) -> bool {
    crate::project::is_trusted(folder)
}

/// `path` as the app shows it to the user, with the home folder as `~`.
pub fn display_path(path: &Path) -> String {
    crate::app::display_path(path)
}

/// What this module keeps beyond the editors.
pub fn state() -> Rc<crate::context::AppContext> {
    crate::context::app()
}
