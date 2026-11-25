//! An AI tab on a session of the app's registry that it did not start, such as a work order's (B37b, E184): it
//! shows the session from its feed, wears the order's badge, keeps mode and sandbox as the order set them, and sends
//! what the user writes into the driver's queue. Closing it leaves the session running. A tab's own session on any
//! path in the registry reaches it through the same feed, taken in as its events were when the tab ran it itself.

use std::rc::Rc;

use ide_core::explain::Explained;
use ide_session::{PathKind, SessionId, Signal, TurnId};
use ide_thread::Change;
use ide_ui_common::window::AsWindow;

use super::acp::Agent;
use super::driven::{Attach, Driven};
use super::source::{self, Problem, Source};
use super::tab::AgentTab;

/// Opens an AI tab on the running session `attach` names in `controller`'s window, or brings the window's tab on it
/// to the front; `None` when the session is gone or has ended. `state` holds the AI tabs.
pub fn attach(controller: &(impl AsWindow + ?Sized), state: &crate::State, attach: Attach) -> Option<Rc<AgentTab>> {
    let controller = &controller.as_window();
    let open = (state.agent_tabs.tabs.borrow().iter())
        .find(|tab| tab.belongs_to(controller) && tab.driven_session() == Some(attach.session))
        .cloned();
    if let Some(tab) = open {
        tab.select();
        return Some(tab);
    }
    let driven = Driven::subscribe(&attach)?;
    let Some(thread) = driven.snapshot() else {
        driven.leave();
        return None;
    };
    let cwd = thread.cwd().to_path_buf();
    let badge = driven.badge().to_string();
    let source = Source::Driven(driven);
    let tab = AgentTab::shared(AgentTab::build(controller, state, Agent::Claude, cwd, thread, source));
    {
        let mut tab_state = tab.state.borrow_mut();
        tab_state.set_custom_title(attach.title);
        if !tab_state.sandbox() {
            tab_state.toggle_sandbox();
        }
    }
    tab.header.set_badge(Some(&badge));
    tab.add_to_window(controller);
    state.agent_tabs.tabs.borrow_mut().push(tab.clone());
    tab.take_feed();
    super::changed(controller);
    tab.select();
    Some(tab)
}

/// Hands the AI tabs on `session` what its driver published for them, after the runtime's flush of it; a tab on a
/// session the registry let go shows it as ended.
pub fn session_fed(state: &crate::State, session: SessionId) {
    let tabs: Vec<Rc<AgentTab>> = (state.agent_tabs.tabs.borrow().iter())
        .filter(|tab| tab.fed_session() == Some(session))
        .cloned()
        .collect();
    for tab in tabs {
        tab.take_feed();
    }
}

impl AgentTab {
    /// Whether the tab follows a session of the registry it did not start; it stays so after the session ended.
    pub(crate) fn is_driven(&self) -> bool {
        self.state.borrow().source().driven().is_some()
    }

    /// The registry's session the tab follows.
    pub(crate) fn driven_session(&self) -> Option<SessionId> {
        self.state.borrow().source().driven().map(Driven::session)
    }

    /// The registry's session whose feed the tab takes in: one it follows, or its own.
    fn fed_session(&self) -> Option<SessionId> {
        let state = self.state.borrow();
        let source = state.source();
        source.driven().or_else(|| source.tab_session()).map(Driven::session)
    }

    /// Takes in what the session's feed holds for the tab: the snapshot first, then the changes since.
    pub(crate) fn take_feed(&self) {
        let own = self.state.borrow().source().tab_session().cloned();
        if let Some(own) = own {
            return self.take_own_feed(&own);
        }
        let Some(driven) = self.state.borrow().source().driven().cloned() else {
            return;
        };
        let Some(batch) = driven.take() else {
            self.refresh();
            return;
        };
        let learned = (batch.signals.iter()).any(|signal| matches!(signal, Signal::Ready | Signal::Capabilities));
        let ended = (batch.signals.iter()).any(|signal| matches!(signal, Signal::Ended { .. }));
        for signal in &batch.signals {
            match signal {
                Signal::Failed(why) => ide_ui_common::diag::report("an order's session", why),
                Signal::AnswerFailed { message, .. } => ide_ui_common::diag::report("answering a request", message),
                _ => {}
            }
        }
        if learned {
            self.state.borrow_mut().learn();
        }
        let changes: &[Change] = &batch.changes;
        self.absorb(batch.thread, changes);
        super::dialogs::requests_changed(self);
        if learned || ended {
            self.refresh();
        }
        if learned {
            super::dialogs::session_ready(self);
        }
    }

    /// Takes in what the tab's own session reported since the last time, as its events did when the tab ran the
    /// session itself.
    fn take_own_feed(&self, own: &Driven) {
        let Some(batch) = own.take() else {
            self.refresh();
            return;
        };
        if batch.changes.is_empty() && batch.signals.is_empty() {
            return;
        }
        let mut heard = Heard::default();
        for signal in batch.signals {
            self.hear(signal, &mut heard);
        }
        match own.kind() {
            PathKind::Claude | PathKind::Replay => self.take_claude(heard, batch.thread, &batch.changes),
            _ => self.take_other(heard, batch.thread, &batch.changes),
        }
    }

    /// Notes in `heard` what `signal` of the tab's own session means, and hands on at once what concerns the message
    /// the driver took.
    fn hear(&self, signal: Signal, heard: &mut Heard) {
        match signal {
            Signal::Capabilities => heard.learned = true,
            Signal::RemoteControl => heard.remote = true,
            Signal::ClaudeFailed(ide_claude::SessionError::RewindRejected(why)) => heard.refused = Some(why),
            Signal::ClaudeFailed(error) => heard.problem = source::problem_of(&error),
            Signal::CodexFailed(error) => heard.problem = Some(Problem::Start(crate::codex::explained(&error))),
            Signal::OpenCodeFailed(error) => heard.problem = Some(Problem::Start(crate::opencode::explained(&error))),
            Signal::AcpFailed(error) => heard.problem = self.acp_problem(&error),
            Signal::Failed(why) if self.agent.is_own() => ide_ui_common::diag::report("the own agent", why),
            Signal::RuleAdded(rule) => crate::own_agent::add_rule(&self.cwd, rule),
            Signal::Delivered { turn, sent, checkpoint } => self.went_out(turn, sent.map_err(Explained), checkpoint),
            Signal::CheckpointFinished { started, finished, .. } => {
                self.state.borrow_mut().checkpoint_finished(&started, &finished);
            }
            Signal::CheckpointFailed { message, .. } => ide_ui_common::diag::report("checkpoint of a turn", message),
            Signal::TurnEnded(end) => self.went_out(end.turn, Err(super::driven::gone()), None),
            Signal::Ended { stderr } => {
                let handed = self.state.borrow().handed_turn();
                if let Some(turn) = handed {
                    self.went_out(turn, Err(super::driven::gone()), None);
                }
                heard.ended = Some(stderr);
            }
            _ => {}
        }
    }

    /// Takes in what the tab's own Claude session, or the recording that plays instead, reported.
    fn take_claude(&self, heard: Heard, thread: std::sync::Arc<ide_thread::Thread>, changes: &[Change]) {
        if let Some(why) = heard.refused {
            self.rewind_refused(&why);
            return;
        }
        if heard.learned {
            self.state.borrow_mut().learn();
        }
        if heard.problem.is_some() {
            self.set_problem(heard.problem);
        }
        self.absorb(thread, changes);
        super::dialogs::requests_changed(self);
        if heard.learned {
            self.refresh();
            super::dialogs::session_ready(self);
        } else if heard.remote {
            self.refresh_remote();
        }
    }

    /// Takes in what the tab's own session on another path than Claude reported: Codex, OpenCode, ACP or the own
    /// agent.
    fn take_other(&self, heard: Heard, thread: std::sync::Arc<ide_thread::Thread>, changes: &[Change]) {
        if heard.learned {
            self.state.borrow_mut().learn();
            let codex = self.state.borrow().source().codex();
            if let Some(session) = codex {
                crate::codex::remember_threads(&session, &self.cwd);
            }
            if self.agent.is_acp() {
                self.acp_apply_choice();
                self.send_queued();
            }
        }
        if heard.problem.is_some() {
            self.set_problem(heard.problem);
        }
        self.absorb(thread, changes);
        super::dialogs::requests_changed(self);
        if heard.remote {
            self.codex_remote_heard();
        }
        if heard.learned {
            self.refresh();
            super::dialogs::session_ready(self);
        } else if heard.remote {
            self.refresh_remote();
        }
        self.codex_move_if_idle();
        if let Some(stderr) = heard.ended {
            self.acp_ended(&stderr);
        }
    }

    /// The message the driver took for `turn` went out with `checkpoint` taken before it, or did not: the agent
    /// refused it, or the turn or the session ended before it went out. A message that went out before is left alone.
    /// A message of an agent that gives prompts no id keeps the app's own, which the checkpoint is kept under (I6).
    fn went_out(&self, turn: TurnId, sent: Result<Option<String>, Explained>, checkpoint: Option<ide_git::Checkpoint>) {
        let Some(message) = self.state.borrow_mut().take_handed(turn) else {
            return;
        };
        let app_id = message
            .message_id
            .clone()
            .filter(|_| super::actions::app_ids(&self.agent));
        if sent.is_err() {
            self.not_sent(&message);
        }
        self.state.borrow_mut().delivering(&message);
        self.delivered(&message.text, sent.map(|id| id.or(app_id)), checkpoint);
    }
}

/// What the signals of one batch of the tab's own session said, beyond what is handed on at once.
#[derive(Default)]
struct Heard {
    /// What the agent offers changed.
    learned: bool,
    /// Remote Control changed.
    remote: bool,
    /// What keeps the session from working.
    problem: Option<Problem>,
    /// Why Claude refused to rewind.
    refused: Option<String>,
    /// The end of the agent's error output, once its process ended.
    ended: Option<String>,
}
