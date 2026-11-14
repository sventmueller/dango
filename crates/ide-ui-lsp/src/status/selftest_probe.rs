//! The self-test's hooks of `status.rs`.

use super::*;

/// The breadcrumbs the bar of `view` shows, for the self-test.
pub fn crumbs(view: &dyn Editor) -> String {
    let state = view.lsp().borrow();
    state
        .status
        .as_ref()
        .filter(|bar| !bar.crumbs.isHidden())
        .map(|bar| bar.crumbs.title().to_string())
        .unwrap_or_default()
}

/// What the bar of `view` shows, for the self-test: the problems and the server's line.
pub fn shown(view: &dyn Editor) -> (String, String) {
    let state = view.lsp().borrow();
    state.status.as_ref().map_or_else(Default::default, |bar| {
        (bar.problems.title().to_string(), bar.server.stringValue().to_string())
    })
}

/// The passing remark the status bar of `view` shows, for the self-test.
pub fn note_of(view: &dyn Editor) -> Option<String> {
    view.lsp().borrow().note.clone()
}
