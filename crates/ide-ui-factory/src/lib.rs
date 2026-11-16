//! The Leitstand's views (phase 13, E175, E183, E185): the panel "Software Factory" beside the activity bar with the
//! folder's orders by station, the Leitstand tab with fleet line, station strip and sortable table, the tab of each order
//! with timeline, request cards, evidence and its buttons G1 and G2, the sheet "Neuer Auftrag", the dialog "Software
//! Factory für <Ordner>" and the items of "Ablage ▸ Auftrag". What they show is decided in [`model`] without AppKit (E98) from
//! `ide_factory`'s models; the orchestrator itself is the app's, which the crate reaches through its [`Host`] and
//! windows through `ide_ui_common::window` (E99). The app hands the crate its [`State`] where it attaches a panel or
//! builds the menus, and calls [`changed`] whenever a folder's factory took a step.
//!
//! An order's tab finds itself again by its key, which the layout keeps (E64):
//!
//! ```
//! use ide_factory::OrderId;
//!
//! let key = ide_ui_factory::order_tab_key(&OrderId("o-7".into()));
//! assert_eq!(ide_ui_factory::order_of_tab_key(&key), Some(OrderId("o-7".into())));
//! ```

mod actions;
mod board;
mod commands;
mod host;
mod metrics;
pub mod model;
mod order;
mod order_view;
mod panel;
mod project;
mod rows;
mod sheet;
mod table;
mod ui;

use std::path::Path;
use std::rc::Rc;

use ide_factory::OrderId;
use ide_ui_common::window::Window;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;

pub use host::{FolderNotices, Host, Notification, SpecAction, State, install};
pub use project::roles::fill as fill_choices;

/// The Leitstand tab as the layout keeps it (E64, F76).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeptLeitstand {}

impl ide_core::layout::Kind for KeptLeitstand {
    const NAME: &'static str = "leitstand";
}

/// An order's tab as the layout keeps it, by the order's id (E64, F76).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeptOrder {
    /// The order's id.
    pub order: String,
}

impl ide_core::layout::Kind for KeptOrder {
    const NAME: &'static str = "order";
}

/// Puts the Leitstand panel of `window`'s folder in place.
pub fn attach(window: Window, state: &Rc<State>) {
    if let Some(root) = window.project_root() {
        panel::FactoryPanel::attach(window, root, state);
    }
}

/// `window`'s folder closes: its panel goes; its tabs close with the window.
pub fn window_closed(window: Window, state: &State) {
    state.panels.detach(window);
}

/// The factory of `root` took a step: every panel and tab of the folder shows it, and the user hears about an order
/// that began to wait, failed or is ready for acceptance (E175).
pub fn changed(state: &State, root: &Path) {
    for panel in state.panels.for_root(root) {
        panel.reload();
    }
    for tab in state.boards.live().into_iter().filter(|tab| tab.root() == root) {
        tab.reload();
    }
    for tab in order::for_root(state, root) {
        tab.reload();
    }
    let notices = state
        .notices
        .borrow_mut()
        .entry(root.to_path_buf())
        .or_default()
        .update(&host::host().panel(root));
    for notice in notices {
        host::host().notify(
            root,
            Notification {
                order: notice.order().clone(),
                title: notice.title().to_string(),
                body: notice.body().to_string(),
            },
        );
    }
}

/// Opens the Leitstand tab of `window`, or brings it to the front; whether it shows.
pub fn show_leitstand(window: Window, state: &Rc<State>) -> bool {
    state
        .panels
        .of(window)
        .is_some_and(|panel| board::show(&panel.target()))
}

/// Opens the tab of the order `order` in `window`, or brings it to the front; whether it shows.
pub fn open_order(window: Window, state: &Rc<State>, order: &OrderId) -> bool {
    state
        .panels
        .of(window)
        .is_some_and(|panel| order::open(&panel.target(), order))
}

/// Shows the sheet "Neuer Auftrag" on `window`.
pub fn new_order(window: Window, state: &Rc<State>) {
    if let Some(panel) = state.panels.of(window) {
        sheet::show(&panel.target());
    }
}

/// Shows the sheet "Neuer Auftrag" on `window` filled in from an issue (E176); whether the folder has a Leitstand.
pub fn new_order_from_issue(window: Window, state: &Rc<State>, prefill: model::sheet::Prefill) -> bool {
    let Some(panel) = state.panels.of(window) else {
        return false;
    };
    sheet::show_prefilled(&panel.target(), Some(prefill));
    true
}

/// Shows "Software Factory für <Ordner>" on `window`.
pub fn project_settings(window: Window, state: &Rc<State>) {
    if let Some(panel) = state.panels.of(window) {
        project::show(&panel.target());
    }
}

/// The key of the tab of `order`, by which the window finds it and the layout keeps it.
pub fn order_tab_key(order: &OrderId) -> String {
    order::tab_key(order)
}

/// The order of an order tab's key.
pub fn order_of_tab_key(key: &str) -> Option<OrderId> {
    order::order_of_key(key)
}

/// "Neuer Auftrag …" and the submenu "Auftrag" for the File menu; `key` gives each command's shortcut.
pub fn file_items(mtm: MainThreadMarker, key: &dyn Fn(&str) -> String, state: &Rc<State>) -> Vec<Retained<NSMenuItem>> {
    let target = state.commands.get_or_init(|| commands::OrderCommands::new(mtm, state));
    commands::items(mtm, key, target)
}

/// The wall clock in milliseconds, for how long ago something was.
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The local day of the wall clock `wall`, such as `2026-11-16`, by the rule the app keys the daily account with.
pub(crate) fn local_day(wall: u64) -> String {
    let seconds = libc::time_t::try_from(wall / 1000).unwrap_or(0);
    // SAFETY: An all-zero `tm` is a valid value of the plain C struct, which `localtime_r` overwrites.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: Both pointers are to live locals of the right types for the call, which keeps neither.
    let filled = unsafe { libc::localtime_r(&raw const seconds, &raw mut tm) };
    if filled.is_null() {
        return "1970-01-01".into();
    }
    format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday)
}

/// What the self-test reads from a window's Leitstand.
#[cfg(feature = "selftest")]
pub mod selftest;
