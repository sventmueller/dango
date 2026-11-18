//! The self-test's hooks of `attach.rs`: a work order's session that plays a script, started without the order's
//! stations, and the AI tab on it.

use std::collections::BTreeMap;
use std::path::Path;

use ide_factory::routing::Agent;
use ide_factory::{OrderId, SessionKey};
use ide_session::{AgentPath, Contract, Launch, Script};

use super::super::Link;
use super::*;

impl Leitstand {
    /// Starts a builder's session of the order `order` titled `title` in `folder`, the order's worktree, that plays
    /// `script`; its id.
    pub(crate) fn start_scripted(&self, folder: &Path, order: &str, title: &str, script: Script) -> Option<SessionId> {
        let contract = Contract::Order(ide_session::Order {
            order: order.to_string(),
            role: ide_session::Role::Builder,
            worktree: folder.to_path_buf(),
            build_dir: None,
            read_roots: Vec::new(),
            allowed_domains: Vec::new(),
            refuse_outside: false,
            frozen: None,
            repository: None,
            mcp_servers: Vec::new(),
        });
        let launch = Launch::new(AgentPath::Scripted(script), folder, Vec::new(), contract.clone());
        let mut sessions = self.sessions.borrow_mut();
        let registry = sessions.registry(&self.back());
        let id = registry
            .start(launch, ide_session::rule::for_contract(&contract))
            .ok()?;
        let subscriber = registry.get_mut(id)?.subscribe();
        let link = Link {
            root: folder.to_path_buf(),
            order: OrderId(order.to_string()),
            title: title.to_string(),
            key: SessionKey(format!("selftest-{id}")),
            role: ide_factory::Role::Builder,
            index: None,
            agent: Agent::Claude,
            subscriber,
            prompts: BTreeMap::new(),
            requests: BTreeMap::new(),
            started: false,
            resume: None,
            activity: None,
            usage: None,
            limits: Vec::new(),
        };
        sessions.links.insert(id, link);
        Some(id)
    }

    /// Opens the AI tab on session `id` in `controller`'s window, as "Sitzung öffnen" does for an order's session.
    pub(crate) fn open_session_tab(
        &self,
        cx: &Rc<AppContext>,
        controller: &crate::window::WindowController,
        id: SessionId,
    ) -> bool {
        let found = self.sessions.borrow().found(id);
        found.is_some_and(|found| self.attach_tab(cx, controller, found))
    }

    /// Whether session `id` still runs, and how many views follow its feed besides the runtime.
    pub(crate) fn session_followers(&self, id: SessionId) -> Option<(bool, usize)> {
        let sessions = self.sessions.borrow();
        let driver = sessions.registry.as_ref()?.get(id)?;
        Some((!driver.is_ended(), driver.subscribers().saturating_sub(1)))
    }

    /// Ends session `id` and lets it go, as the end of its order does.
    pub(crate) fn end_session(&self, id: SessionId) {
        let mut sessions = self.sessions.borrow_mut();
        sessions.links.remove(&id);
        if let Some(registry) = sessions.registry.as_mut() {
            registry.remove(id);
        }
    }
}
