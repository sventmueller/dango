//! The AI tab: one agent session per tab with its timeline, composer and status, as the official Claude Code
//! extension shows it, drawn natively. Claude runs through `ide_claude::Session`, other agents over ACP through
//! `ide_acp::Session`; the self-test feeds recordings through `ide_claude::replay::Replay` instead.

mod acp;
mod acp_words;
mod actions;
mod attrs;
mod cards;
mod checkpoint;
mod choices;
mod compose;
mod composer;
mod context;
mod context_view;
mod enlarge;
mod figures;
mod format;
mod header;
mod inbox;
mod markdown;
mod mention;
mod notify;
mod outside;
mod overlay;
mod own;
pub mod plan;
mod refresh;
mod rewind;
mod row;
mod source;
mod status;
pub mod style;
mod suggest;
mod tab;
mod target;
mod text;
mod timeline;
mod ui;
mod usage;

use std::cell::RefCell;
use std::rc::Rc;

use crate::window::WindowController;
pub use acp::{AcpAgent, Agent, Availability, agent_name, availability, cached_register, confirm_install, launch_of};
#[cfg(feature = "selftest")]
pub use acp::{set_test_launch, set_test_register};
pub use actions::ask_text;
#[cfg(feature = "selftest")]
pub use actions::{alerts_seen, answer_alerts};
#[cfg(feature = "selftest")]
pub use cards::window_name;
#[cfg(feature = "selftest")]
pub use context_view::{click as click_context, close as close_context, cost_text, seen as context_seen, token_text};
#[cfg(feature = "selftest")]
pub use figures::{asked_for as asked_for_figure, is_dark};
#[cfg(feature = "selftest")]
pub use format::percent;
pub use format::{age, elapsed, iso_time, time_of_day, tokens};
pub use outside::ci_log;
pub use source::Resume;
pub use tab::AgentTab;
#[cfg(feature = "selftest")]
pub(crate) use tab::{INPUT_INSET, INPUT_WIDTH, SIDE_MARGIN};
pub use ui::{framed, icon, insets, separator};
#[cfg(feature = "selftest")]
pub use usage::inject as inject_usage;

thread_local! {
    /// Every open AI tab, in the order they were opened; a tab leaves when it is closed.
    static TABS: RefCell<Vec<Rc<AgentTab>>> = const { RefCell::new(Vec::new()) };
}

/// Opens an AI tab with a new Claude session in the window's folder, or with the past session `resume`.
pub fn open_tab(controller: &WindowController, resume: Option<Resume>) -> Option<Rc<AgentTab>> {
    open_agent(controller, Agent::Claude, resume)
}

/// Opens an AI tab with a new session of `agent` in the window's folder, or with its past session `resume`.
pub fn open_agent(controller: &WindowController, agent: Agent, resume: Option<Resume>) -> Option<Rc<AgentTab>> {
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

fn started(controller: &WindowController, tab: &Rc<AgentTab>) {
    TABS.with(|tabs| tabs.borrow_mut().push(tab.clone()));
    tab.start();
    changed(controller);
}

/// Opens an AI tab with the app's own agent (E6), talking to `choice` or else to the model new threads start with.
pub fn open_own(controller: &WindowController, choice: Option<crate::own_agent::ModelChoice>) -> Option<Rc<AgentTab>> {
    let tab = AgentTab::open(controller, Agent::Own, None)?;
    tab.state.borrow_mut().own = choice;
    started(controller, &tab);
    Some(tab)
}

/// Opens an AI tab with a new session of `agent` that works in `cwd`, such as a thread's worktree (I1), titled
/// `title`.
pub fn open_agent_in(
    controller: &WindowController,
    agent: Agent,
    cwd: std::path::PathBuf,
    title: Option<&str>,
) -> Option<Rc<AgentTab>> {
    let tab = AgentTab::open_in(controller, agent, cwd, None)?;
    if let Some(title) = title {
        tab.set_custom_title(title);
    }
    TABS.with(|tabs| tabs.borrow_mut().push(tab.clone()));
    tab.start();
    changed(controller);
    Some(tab)
}

/// Whether an open AI tab works in `folder`.
pub fn working_in(folder: &std::path::Path) -> bool {
    TABS.with(|tabs| tabs.borrow().iter().any(|tab| tab.cwd() == folder))
}

pub use own::{fill_menu as fill_own_menu, menu_choice as own_menu_choice};

/// Opens an AI tab with the agent of the "+" menu at `index`.
pub fn open_preset(controller: &WindowController, index: usize) -> Option<Rc<AgentTab>> {
    let agent = AcpAgent::presets().into_iter().nth(index)?;
    open_agent(controller, Agent::Acp(agent), None)
}

/// Whether `agent` is installed and set up, as the menus of new tabs list only such agents: Claude's program is
/// there, an ACP agent starts, the own agent has a provider.
pub fn installed(agent: &Agent) -> bool {
    let env = crate::environment::get().unwrap_or_default();
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
    let env = crate::environment::get().unwrap_or_default();
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
fn get(key: u64) -> Option<Rc<AgentTab>> {
    TABS.with(|tabs| tabs.borrow().iter().find(|t| t.key == key).cloned())
}

/// The window of the AI tab with `key`, for a dictation whose composer closed.
#[cfg(feature = "dictation")]
pub fn controller_of(key: u64) -> Option<objc2::rc::Retained<crate::window::WindowController>> {
    get(key)?.controller()
}

/// An answer's markdown as its card sets it, for the self-test.
#[cfg(feature = "selftest")]
pub fn answer_text(
    mtm: objc2::MainThreadMarker,
    markdown: &str,
) -> objc2::rc::Retained<objc2_foundation::NSAttributedString> {
    markdown::render_with_figures(mtm, markdown, style::BODY, markdown::Tone::Primary, 500.0)
}

#[cfg(feature = "selftest")]
pub fn answer_height(mtm: objc2::MainThreadMarker, markdown: &str, width: f64) -> f64 {
    use objc2_app_kit::NSView;
    let rendered = markdown::render_with_figures(mtm, markdown, style::BODY, markdown::Tone::Primary, width);
    let text = text::CardText::new(mtm, &rendered, width, true);
    let view: &NSView = &text;
    view.intrinsicContentSize().height
}

/// Every open AI tab.
pub fn all() -> Vec<Rc<AgentTab>> {
    TABS.with(|tabs| tabs.borrow().clone())
}

/// The AI tabs of `controller`'s window.
pub fn of_window(controller: &WindowController) -> Vec<Rc<AgentTab>> {
    all().into_iter().filter(|t| t.belongs_to(controller)).collect()
}

/// The open tab that shows the agent session `session`, in `controller`'s window.
pub fn find_session(controller: &WindowController, session: &str) -> Option<Rc<AgentTab>> {
    of_window(controller)
        .into_iter()
        .find(|t| t.session_id().as_deref() == Some(session))
}

/// Forgets the tab with `key` and ends its session.
fn remove(key: u64) {
    let removed = TABS.with(|tabs| {
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
pub fn running_in(controller: &WindowController) -> Vec<String> {
    of_window(controller)
        .into_iter()
        .filter(|tab| {
            let thread = tab.view_snapshot();
            thread.status() != ide_thread::Status::Idle
                || thread.subagents().iter().any(|s| !s.status.is_final())
                || thread.background().iter().any(|b| !b.status.is_final() && !b.ambient)
        })
        .map(|tab| tab.title())
        .collect()
}

/// Ends the sessions of a window that closes.
pub fn window_closed(controller: &WindowController) {
    for tab in of_window(controller) {
        remove(tab.key);
    }
}

/// Tells the AI panel and the activity view of `controller`'s window that sessions changed.
fn changed(controller: &WindowController) {
    crate::agent_panel::refresh(controller);
    crate::activity::refresh(controller);
}

/// Catches up the selected tab after AppKit switches native windows or split documents.
pub fn show_pending(controller: &WindowController) {
    for tab in of_window(controller).iter().filter(|tab| tab.is_shown()) {
        tab.show_pending();
    }
}

/// Opens the list of `part` of the session in the AI tab the window shows.
pub fn show_work(controller: &WindowController, part: crate::activity::Part) {
    if let Some(tab) = of_window(controller).into_iter().find(|t| t.is_shown()) {
        tab.show_work(part, None);
    }
}

/// Whether the window shows an AI tab whose list of `part` has anything to show.
pub fn can_show_work(controller: &WindowController, part: crate::activity::Part) -> bool {
    of_window(controller)
        .into_iter()
        .find(|t| t.is_shown())
        .is_some_and(|t| t.has_work(part))
}

/// Moves the keyboard focus into the composer of the selected AI tab, or back out of it (⌘Esc); opens a tab when the
/// window has none.
pub fn toggle_focus(controller: &WindowController) {
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

#[cfg(feature = "selftest")]
pub use cards::Part;
#[cfg(feature = "selftest")]
pub use cards::notice_text;
#[cfg(feature = "selftest")]
pub use cards::request::ALLOW_ONCE;

/// Whether `@terminal` finds a terminal's output in `controller`'s window.
#[cfg(feature = "selftest")]
pub fn terminal_context(controller: &WindowController) -> bool {
    context::terminal(controller)
        .is_some_and(|a| matches!(a, ide_thread::Attachment::External { text, .. } if !text.is_empty()))
}

/// What `@browser` hands over from `controller`'s window, as outside text, given to `done`.
#[cfg(feature = "selftest")]
pub fn browser_context(controller: &WindowController, done: impl FnOnce(Option<String>) + 'static) {
    context::browser(controller, move |page| {
        done(page.and_then(|a| match a {
            ide_thread::Attachment::External { text, .. } => Some(text),
            _ => None,
        }));
    });
}

/// How many notifications sessions posted, or would have where posting is not allowed.
#[cfg(feature = "selftest")]
pub fn notifications_posted() -> usize {
    notify::posted()
}

/// Opens an AI tab in `cwd` titled `title` that plays recordings instead of running `claude`, for the self-test.
#[cfg(feature = "selftest")]
pub fn open_replay_in(controller: &WindowController, cwd: std::path::PathBuf, title: &str) -> Rc<AgentTab> {
    let tab = AgentTab::open_replay_in(controller, cwd);
    tab.set_custom_title(title);
    TABS.with(|tabs| tabs.borrow_mut().push(tab.clone()));
    changed(controller);
    tab
}

/// Opens an AI tab that plays recordings instead of running `claude`, for the self-test.
#[cfg(feature = "selftest")]
pub fn open_replay(controller: &WindowController) -> Option<Rc<AgentTab>> {
    let tab = AgentTab::open_replay(controller)?;
    TABS.with(|tabs| tabs.borrow_mut().push(tab.clone()));
    changed(controller);
    Some(tab)
}
