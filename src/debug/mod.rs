//! The debugger (B1, E94) as the app reaches it: `ide_ui_debug` by the window, and the parts that live in the
//! editor — breakpoints in the gutter, the paused line with its values, and the rotor "Haltepunkte".

mod editor;
mod rotor;

use std::path::Path;

use ide_core::debugging::ConsoleLine;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;

use crate::editor::EditorTextView;
use crate::window::WindowController;

pub use editor::{draw_paused_line, gutter_click, gutter_mark, gutter_menu, hover, paused_in, paused_line};
pub use ide_ui_debug::gutter::{draw as draw_gutter_mark, draw_paused_arrow};
pub use rotor::{Rotors, attach as attach_rotor};

/// Puts the debugger of the window's folder in place, with the breakpoints and watches the project kept.
pub fn attach(controller: &WindowController) {
    ide_ui_debug::attach(controller.handle());
}

/// The user trusts `root` now: its configurations are read and sessions may start.
pub fn trust_changed(root: &Path) {
    ide_ui_debug::trust_changed(root);
}

/// The window of `controller` closes: its sessions stop, a launched program with them, an attached one let go.
pub fn window_closed(controller: &WindowController) {
    ide_ui_debug::window_closed(controller.handle());
}

/// The Debug menu of the menu bar; `key` gives each command's shortcut.
pub fn menu(mtm: MainThreadMarker, key: &dyn Fn(&str) -> String) -> Retained<NSMenuItem> {
    ide_ui_debug::menu(mtm, key)
}

/// An edit of `editor`'s text moves the breakpoints of its file along.
pub fn edited(editor: &EditorTextView, change: &ide_document::Change) {
    editor::edited(editor, change);
}

/// The document `id` of `controller`'s window closes; a console among them is forgotten.
pub fn closing(controller: &WindowController, id: isize) {
    ide_ui_debug::closing(controller.handle(), id);
}

/// The name and history of the console in tab `id`, for the layout (Review C18).
pub fn console_of_tab(controller: &WindowController, id: isize) -> Option<(String, Vec<ConsoleLine>)> {
    ide_ui_debug::console_of_tab(controller.handle(), id)
}

/// Opens a console that came back with the layout, finished and only to read; no session starts (Review C18).
pub fn reopen_console(controller: &WindowController, title: &str, history: Vec<ConsoleLine>) -> bool {
    ide_ui_debug::reopen_console(controller.handle(), title, history)
}

/// The debug sessions of `controller`'s window for the activity view: id, name and how it stands.
pub fn activity(controller: &WindowController) -> Vec<(u64, String, String)> {
    ide_ui_debug::activity(controller.handle())
}

/// Shows the debug panel of `controller`'s window, as the activity view's "Zeigen" does.
pub fn show(controller: &WindowController) {
    ide_ui_debug::show(controller.handle());
}

/// Stops the session `id` of `controller`'s window, as the activity view's "Stopp" does.
pub fn stop(controller: &WindowController, id: u64) {
    ide_ui_debug::stop(controller.handle(), id);
}

pub(crate) use editor::{forget_values, refresh_editors};

#[cfg(feature = "selftest")]
pub mod probe;
