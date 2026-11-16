//! An AI tab on a session of the app's registry that it did not start, such as a work order's (B37b, E184): it
//! shows the session from its feed, wears the order's badge, keeps mode and sandbox as the order set them, and sends
//! what the user writes into the driver's queue. Closing it leaves the session running.

use std::rc::Rc;

use ide_session::{SessionId, Signal};
use ide_thread::Change;
use ide_ui_common::window::AsWindow;

use super::acp::Agent;
use super::driven::{Attach, Driven};
use super::source::Source;
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
    let (cwd, thread_id) = (thread.cwd().to_path_buf(), thread.id().clone());
    let badge = driven.badge().to_string();
    let source = Source::Driven(driven);
    let tab = AgentTab::shared(AgentTab::build(
        controller,
        Agent::Claude,
        cwd,
        thread_id,
        thread,
        source,
    ));
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
        .filter(|tab| tab.driven_session() == Some(session))
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

    /// Takes in what the session's feed holds for the tab: the snapshot first, then the changes since.
    pub(crate) fn take_feed(&self) {
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
}
