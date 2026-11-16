//! The self-test's hooks of the Leitstand: the scripted session its orders start instead of an agent's CLI, one for
//! every session or one per order title and role, the paths it reports as ready instead of looking for them (F61),
//! and what the self-test reads of the wake lock (R12).

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use ide_factory::Role;
use ide_factory::routing::{Agent, AgentStatus};

/// What the self-test set, and what it reads.
#[derive(Default)]
pub(crate) struct Probe {
    script: RefCell<Option<ide_session::Script>>,
    /// Scripts by order title and role; an empty title stands for every order.
    scripts: RefCell<BTreeMap<(String, Role), ide_session::Script>>,
    agents: RefCell<Option<BTreeMap<Agent, AgentStatus>>>,
    /// Whether a factory asked for the wake lock since the self-test started.
    wake_asked: Cell<bool>,
}

impl Probe {
    /// The script a new session of `role` for the order titled `title` plays, while one is set: its own, else the
    /// role's for every order, else the one for every session.
    pub(super) fn script_for(&self, title: &str, role: Role) -> Option<ide_session::Script> {
        let scripts = self.scripts.borrow();
        scripts
            .get(&(title.to_string(), role))
            .or_else(|| scripts.get(&(String::new(), role)))
            .cloned()
            .or_else(|| self.script.borrow().clone())
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

    /// Lets the sessions of each order title and role play their own script, an empty title standing for every order;
    /// an empty map lets them play the one of `use_script` again.
    pub(crate) fn use_scripts(&self, scripts: BTreeMap<(String, Role), ide_session::Script>) {
        *self.probe.scripts.borrow_mut() = scripts;
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

    /// Writes `logs`, each an order's id and its log's lines, where the factory of `root` reads them when it starts.
    pub(crate) fn seed_logs(
        &self,
        root: &std::path::Path,
        logs: &[(ide_factory::OrderId, String)],
    ) -> std::io::Result<()> {
        let project = super::store::project(&super::store::base(), root);
        for (order, text) in logs {
            let file = super::store::log_file(&project, order);
            if let Some(folder) = file.parent() {
                std::fs::create_dir_all(folder)?;
            }
            std::fs::write(file, text)?;
        }
        Ok(())
    }

    /// Whether a factory asked for the wake lock, and whether the app holds the activity that keeps the Mac awake.
    pub(crate) fn wake_lock(&self) -> (bool, bool) {
        let held = self.shared.borrow().power.holds();
        (self.probe.wake_asked.get(), held)
    }
}
