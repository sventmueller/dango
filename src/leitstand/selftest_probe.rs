//! The self-test's hooks of the Leitstand: the scripted session its orders start instead of an agent's CLI and the
//! paths it reports as ready instead of looking for them (F61), and what the self-test reads of the wake lock (R12).

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use ide_factory::routing::{Agent, AgentStatus};

/// What the self-test set, and what it reads.
#[derive(Default)]
pub(crate) struct Probe {
    script: RefCell<Option<ide_session::Script>>,
    agents: RefCell<Option<BTreeMap<Agent, AgentStatus>>>,
    /// Whether a factory asked for the wake lock since the self-test started.
    wake_asked: Cell<bool>,
}

impl Probe {
    /// The script every new session of an order plays, while one is set.
    pub(super) fn script(&self) -> Option<ide_session::Script> {
        self.script.borrow().clone()
    }

    /// The paths a folder's factory hears of, while the self-test sets them.
    pub(super) fn agents(&self) -> Option<BTreeMap<Agent, AgentStatus>> {
        self.agents.borrow().clone()
    }

    /// A factory asked to hold the wake lock, or to let it go.
    pub(super) fn note_wake(&self, hold: bool) {
        if hold {
            self.wake_asked.set(true);
        }
    }
}

#[allow(dead_code, reason = "the self-test scenarios of B42 set it")]
impl super::Leitstand {
    /// Lets every session an order starts from now on play `script`, or start the agent's CLI again with `None`.
    pub(crate) fn use_script(&self, script: Option<ide_session::Script>) {
        *self.probe.script.borrow_mut() = script;
    }

    /// Lets every folder whose factory starts from now on hear of `agents` as the paths there are, or look for them
    /// again with `None`.
    pub(crate) fn use_agents(&self, agents: Option<BTreeMap<Agent, AgentStatus>>) {
        *self.probe.agents.borrow_mut() = agents;
    }

    /// Whether the factory of `root` has gate commands, once it read the folder's workflow file.
    pub(crate) fn has_gates(&self, root: &std::path::Path) -> bool {
        self.folders
            .borrow()
            .get(root)
            .is_some_and(|f| f.factory.rules().has_gates())
    }

    /// Whether a factory asked for the wake lock, and whether the app holds the activity that keeps the Mac awake.
    pub(crate) fn wake_lock(&self) -> (bool, bool) {
        let held = self.shared.borrow().power.holds();
        (self.probe.wake_asked.get(), held)
    }
}
