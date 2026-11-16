//! An AI tab on a session of the app's registry that it did not start, such as a work order's (B37b, E184): it
//! shows the session from its feed, wears the order's badge, keeps mode and sandbox as the order set them, and sends
//! what the user writes into the driver's queue. Closing it leaves the session running. A Claude tab's own session
//! in the registry reaches it through the same feed, taken in as its events were when the tab ran it itself.

use std::rc::Rc;

use ide_core::explain::Explained;
use ide_session::{SessionId, Signal, TurnId};
use ide_thread::Change;
use ide_ui_common::window::AsWindow;

use super::acp::Agent;
use super::driven::{Attach, Driven};
use super::source::{self, Source};
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

    /// The registry's session whose feed the tab takes in: one it follows, or its own Claude session.
    fn fed_session(&self) -> Option<SessionId> {
        let state = self.state.borrow();
        let source = state.source();
        source.driven().or_else(|| source.claude()).map(Driven::session)
    }

    /// Takes in what the session's feed holds for the tab: the snapshot first, then the changes since.
    pub(crate) fn take_feed(&self) {
        let own = self.state.borrow().source().claude().cloned();
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

    /// Takes in what the tab's own Claude session reported since the last time, as its events did when the tab ran
    /// the session itself.
    fn take_own_feed(&self, own: &Driven) {
        let Some(batch) = own.take() else {
            self.refresh();
            return;
        };
        if batch.changes.is_empty() && batch.signals.is_empty() {
            return;
        }
        let (mut learned, mut remote, mut problem, mut refused) = (false, false, None, None);
        for signal in batch.signals {
            match signal {
                Signal::Capabilities => learned = true,
                Signal::RemoteControl => remote = true,
                Signal::ClaudeFailed(ide_claude::SessionError::RewindRejected(why)) => refused = Some(why),
                Signal::ClaudeFailed(error) => problem = source::problem_of(&error),
                Signal::Delivered { turn, sent, checkpoint } => {
                    self.went_out(turn, sent.map_err(Explained), checkpoint)
                }
                Signal::CheckpointFinished { started, finished, .. } => {
                    self.state.borrow_mut().checkpoint_finished(&started, &finished);
                }
                Signal::CheckpointFailed { message, .. } => {
                    ide_ui_common::diag::report("checkpoint of a turn", message)
                }
                Signal::TurnEnded(end) => self.went_out(end.turn, Err(super::driven::gone()), None),
                Signal::Ended { .. } => {
                    let handed = self.state.borrow().handed_turn();
                    if let Some(turn) = handed {
                        self.went_out(turn, Err(super::driven::gone()), None);
                    }
                }
                _ => {}
            }
        }
        if let Some(why) = refused {
            self.rewind_refused(&why);
            return;
        }
        if learned {
            self.state.borrow_mut().learn();
        }
        if problem.is_some() {
            self.set_problem(problem);
        }
        self.absorb(batch.thread, &batch.changes);
        super::dialogs::requests_changed(self);
        if learned {
            self.refresh();
            super::dialogs::session_ready(self);
        } else if remote {
            self.refresh_remote();
        }
    }

    /// The message the driver took for `turn` went out with `checkpoint` taken before it, or did not: the agent
    /// refused it, or the turn or the session ended before it went out. A message that went out before is left alone.
    fn went_out(&self, turn: TurnId, sent: Result<Option<String>, Explained>, checkpoint: Option<ide_git::Checkpoint>) {
        let Some(message) = self.state.borrow_mut().take_handed(turn) else {
            return;
        };
        self.state.borrow_mut().delivering(&message);
        self.delivered(&message.text, sent, checkpoint);
    }
}
