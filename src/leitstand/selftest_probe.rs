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

    /// What the self-test sees of the wake lock (E169, R12).
    pub(crate) fn wake_lock(&self) -> WakeLock {
        let shared = self.shared.borrow();
        WakeLock {
            asked: self.probe.wake_asked.get(),
            requested: shared.power.requested(),
            activity: shared.power.holds(),
        }
    }

    /// Tells every folder that the Mac goes to sleep or woke up, as the workspace's notifications do (E168).
    pub(crate) fn simulate_sleep(&self, cx: &std::rc::Rc<crate::context::AppContext>, asleep: bool) {
        self.sleep(cx, asleep);
    }

    /// The wall clock when this run of the app started.
    pub(crate) fn started_at(&self) -> u64 {
        self.clock.run()
    }

    /// The ids of the orders of `root`'s factory.
    pub(crate) fn order_ids(&self, root: &std::path::Path) -> Vec<ide_factory::OrderId> {
        self.folders
            .borrow()
            .get(root)
            .map(|f| f.factory.orders().map(|o| o.id().clone()).collect())
            .unwrap_or_default()
    }

    /// The entries of the log of `order` of `root` as they are on disk now.
    pub(crate) fn order_log(
        &self,
        root: &std::path::Path,
        order: &ide_factory::OrderId,
    ) -> Vec<ide_factory::log::LogEntry> {
        let project = super::store::project(&super::store::base(), root);
        std::fs::read_to_string(super::store::log_file(&project, order))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
}

/// What the self-test sees of the wake lock.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WakeLock {
    /// A factory asked to hold it since the self-test started.
    pub(crate) asked: bool,
    /// A factory holds it now.
    pub(crate) requested: bool,
    /// The activity that keeps the Mac awake is held, which a self-test never does.
    pub(crate) activity: bool,
}
