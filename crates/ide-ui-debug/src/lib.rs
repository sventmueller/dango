//! The debugger (B1, E94): the panel with sessions, threads, call stacks and breakpoints, the inspector with the
//! selected frame's variables and watch expressions, the Debug menu, the debug console as a tab, and the breakpoints
//! and paused place the app's editors draw. The protocol and the adapters are `ide_dap`'s; the crate knows a window
//! by its id and reaches the app's editors and terminals through its `Host` (E99).
//!
//! The breakpoint menu items of a line carry what they change, and read it back:
//!
//! ```
//! use ide_ui_debug::gutter::words;
//! use ide_ui_debug::model::breakpoints::Look;
//!
//! assert!(words(Some(Look::Enabled), false, 4).is_some_and(|w| w.ends_with('5')));
//! assert!(words(None, false, 4).is_none());
//! ```

mod actions;
mod commands;
mod console;
mod controller;
pub mod gutter;
mod host;
pub use ide_dap::model;
mod panel;
mod rows;
mod session;
mod sheets;
mod ui;

use std::path::Path;

use ide_core::debugging::ConsoleLine;
use ide_ui_common::window::{Panel, Window};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;

pub use host::{Host, State, install};
pub use panel::DebugPanel;

/// Puts the debugger of `window`'s folder in place, with the breakpoints and watches the project kept.
pub fn attach(window: Window) {
    if let Some(root) = window.project_root() {
        DebugPanel::attach(&window, root);
    }
}

/// The debugger of `window`.
pub fn of(window: Window) -> Option<Retained<DebugPanel>> {
    panel::of(&window)
}

/// The user trusts `root` now: its configurations are read and sessions may start.
pub fn trust_changed(root: &Path) {
    if let Some(panel) = panel::for_root(root) {
        panel.load_configurations();
        panel.refresh();
    }
}

/// `window` closes: its sessions stop, a launched program with them, an attached one let go.
pub fn window_closed(window: Window) {
    if let Some(panel) = panel::of(&window) {
        panel.end_all();
    }
}

/// The Debug menu of the menu bar; `key` gives each command's shortcut.
pub fn menu(mtm: MainThreadMarker, key: &dyn Fn(&str) -> String) -> Retained<NSMenuItem> {
    let state = host::state();
    let target = state.commands.get_or_init(|| commands::target(mtm));
    commands::menu(mtm, key, target)
}

/// The document `tab` of `window` closes; a console among them is forgotten.
pub fn closing(window: Window, tab: isize) {
    if let Some(panel) = panel::of(&window) {
        panel.console_closed(tab);
    }
}

/// The name and history of the console in tab `tab`, for the layout (Review C18).
pub fn console_of_tab(window: Window, tab: isize) -> Option<(String, Vec<ConsoleLine>)> {
    let console = panel::of(&window)?.console_of_tab(tab)?;
    let title = console.title.borrow().clone();
    Some((title, console.lines()))
}

/// Opens a console that came back with the layout, finished and only to read; no session starts (Review C18).
pub fn reopen_console(window: Window, title: &str, history: Vec<ConsoleLine>) -> bool {
    panel::of(&window).is_some_and(|panel| panel.reopen_console(title, history))
}

/// The debug sessions of `window` for the activity view: id, name and how it stands.
pub fn activity(window: Window) -> Vec<(u64, String, String)> {
    let Some(panel) = panel::of(&window) else {
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

/// Shows the debug panel of `window`, as the activity view's "Zeigen" does.
pub fn show(window: Window) {
    window.activate_panel(Panel::Debug);
}

/// Stops the session `id` of `window`, as the activity view's "Stopp" does.
pub fn stop(window: Window, id: u64) {
    if let Some(session) = panel::of(&window).and_then(|p| p.session(id)) {
        session.stop();
    }
}

/// What the self-test reads from and does with a window's debugger.
#[cfg(feature = "selftest")]
pub mod selftest;
