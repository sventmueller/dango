//! The debugger (B1, E94): the sixth panel with sessions, threads, call stacks and breakpoints, the inspector with the
//! selected frame's variables and watch expressions, the Debug menu, the debug console as a tab, breakpoints in the
//! gutter and the paused line with its values in the editor. The protocol and the adapters are `ide_dap`'s.

mod actions;
mod commands;
mod console;
mod controller;
mod editor;
mod gutter;
pub mod model;
mod panel;
mod rotor;
mod rows;
mod session;
mod sheets;
mod ui;

use std::cell::OnceCell;
use std::path::Path;

use ide_core::debugging::ConsoleLine;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;

use crate::editor::EditorTextView;
use crate::window::WindowController;

pub use editor::{draw_paused_line, hover, paused_in};
pub use gutter::{
    click as gutter_click, draw as draw_gutter_mark, draw_paused_arrow, mark as gutter_mark, menu as gutter_menu,
};
pub use panel::DebugPanels;
pub use rotor::attach as attach_rotor;

/// The debuggers of every window, the Debug menu's target and the editors' breakpoint rotors.
#[derive(Default)]
pub struct DebugState {
    panels: DebugPanels,
    commands: OnceCell<Retained<commands::DebugCommands>>,
    rotors: rotor::Rotors,
}

/// Puts the debugger of the window's folder in place, with the breakpoints and watches the project kept.
pub fn attach(controller: &WindowController) {
    if let Some(root) = controller.project_root() {
        panel::DebugPanel::attach(controller, root);
    }
}

/// The user trusts `root` now: its configurations are read and sessions may start.
pub fn trust_changed(root: &Path) {
    if let Some(panel) = panel::for_root(root) {
        panel.load_configurations();
        panel.refresh();
    }
}

/// The window of `controller` closes: its sessions stop, a launched program with them, an attached one let go.
pub fn window_closed(controller: &WindowController) {
    if let Some(panel) = panel::of(controller) {
        panel.end_all();
    }
}

/// The Debug menu of the menu bar; `key` gives each command's shortcut.
pub fn menu(
    mtm: MainThreadMarker,
    key: &dyn Fn(&str) -> String,
    context: &crate::context::AppContext,
) -> Retained<NSMenuItem> {
    let target = context.debug.commands.get_or_init(|| commands::target(mtm));
    commands::menu(mtm, key, target)
}

/// An edit of `editor`'s text moves the breakpoints of its file along.
pub fn edited(editor: &EditorTextView, change: &ide_document::Change) {
    editor::edited(editor, change);
}

/// The document `id` of `controller`'s window closes; a console among them is forgotten.
pub fn closing(controller: &WindowController, id: isize) {
    if let Some(panel) = panel::of(controller) {
        panel.console_closed(id);
    }
}

/// The name and history of the console in tab `id`, for the layout (Review C18).
pub fn console_of_tab(controller: &WindowController, id: isize) -> Option<(String, Vec<ConsoleLine>)> {
    let console = panel::of(controller)?.console_of_tab(id)?;
    let title = console.title.borrow().clone();
    Some((title, console.lines()))
}

/// Opens a console that came back with the layout, finished and only to read; no session starts (Review C18).
pub fn reopen_console(controller: &WindowController, title: &str, history: Vec<ConsoleLine>) -> bool {
    panel::of(controller).is_some_and(|panel| panel.reopen_console(title, history))
}

/// The debug sessions of `controller`'s window for the activity view: id, name and how it stands.
pub fn activity(controller: &WindowController) -> Vec<(u64, String, String)> {
    let Some(panel) = panel::of(controller) else {
        return Vec::new();
    };
    let model = panel.read();
    model
        .sessions
        .iter()
        .filter(|s| !s.is_over())
        .map(|s| (s.id, s.name.clone(), rows::phase_words(s)))
        .collect()
}

/// Shows the debug panel of `controller`'s window, as the activity view's "Zeigen" does.
pub fn show(controller: &WindowController) {
    controller.activate_panel(crate::window::Panel::Debug);
}

/// Stops the session `id` of `controller`'s window, as the activity view's "Stopp" does.
pub fn stop(controller: &WindowController, id: u64) {
    if let Some(session) = panel::of(controller).and_then(|p| p.session(id)) {
        session.stop();
    }
}

/// What the self-test reads from and does with a window's debugger.
#[cfg(feature = "selftest")]
pub mod selftest;
