//! The debugger (B1, E94) as the app reaches it: `ide_ui_debug` by the window, and the parts that live in the
//! editor — breakpoints in the gutter, the paused line with its values, and the rotor "Haltepunkte".

mod editor;
mod rotor;

use std::path::Path;

use ide_dap::kept::ConsoleLine;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;

use crate::editor::EditorTextView;
use crate::window::{TabId, WindowController};

pub(crate) use editor::{draw_paused_line, gutter_click, gutter_marks, gutter_menu, hover, paused_in, paused_line};
pub(crate) use ide_ui_debug::gutter::{draw as draw_gutter_mark, draw_paused_arrow};
pub(crate) use rotor::{Rotors, attach as attach_rotor};

/// Puts the debugger of the window's folder in place, with the breakpoints and watches the project kept.
pub(crate) fn attach(controller: &WindowController) {
    ide_ui_debug::attach(controller.handle());
}

/// The user trusts `root` now: its configurations are read and sessions may start.
pub(crate) fn trust_changed(root: &Path) {
    ide_ui_debug::trust_changed(root);
}

/// The window of `controller` closes: its sessions stop, a launched program with them, an attached one let go.
pub(crate) fn window_closed(controller: &WindowController) {
    ide_ui_debug::window_closed(controller.handle());
}

/// The Debug menu of the menu bar; `key` gives each command's shortcut.
pub(crate) fn menu(mtm: MainThreadMarker, key: &dyn Fn(&str) -> String) -> Retained<NSMenuItem> {
    ide_ui_debug::menu(mtm, key)
}

/// An edit of `editor`'s text moves the breakpoints of its file along.
pub(crate) fn edited(editor: &EditorTextView, change: &ide_document::Change) {
    editor::edited(editor, change);
}

/// The document `id` of `controller`'s window closes; a console among them is forgotten.
pub(crate) fn closing(controller: &WindowController, id: TabId) {
    ide_ui_debug::closing(controller.handle(), id);
}

/// A debug console as the layout keeps it, which comes back as the finished history of its session, only to read; no
/// session starts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct KeptConsole {
    /// The name of the session it belonged to.
    pub(crate) title: String,
    /// What it showed, oldest first.
    #[serde(default)]
    pub(crate) history: Vec<ConsoleLine>,
}

impl ide_core::layout::Kind for KeptConsole {
    const NAME: &'static str = "debug";
}

/// The name and history of the console in tab `id`, for the layout (Review C18).
pub(crate) fn console_of_tab(controller: &WindowController, id: TabId) -> Option<(String, Vec<ConsoleLine>)> {
    ide_ui_debug::console_of_tab(controller.handle(), id)
}

/// Opens a console that came back with the layout, finished and only to read; no session starts (Review C18).
pub(crate) fn reopen_console(controller: &WindowController, title: &str, history: Vec<ConsoleLine>) -> bool {
    ide_ui_debug::reopen_console(controller.handle(), title, history)
}

/// The debug sessions of `controller`'s window for the activity view: id, name and how it stands.
pub(crate) fn activity(controller: &WindowController) -> Vec<(u64, String, String)> {
    ide_ui_debug::activity(controller.handle())
}

/// Shows the debug panel of `controller`'s window, as the activity view's "Zeigen" does.
pub(crate) fn show(controller: &WindowController) {
    ide_ui_debug::show(controller.handle());
}

/// Stops the session `id` of `controller`'s window, as the activity view's "Stopp" does.
pub(crate) fn stop(controller: &WindowController, id: u64) {
    ide_ui_debug::stop(controller.handle(), id);
}

pub(crate) use editor::{forget_values, refresh_editors};

#[cfg(feature = "selftest")]
pub(crate) mod probe;
