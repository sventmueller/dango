//! The AI tab: one agent session per tab with its timeline, composer and status, as the official Claude Code
//! extension shows it, drawn natively. Claude runs through `ide_claude::Session`, other agents over ACP through
//! `ide_acp::Session`; the self-test feeds recordings through `ide_claude::replay::Replay` instead.

mod acp;
mod acp_install;
mod acp_words;
mod actions;
pub mod agent_panel;
mod attrs;
mod cards;
mod checkpoint;
mod choices;
mod codex;
mod compose;
mod composer;
mod context;
mod context_view;
mod dialogs;
mod enlarge;
mod figures;
mod format;
mod header;
mod host;
mod inbox;
mod markdown;
mod mention;
mod model;
mod notify;
mod opencode;
mod outside;
mod overlay;
mod own;
pub mod own_agent;
pub mod plan;
mod refresh;
mod requests;
mod rewind;
mod rewind_actions;
mod row;
mod source;
mod status;
mod stop;
pub mod style;
mod suggest;
mod tab;
#[cfg(feature = "selftest")]
mod tab_probe;
mod tab_session;
mod target;
mod text;
mod timeline;
#[cfg(feature = "selftest")]
mod timeline_probe;
mod ui;
mod usage;
pub mod work;

use std::cell::RefCell;
use std::rc::Rc;

use ide_ui_common::window::{AsWindow, Notices, Window};

pub use acp::{AcpAgent, Agent, Availability, agent_name, availability, cached_register, launch_of};
pub use acp_install::confirm_install;
pub(crate) use actions::ask_text;
pub use cards::request::scope_name;
pub use dialogs::remove_claude_rule;
pub use format::{age, elapsed, tokens};
pub(crate) use format::{iso_time, short_age, time_of_day};
pub use host::{Host, install};
pub use outside::ci_log;
pub use source::Resume;
pub use stop::Stop;
pub use tab::AgentTab;
pub use ui::{framed, icon, insets, separator};

/// An AI thread as the layout keeps it, opened again by resuming its session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeptSession {
    /// The agent's id for the session.
    pub session: String,
}

impl ide_core::layout::Kind for KeptSession {
    const NAME: &'static str = "agent";
}

/// The settings this crate declares, for the app's registry of every setting.
pub const SETTINGS: &[ide_core::settings::Declared] = &[
    own_agent::ENDPOINTS.declared(),
    own_agent::MODEL.declared(),
    own_agent::EFFORT.declared(),
    own_agent::PRICES.declared(),
    own_agent::RULES.declared(),
    own_agent::ALLOWED.declared(),
];

/// Every open AI tab, in the order they were opened; a tab leaves when it is closed.
#[derive(Default)]
pub(crate) struct AgentTabs {
    tabs: RefCell<Vec<Rc<AgentTab>>>,
    keys: ide_ui_common::registry::Serial,
}

/// The number of an AI tab; never reused while the app runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentKey(u64);

impl AgentKey {
    /// The number under which the AI tab is known, for a tag or a name.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The key of an AI tab that opens now, which no tab had before.
pub(crate) fn next_key() -> AgentKey {
    AgentKey(crate::host::state().agent_tabs.keys.next())
}

/// The state of the AI tabs, what they show, and the Agents panels.
#[derive(Default)]
pub struct State {
    agent_panels: agent_panel::AgentPanels,
    agent_tabs: AgentTabs,
    plan: plan::PlanState,
    figures: figures::Figures,
    enlarged_picture: enlarge::EnlargedPicture,
    context_gauges: context_view::ContextGauges,
    page_selections: context::PageSelections,
    usage: usage::Usage,
    #[cfg(feature = "selftest")]
    rewind_alerts: rewind_actions::RewindAlerts,
}

fn with_tabs<R>(f: impl FnOnce(&RefCell<Vec<Rc<AgentTab>>>) -> R) -> R {
    f(&crate::host::state().agent_tabs.tabs)
}

/// Opens an AI tab with a new Claude session in the window's folder, or with the past session `resume`.
pub fn open_tab(controller: &(impl AsWindow + ?Sized), resume: Option<Resume>) -> Option<Rc<AgentTab>> {
    let controller = &controller.as_window();
    open_agent(controller, Agent::Claude, resume)
}

/// Opens an AI tab with a new session of `agent` in the window's folder, or with its past session `resume`.
pub fn open_agent(controller: &(impl AsWindow + ?Sized), agent: Agent, resume: Option<Resume>) -> Option<Rc<AgentTab>> {
    let controller = &controller.as_window();
    if let Some(resume) = &resume
        && let Some(open) = find_session(controller, &resume.session)
    {
        open.select();
        return Some(open);
    }
    let tab = AgentTab::open(controller, agent, resume)?;
    started(controller, &tab);
    Some(tab)
}

fn started(controller: &Window, tab: &Rc<AgentTab>) {
    with_tabs(|tabs| tabs.borrow_mut().push(tab.clone()));
    tab.start();
    changed(controller);
}

/// Opens an AI tab with the app's own agent (E6), talking to `choice` or else to the model new threads start with.
pub fn open_own(
    controller: &(impl AsWindow + ?Sized),
    choice: Option<crate::own_agent::ModelChoice>,
) -> Option<Rc<AgentTab>> {
    let controller = &controller.as_window();
    let tab = AgentTab::open(controller, Agent::Own, None)?;
    tab.state.borrow_mut().pick_own(choice);
    started(controller, &tab);
    Some(tab)
}

/// Opens an AI tab with a new session of `agent` that works in `cwd`, such as a thread's worktree (I1), titled
/// `title`.
pub fn open_agent_in(
    controller: &(impl AsWindow + ?Sized),
    agent: Agent,
    cwd: std::path::PathBuf,
    title: Option<&str>,
) -> Option<Rc<AgentTab>> {
    let controller = &controller.as_window();
    let tab = AgentTab::open_in(controller, agent, cwd, None)?;
    if let Some(title) = title {
        tab.set_custom_title(title);
    }
    with_tabs(|tabs| tabs.borrow_mut().push(tab.clone()));
    tab.start();
    changed(controller);
    Some(tab)
}

/// Whether an open AI tab works in `folder`.
pub fn working_in(folder: &std::path::Path) -> bool {
    with_tabs(|tabs| tabs.borrow().iter().any(|tab| tab.cwd() == folder))
}

pub use own::{fill_menu as fill_own_menu, menu_choice as own_menu_choice};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// Opens an AI tab with the agent of the "+" menu at `index`.
pub fn open_preset(controller: &(impl AsWindow + ?Sized), index: usize) -> Option<Rc<AgentTab>> {
    let controller = &controller.as_window();
    let agent = AcpAgent::presets().into_iter().nth(index)?;
    open_agent(controller, Agent::Acp(agent), None)
}

/// Whether `agent` is installed and set up, as the menus of new tabs list only such agents: Claude's program is
/// there, an ACP agent starts, the own agent has a provider.
pub fn installed(agent: &Agent) -> bool {
    let env = &ide_ui_common::window::environment(std::time::Duration::ZERO)[..];
    match agent {
        Agent::Claude => claude_program(env).is_some(),
        Agent::Acp(acp) => matches!(
            availability(acp, env, std::path::Path::new("/")),
            Availability::Ready(_)
        ),
        Agent::Own => own::set_up(),
    }
}

/// The ACP agents of the "+" menu that are installed, with their place among all presets.
pub fn installed_presets() -> Vec<(usize, String)> {
    AcpAgent::presets()
        .into_iter()
        .enumerate()
        .filter(|(_, agent)| agent.offered() && installed(&Agent::Acp(agent.clone())))
        .map(|(index, agent)| (index, agent.name))
        .collect()
}

/// The agents of the "+" menu, and whether each can be opened: it is there or can be installed.
pub fn preset_menu() -> Vec<(String, bool)> {
    let env = &ide_ui_common::window::environment(std::time::Duration::ZERO)[..];
    let cwd = std::path::Path::new("/");
    AcpAgent::presets()
        .into_iter()
        .map(|agent| {
            let usable = !matches!(availability(&agent, env, cwd), Availability::Unavailable(_));
            (agent.name, usable)
        })
        .collect()
}

/// Claude's own program on the login shell's PATH in `env` or where its installer puts it (E21).
pub fn claude_program(env: &[(String, String)]) -> Option<std::path::PathBuf> {
    source::find_claude(env)
}

/// The tab with `key`, if it is open.
fn get(key: AgentKey) -> Option<Rc<AgentTab>> {
    with_tabs(|tabs| tabs.borrow().iter().find(|t| t.key == key).cloned())
}

/// The window of the AI tab with `key`, for a dictation whose composer closed.
#[cfg(feature = "dictation")]
pub(crate) fn controller_of(key: AgentKey) -> Option<Window> {
    get(key)?.controller()
}

/// Every open AI tab.
pub fn all() -> Vec<Rc<AgentTab>> {
    with_tabs(|tabs| tabs.borrow().clone())
}

/// The AI tabs of `controller`'s window.
pub fn of_window(controller: &(impl AsWindow + ?Sized)) -> Vec<Rc<AgentTab>> {
    let controller = &controller.as_window();
    all().into_iter().filter(|t| t.belongs_to(controller)).collect()
}

/// The open tab that shows the agent session `session`, in `controller`'s window.
pub(crate) fn find_session(controller: &(impl AsWindow + ?Sized), session: &str) -> Option<Rc<AgentTab>> {
    let controller = &controller.as_window();
    of_window(controller)
        .into_iter()
        .find(|t| t.session_id().as_deref() == Some(session))
}

/// Forgets the tab with `key` and ends its session.
fn remove(key: AgentKey) {
    let removed = with_tabs(|tabs| {
        let mut tabs = tabs.borrow_mut();
        let position = tabs.iter().position(|t| t.key == key)?;
        Some(tabs.remove(position))
    });
    if let Some(tab) = removed {
        tab.close();
        if let Some(controller) = tab.controller() {
            plan::close_docs(&controller, key);
            changed(&controller);
        }
    }
}

/// The AI tabs of `controller`'s window that work now, by title: a turn, a subagent or background work that runs.
pub fn running_in(controller: &(impl AsWindow + ?Sized)) -> Vec<String> {
    let controller = &controller.as_window();
    of_window(controller)
        .into_iter()
        .filter(|tab| works(tab))
        .map(|tab| tab.title())
        .collect()
}

/// The AI tabs of every window that work now, by title.
pub fn running() -> Vec<String> {
    all()
        .into_iter()
        .filter(|tab| works(tab))
        .map(|tab| tab.title())
        .collect()
}

fn works(tab: &AgentTab) -> bool {
    let thread = tab.view_snapshot();
    thread.status() != ide_thread::Status::Idle
        || thread.subagents().iter().any(|s| !s.status.is_final())
        || thread.background().iter().any(|b| !b.status.is_final() && !b.ambient)
}

/// Ends the sessions of a window that closes.
pub fn window_closed(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    for tab in of_window(controller) {
        remove(tab.key);
    }
}

/// Tells the AI panel and the activity view of `controller`'s window that sessions changed.
fn changed(controller: &Window) {
    crate::agent_panel::refresh(controller);
    controller.refresh_activity();
}

/// Catches up the selected tab after AppKit switches native windows or split documents.
pub fn show_pending(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    for tab in of_window(controller).iter().filter(|tab| tab.is_shown()) {
        tab.show_pending();
    }
}

/// Opens the list of `part` of the session in the AI tab the window shows.
pub fn show_work(controller: &(impl AsWindow + ?Sized), part: crate::work::Part) {
    let controller = &controller.as_window();
    if let Some(tab) = of_window(controller).into_iter().find(|t| t.is_shown()) {
        tab.show_work(part, None);
    }
}

/// Whether the window shows an AI tab whose list of `part` has anything to show.
pub fn can_show_work(controller: &(impl AsWindow + ?Sized), part: crate::work::Part) -> bool {
    let controller = &controller.as_window();
    of_window(controller)
        .into_iter()
        .find(|t| t.is_shown())
        .is_some_and(|t| t.has_work(part))
}

/// Moves the keyboard focus into the composer of the selected AI tab, or back out of it (⌘Esc); opens a tab when the
/// window has none.
pub fn toggle_focus(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    let tabs = of_window(controller);
    if let Some(focused) = tabs.iter().find(|t| t.composer_has_focus()) {
        focused.leave_focus();
        return;
    }
    let shown = tabs.iter().find(|t| t.is_shown()).or_else(|| tabs.last()).cloned();
    match shown {
        Some(tab) => {
            tab.select();
            tab.focus_composer();
        }
        None => {
            if let Some(tab) = open_tab(controller, None) {
                tab.focus_composer();
            }
        }
    }
}
