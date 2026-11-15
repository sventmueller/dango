//! What the self-test reads from and does with the parts of `settings.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// Where the language servers tab sits in the toolbar.
pub(super) const SERVERS_TAB: isize = 4;

pub(crate) use super::predict::shown as prediction_shown;

pub(crate) use super::predict::{LocalShown, local_shown as prediction_local};

/// Brings the settings window to the front on its language servers tab, for the self-test.
pub(crate) fn show_servers(mtm: MainThreadMarker) {
    show_tab(mtm, SERVERS_TAB);
}

pub(crate) use super::servers::selftest as servers_selftest;

pub(crate) use super::editor::shown as editor_switches;

/// The list of Claude's stored rules in the Agents tab, once the settings window was built, for the self-test.
pub(crate) fn stored_rules() -> Option<Retained<stored_rules::StoredRules>> {
    agents_tab_now().map(|agents| agents.stored_rules())
}

/// What the Agents tab shows, line by line, for the self-test.
pub(crate) fn agents_lines() -> Vec<String> {
    agents_tab_now().map(|agents| agents.lines()).unwrap_or_default()
}

/// Opens the ACP registry's window as the Agents tab's button does; the window and the ids it lists, for the
/// self-test.
pub(crate) fn open_register() -> Option<(Retained<objc2_app_kit::NSPanel>, Vec<String>)> {
    agents::open_register(&*agents_tab_now()?);
    agents::register_window()
}

pub(crate) use super::agents::register_window;

/// The CI tab of the open settings window, for the self-test.
pub(crate) fn ci_tab() -> Option<Retained<ci::CiSettings>> {
    with_window(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().ci.clone())
    })
}

pub(crate) use super::ci::{List as CiList, selftest as ci_selftest};

/// The Agent tab of the open settings window, for the self-test.
pub(crate) fn agent_tab() -> Option<Retained<agent::AgentSettings>> {
    with_window(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, controller)| controller.ivars().agent.clone())
    })
}

pub(crate) use super::agent::{AgentSettings, List as AgentList, selftest as agent_selftest};

pub(crate) use super::commit_message::selftest as commit_message_selftest;
