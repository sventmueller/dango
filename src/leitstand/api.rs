//! What the Leitstand's views (B42) and the rest of the app use: the models of `ide_factory::panel` for a folder, the
//! user's commands, and a notice after every change. Everything runs on the main thread; a view reads the models
//! anew when its listener hears of its folder, or compares `revision` when it redraws on its own.
//!
//! ```text
//! let id = cx.leitstand().listen(move |root| view.refresh(root));
//! let panel = cx.leitstand().panel(&root);
//! cx.leitstand().command(&cx, &root, Command::Approve(order));
//! cx.leitstand().unlisten(id);
//! ```

use std::path::{Path, PathBuf};
use std::rc::Rc;

use ide_factory::event::BaseRef;
use ide_factory::options::{NewOrder, OptionChange};
use ide_factory::panel::{Board, Fleet, Panel};
use ide_factory::workflow::WorkflowError;
use ide_factory::workflow::rules::Loosening;
use ide_factory::{HunkId, InputKind, Order, OrderId, Refusal, RequestId, RestoreProblem, SessionEvent};

use super::{Delivery, GIT, Leitstand, git, order_name, store};
use crate::context::AppContext;

/// A listener's id, to stop listening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ListenerId(u64);

/// What the user can tell the Leitstand.
#[allow(dead_code, reason = "the Leitstand views of B42 send them")]
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Command {
    /// A new order from the sheet (E185); it starts at the working copy's branch and HEAD.
    New(NewOrder),
    /// G1: approve the specification.
    Approve(OrderId),
    /// Comment the specification for the planner.
    CommentSpec(OrderId, String),
    /// The user opened the order's diff (R70).
    DiffOpened(OrderId),
    /// G2: accept the evidence of `fingerprint`, without the `rejected` hunks (C55), named `path#index` in the diff
    /// of the order since its base.
    Accept {
        order: OrderId,
        fingerprint: String,
        rejected: Vec<HunkId>,
    },
    BackToBuilder(OrderId, String),
    /// New criteria, one per line (E184).
    ChangeCriteria(OrderId, String),
    ChangeOption(OrderId, OptionChange),
    ByHand(OrderId),
    BackToLeitstand(OrderId),
    Abort(OrderId),
    /// Resume one paused order, or all of them (E167).
    Resume(Option<OrderId>),
    /// Let a wait pass, such as a ref change or a changed agent configuration.
    Continue(OrderId),
    /// Answer a question of the order's session in the user's words.
    Answer {
        order: OrderId,
        request: RequestId,
        text: String,
    },
    /// Allow or refuse a permission request outside the order's bounds, which reaches the user without O1 (C72).
    Decide {
        order: OrderId,
        request: RequestId,
        allow: bool,
    },
    /// Confirm the gate command set of this hash (E190).
    ConfirmGates(String),
    /// Confirm a loosening the workflow file asks for (E190).
    ConfirmLoosening(Loosening),
}

#[allow(dead_code, reason = "the Leitstand views of B42 call these")]
impl Leitstand {
    /// Calls `listener` with a folder's root after each change of its orders, until `unlisten`.
    pub(crate) fn listen(&self, listener: impl Fn(&Path) + 'static) -> ListenerId {
        let mut listeners = self.listeners.borrow_mut();
        let id = ListenerId(listeners.last().map_or(1, |(id, _)| id.0 + 1));
        listeners.push((id, Rc::new(listener)));
        id
    }

    /// Stops calling the listener `id`.
    pub(crate) fn unlisten(&self, id: ListenerId) {
        self.listeners.borrow_mut().retain(|(own, _)| *own != id);
    }

    /// How often the orders of `root` changed so far; 0 while it has no factory.
    pub(crate) fn revision(&self, root: &Path) -> u64 {
        self.folders.borrow().get(root).map_or(0, |f| f.revision)
    }

    /// Whether `root` has a factory: it is trusted and a window showed it.
    pub(crate) fn has(&self, root: &Path) -> bool {
        self.folders.borrow().contains_key(root)
    }

    /// The panel's list of `root`'s orders, grouped by station (E175).
    pub(crate) fn panel(&self, root: &Path) -> Option<Panel> {
        let now = self.clock.now();
        self.folders.borrow().get(root).map(|f| Panel::new(&f.factory, now))
    }

    /// The Leitstand tab's station strip and table of `root`'s orders.
    pub(crate) fn board(&self, root: &Path) -> Option<Board> {
        let now = self.clock.now();
        self.folders.borrow().get(root).map(|f| Board::new(&f.factory, now))
    }

    /// The fleet line of `root`, with the quotas and the daily account across projects.
    pub(crate) fn fleet(&self, root: &Path) -> Option<Fleet> {
        let now = self.clock.now();
        let (_, day_start) = super::clock::local_day(now.wall);
        let shared = self.shared.borrow();
        self.folders
            .borrow()
            .get(root)
            .map(|f| Fleet::new(&f.factory, &shared.quotas, now, day_start, f.worktree_bytes))
    }

    /// What `read` gives of the order `order` of `root`: its log, evidence, criteria, sessions.
    pub(crate) fn with_order<R>(&self, root: &Path, order: &OrderId, read: impl FnOnce(&Order) -> R) -> Option<R> {
        self.folders.borrow().get(root)?.factory.order(order).map(read)
    }

    /// The errors of `root`'s workflow file; while there are any, no new station starts (E190).
    pub(crate) fn errors(&self, root: &Path) -> Vec<WorkflowError> {
        self.folders
            .borrow()
            .get(root)
            .map(|f| f.factory.errors().to_vec())
            .unwrap_or_default()
    }

    /// The loosenings `root`'s workflow file asks for that wait for the user's confirmation (E190).
    pub(crate) fn pending_loosenings(&self, root: &Path) -> Vec<Loosening> {
        self.folders
            .borrow()
            .get(root)
            .map(|f| f.factory.rules().pending().to_vec())
            .unwrap_or_default()
    }

    /// The hash of `root`'s gate command set and whether the user confirmed it (E190).
    pub(crate) fn gates(&self, root: &Path) -> Option<(String, bool)> {
        let folders = self.folders.borrow();
        let rules = folders.get(root)?.factory.rules();
        Some((rules.gate_hash().to_string(), rules.gates_confirmed()))
    }

    /// The commands the last step of `root` refused, with why.
    pub(crate) fn refusals(&self, root: &Path) -> Vec<Refusal> {
        self.folders
            .borrow()
            .get(root)
            .map(|f| f.refusals.clone())
            .unwrap_or_default()
    }

    /// What went wrong reading `root`'s order logs at the start.
    pub(crate) fn restore_problems(&self, root: &Path) -> Vec<RestoreProblem> {
        self.folders
            .borrow()
            .get(root)
            .map(|f| f.problems.clone())
            .unwrap_or_default()
    }

    /// Whether the Mac is kept awake for running orders (E169).
    pub(crate) fn holds_wake_lock(&self) -> bool {
        super::power::held(self)
    }

    /// Carries out the user's `command` for the orders of `root`; a refused one shows in `refusals`. A new order's id,
    /// which its first change names.
    pub(crate) fn command(&self, cx: &Rc<AppContext>, root: &Path, command: Command) -> Option<OrderId> {
        if !self.has(root) {
            return None;
        }
        let input = match command {
            Command::New(order) => return Some(self.new_order(root, order)),
            Command::Decide { order, request, allow } => return self.decide_request(cx, root, &order, request, allow),
            Command::ConfirmLoosening(loosening) => {
                self.confirm(root, loosening.hash());
                return None;
            }
            Command::Approve(order) => InputKind::Approve(order),
            Command::CommentSpec(order, text) => InputKind::CommentSpec(order, text),
            Command::DiffOpened(order) => InputKind::DiffOpened(order),
            Command::Accept {
                order,
                fingerprint,
                rejected,
            } => InputKind::Accept {
                order,
                fingerprint,
                rejected,
            },
            Command::BackToBuilder(order, text) => InputKind::BackToBuilder(order, text),
            Command::ChangeCriteria(order, text) => InputKind::ChangeCriteria(order, text),
            Command::ChangeOption(order, change) => InputKind::ChangeOption(order, change),
            Command::ByHand(order) => InputKind::ByHand(order),
            Command::BackToLeitstand(order) => InputKind::BackToLeitstand(order),
            Command::Abort(order) => InputKind::Abort(order),
            Command::Resume(order) => InputKind::Resume(order),
            Command::Continue(order) => InputKind::Continue(order),
            Command::Answer { order, request, text } => InputKind::Answer { order, request, text },
            Command::ConfirmGates(hash) => InputKind::ConfirmGates(hash),
        };
        self.input(cx, root, input);
        None
    }

    /// A new order's id, unique in the app's data folder and as a branch name.
    fn order_id(&self, root: &Path) -> OrderId {
        let project = self
            .folders
            .borrow()
            .get(root)
            .map(|f| f.project.clone())
            .unwrap_or_default();
        let mut n = self.clock.now().wall;
        loop {
            let id = OrderId(format!("o-{n}"));
            let taken = store::log_file(&project, &id).exists()
                || self
                    .folders
                    .borrow()
                    .get(root)
                    .is_some_and(|f| f.factory.order(&id).is_some());
            if !taken {
                return id;
            }
            n += 1;
        }
    }

    /// Reads the working copy's branch, HEAD and whether it has uncommitted changes, then hands the new order over.
    fn new_order(&self, root: &Path, order: NewOrder) -> OrderId {
        let id = self.order_id(root);
        let (folder, new) = (root.to_path_buf(), id.clone());
        let back = self.back();
        let queued = GIT.try_run(move || {
            let login = crate::environment::wait(std::time::Duration::from_secs(15)).unwrap_or_default();
            // Without a repository the order still comes in, and its worktree fails in words the user reads.
            let main = git::open(&folder, login).ok();
            let main = main.as_ref();
            let branch = main.and_then(|m| m.current_branch().ok().flatten()).unwrap_or_default();
            let commit = (main.and_then(|m| m.head().ok().flatten()))
                .map(|c| c.to_string())
                .unwrap_or_default();
            let options = ide_git::StatusOptions {
                untracked: ide_git::Untracked::No,
                ignored: false,
            };
            let dirty = main.is_some_and(|m| m.status(options).is_ok_and(|status| !status.entries.is_empty()));
            let input = InputKind::NewOrder {
                id: new,
                order,
                base: BaseRef { branch, commit },
                dirty,
            };
            back.deliver(Delivery::Input(folder, Box::new(input)));
        });
        if queued.is_err() {
            ide_core::diag::report("starting an order", "the lane took no more work");
        }
        id
    }

    fn decide_request(
        &self,
        cx: &Rc<AppContext>,
        root: &Path,
        order: &OrderId,
        request: RequestId,
        allow: bool,
    ) -> Option<OrderId> {
        let session = self.with_order(root, order, |o| {
            let role = o.turn().map(|t| t.role())?;
            o.session(role).map(|s| s.session().clone())
        })??;
        if self.decide(&session, &request.0, allow) {
            let answered = InputKind::Session(order.clone(), session, SessionEvent::Answered { request });
            self.input(cx, root, answered);
        }
        None
    }

    /// The orders of `root` whose sessions work now, by name.
    pub(crate) fn running_in(&self, root: &Path) -> Vec<String> {
        let sessions = self.sessions.borrow();
        let mut names: Vec<String> = sessions
            .working()
            .into_iter()
            .filter(|link| link.root == root)
            .map(|link| order_name(&link.title))
            .collect();
        names.dedup();
        names
    }

    /// Every order whose session works now, by name, for the question before quitting (E23, C58).
    pub(crate) fn running(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .sessions
            .borrow()
            .working()
            .into_iter()
            .map(|link| order_name(&link.title))
            .collect();
        names.dedup();
        names
    }

    /// Whether a session of an order works in `folder` or below it, such as its worktree.
    pub(crate) fn working_in(&self, folder: &Path) -> bool {
        self.sessions.borrow().works_in(folder)
    }

    /// The sessions of `root`'s orders that work now, for the activity view: name, role and key.
    pub(crate) fn activity(&self, root: &Path) -> Vec<(String, ide_factory::Role, ide_factory::SessionKey)> {
        self.sessions
            .borrow()
            .working()
            .into_iter()
            .filter(|link| link.root == root)
            .map(|link| (order_name(&link.title), link.role, link.key.clone()))
            .collect()
    }

    /// Stops the turn of the order's session `key`, from the activity view; the order then waits for the user.
    pub(crate) fn stop_turn(&self, key: &ide_factory::SessionKey) {
        if let Some(driver) = self.sessions.borrow_mut().driver(key) {
            driver.stop_turn();
        }
    }

    /// The roots whose factory runs, for a view that lists folders.
    pub(crate) fn folders(&self) -> Vec<PathBuf> {
        self.roots()
    }

    /// Tells the activity views of the windows of `root` that its sessions changed.
    pub(super) fn refresh_activity(&self, cx: &Rc<AppContext>, root: &Path) {
        for window in cx.windows().all() {
            if window.project_root().as_deref() == Some(root) {
                crate::activity::refresh(&window);
            }
        }
    }
}
