//! The app as the language servers reach it beyond one editor (E99): the editors there are, a window's call hierarchy,
//! the editor's font and colours, trust, and the state this crate keeps in the running app.

use crate::EditorSerial;
use ide_ui_common::window::WindowId;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSFont};

use crate::editor::Editor;
use crate::hierarchy::Hierarchy;

/// What the language servers keep beyond one editor.
#[derive(Default)]
pub struct State {
    pub(crate) lsp_workspaces: crate::Workspaces,
    pub(crate) lsp_diagnostics: Arc<crate::AgentDiagnostics>,
    pub(crate) server_leases: crate::Leases,
    pub(crate) inlay_requests: crate::inlay::InlayRequests,
    pub(crate) formatting: crate::Formatting,
    pub(crate) completion: crate::Completion,
    pub(crate) signature_help: crate::ShownSignature,
    pub(crate) code_actions: crate::CodeActions,
    pub(crate) location_sheet: crate::LocationSheets,
}

/// What the language servers need of the app.
pub trait Host {
    /// The crate's state in the running app.
    fn state(&self) -> Rc<State>;
    /// The open editor with `serial`.
    fn find_editor(&self, serial: EditorSerial) -> Option<Box<dyn Editor>>;
    /// Every open editor, in every window.
    fn all_editors(&self) -> Vec<Box<dyn Editor>>;
    /// The editor of `path` in the window `window`.
    fn editor_in(&self, window: WindowId, path: &Path) -> Option<Box<dyn Editor>>;
    /// Shows `hierarchy` in the window `window`.
    fn show_hierarchy(&self, window: WindowId, hierarchy: Hierarchy);
    /// The editor's font of code at `size` points.
    fn code_font(&self, size: f64) -> Retained<NSFont>;
    /// The colour of tokens of kind `token`.
    fn palette_color(&self, token: splat_markdown::highlight::Token) -> Retained<NSColor>;
    /// Whether the user trusts `folder` (E22).
    fn is_trusted(&self, folder: &Path) -> bool;
    /// `path` as the app shows it to the user, with the home folder as `~`.
    fn display_path(&self, path: &Path) -> String;
}

static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();

/// Makes `host` the app the language servers belong to; once, before the first editor opens.
pub fn install(host: &'static (dyn Host + Sync)) {
    let _ = HOST.set(host);
}

fn host() -> &'static (dyn Host + Sync) {
    *HOST.get().expect("the app installs its host before the first editor")
}

pub(crate) fn state() -> Rc<State> {
    host().state()
}

pub(crate) fn find_editor(serial: EditorSerial) -> Option<Box<dyn Editor>> {
    host().find_editor(serial)
}

pub(crate) fn all_editors() -> Vec<Box<dyn Editor>> {
    host().all_editors()
}

pub(crate) fn editor_in(window: WindowId, path: &Path) -> Option<Box<dyn Editor>> {
    host().editor_in(window, path)
}

pub(crate) fn show_hierarchy(window: WindowId, hierarchy: Hierarchy) {
    host().show_hierarchy(window, hierarchy);
}

pub(crate) fn code_font(size: f64) -> Retained<NSFont> {
    host().code_font(size)
}

pub(crate) fn palette_color(token: splat_markdown::highlight::Token) -> Retained<NSColor> {
    host().palette_color(token)
}

pub(crate) fn is_trusted(folder: &Path) -> bool {
    host().is_trusted(folder)
}

pub(crate) fn display_path(path: &Path) -> String {
    host().display_path(path)
}
