//! The Leitstand's runtime (docs/phase-13.md, B37a–B45a): one `ide_factory::Factory` per trusted project folder in the
//! app context, since orders belong to the folder and not to a window (E166). Every event becomes an `Input` with both
//! clocks; the step's log entries go to each order's log on the disk lane before any of its effects run (E167), and
//! the effects run on the app's lanes, their results coming back to the main thread by id (E142, E143). Sessions run
//! through one `ide_session::Registry` (E165). The views of B42 read and command it through the functions in `api`.

mod api;
mod clock;
mod effects;
mod freeze;
mod gates;
mod git;
mod lifecycle;
mod manifest;
mod notices;
mod power;
mod rules;
#[cfg(feature = "selftest")]
mod selftest_probe;
mod sessions;
mod store;

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

use dispatch2::MainThreadBound;
use ide_core::lane::Lane;
use ide_core::settings::{Key, Sharing};
use ide_factory::workflow::UserDefaults;
use ide_factory::workflow::rules::Confirmations;
use ide_factory::{Effect, Factory, Input, InputKind, OrderId, Refusal, RestoreProblem, Step};
use objc2::MainThreadMarker;

use crate::context::AppContext;
#[allow(unused_imports, reason = "the Leitstand views of B42 use them")]
pub(crate) use api::{Command, ListenerId};

/// The user's defaults for orders in every project (E183), kept on this Mac (E113).
pub(crate) const DEFAULTS: Key<UserDefaults> = Key::user("factoryDefaults", Sharing::PerMac, UserDefaults::default);

/// The orchestrator's git, one call after the other, so a ref snapshot never sees half of another order's change.
static GIT: Lane = Lane::with_start("ide-orders", 1, 256, ide_process::priority::utility);

/// How often running orders are looked at for their limits (E168).
const TICK: Duration = Duration::from_secs(1);

/// How many ticks pass between two readings of the workflow file, which a `git pull` may change (E190).
const RULES_EVERY: u64 = 10;

/// What happened off the main thread, or later on it, for the Leitstand to take in by ids (E143).
enum Delivery {
    /// An input for the factory of `root`.
    Input(PathBuf, Box<InputKind>),
    /// The log entries of a step were written, but not those of `failed`; the step's effects may run now.
    Logged {
        root: PathBuf,
        effects: Vec<Effect>,
        failed: Vec<OrderId>,
    },
    /// What a folder needs to start: its order logs and confirmations, read from disk.
    Loaded {
        root: PathBuf,
        logs: Vec<String>,
        confirmations: Confirmations,
    },
    /// A session's driver has something to take in (E165).
    Flush(ide_session::SessionId),
    /// A session can be launched now that its program was looked for.
    Launch(Box<sessions::Ready>),
    /// The main repository's refs before a turn, for the ref watch (E173).
    Refs(PathBuf, OrderId, ide_git::RefSnapshot),
    /// The space the order worktrees of `root` take (R68).
    Space(PathBuf, u64),
    /// Time passed.
    Tick,
    /// The user's defaults changed.
    Settings,
    /// The Mac sleeps, or woke up (E168).
    Sleep(bool),
}

/// What a view calls after each change of a folder's orders.
type Listener = Rc<dyn Fn(&Path)>;

/// A folder's factory with what the app keeps beside it.
struct Folder {
    factory: Factory,
    /// The folder of what the Leitstand keeps for it.
    project: PathBuf,
    confirmations: Confirmations,
    /// Raised after every step, for views that compare.
    revision: u64,
    /// The user commands the last step refused.
    refusals: Vec<Refusal>,
    /// What went wrong reading the logs at the start.
    problems: Vec<RestoreProblem>,
    work: effects::Work,
    worktree_bytes: u64,
}

/// Every folder's factory, the sessions of their orders, and what holds across folders.
#[derive(Default)]
pub(crate) struct Leitstand {
    folders: RefCell<BTreeMap<PathBuf, Folder>>,
    /// Folders whose logs are being read.
    loading: RefCell<Vec<PathBuf>>,
    sessions: RefCell<sessions::Sessions>,
    shared: RefCell<lifecycle::Shared>,
    clock: clock::Clock,
    queue: RefCell<VecDeque<(PathBuf, InputKind)>>,
    draining: Cell<bool>,
    /// Steps whose log entries are on their way to disk; effects of later steps wait behind them.
    writing: Cell<usize>,
    listeners: RefCell<Vec<(ListenerId, Listener)>>,
    back: OnceCell<Back>,
    #[cfg(feature = "selftest")]
    probe: selftest_probe::Probe,
}

/// The way back from other threads to the app context on the main thread (E142, E143): work the Leitstand starts
/// carries it instead of looking the context up when its answer arrives. The Leitstand keeps one for as long as the
/// app runs, so the last one never drops on another thread.
#[derive(Clone)]
pub(crate) struct Back(Arc<MainThreadBound<Weak<AppContext>>>);

impl Back {
    /// Has `delivery` taken in on the main thread when its turn comes.
    fn deliver(&self, delivery: Delivery) {
        let back = self.clone();
        crate::util::main_soon(move || back.take(delivery));
    }

    /// `deliver` after `delay`.
    fn deliver_after(&self, delay: Duration, delivery: Delivery) {
        let back = self.clone();
        crate::util::main_after(delay, move || back.take(delivery));
    }

    fn take(&self, delivery: Delivery) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if let Some(cx) = self.0.get(mtm).upgrade() {
            cx.leitstand().take(&cx, delivery);
        }
    }

    /// Tells the Leitstand that the user's settings `names` were written, from any thread.
    pub(crate) fn settings_changed(&self, names: &[String]) {
        if names.iter().any(|name| name == DEFAULTS.declared().name) {
            self.deliver(Delivery::Settings);
        }
    }
}

impl Leitstand {
    /// Binds the Leitstand to the context that holds it, once, as the app starts.
    pub(crate) fn bind(&self, cx: &Rc<AppContext>, mtm: MainThreadMarker) {
        let _ = self
            .back
            .set(Back(Arc::new(MainThreadBound::new(Rc::downgrade(cx), mtm))));
    }

    /// The way back to the main thread for work that starts now.
    pub(crate) fn back(&self) -> Back {
        self.back
            .get()
            .cloned()
            .expect("the app delegate binds the Leitstand before any window opens")
    }

    fn take(&self, cx: &Rc<AppContext>, delivery: Delivery) {
        match delivery {
            Delivery::Input(root, kind) => match *kind {
                InputKind::Rules(rules) if !self.rules_differ(&root, &rules) => {}
                kind => self.input(cx, &root, kind),
            },
            Delivery::Logged { root, effects, failed } => {
                self.writing.set(self.writing.get().saturating_sub(1));
                self.run_effects(cx, &root, effects, &failed);
            }
            Delivery::Loaded {
                root,
                logs,
                confirmations,
            } => self.loaded(cx, root, &logs, confirmations),
            Delivery::Flush(session) => self.flush(cx, session),
            Delivery::Launch(ready) => self.launch(cx, *ready),
            Delivery::Refs(root, order, snapshot) => {
                if let Some(folder) = self.folders.borrow_mut().get_mut(&root) {
                    folder.work.snapshots.insert(order, snapshot);
                }
            }
            Delivery::Space(root, bytes) => {
                if let Some(folder) = self.folders.borrow_mut().get_mut(&root) {
                    folder.worktree_bytes = bytes;
                }
                self.changed(&root);
            }
            Delivery::Tick => self.tick(cx),
            Delivery::Settings => self.reload_all_rules(),
            Delivery::Sleep(asleep) => self.sleep(cx, asleep),
        }
    }

    /// Hands `kind` to the factory of `root`, after the inputs that wait already.
    fn input(&self, cx: &Rc<AppContext>, root: &Path, kind: InputKind) {
        self.queue.borrow_mut().push_back((root.to_path_buf(), kind));
        if self.draining.get() {
            return;
        }
        let _draining = crate::util::FlagGuard::raise(&self.draining);
        loop {
            let next = self.queue.borrow_mut().pop_front();
            let Some((root, kind)) = next else { break };
            self.handle(cx, &root, kind);
        }
    }

    fn handle(&self, cx: &Rc<AppContext>, root: &Path, kind: InputKind) {
        if self.shared.borrow().quitting && !matches!(kind, InputKind::Quit) {
            return;
        }
        let at = self.clock.now();
        let step = {
            let mut folders = self.folders.borrow_mut();
            let Some(folder) = folders.get_mut(root) else { return };
            let step = folder.factory.handle(Input::new(at, kind));
            folder.revision += 1;
            if !step.refusals.is_empty() {
                folder.refusals.clone_from(&step.refusals);
            }
            step
        };
        self.after(cx, root, step);
        self.changed(root);
        self.keep_ticking();
    }

    /// Writes the step's log entries, then runs its effects; effects of orders whose log could not be written do not
    /// run, and their orders hear of it (E167).
    fn after(&self, cx: &Rc<AppContext>, root: &Path, step: Step) {
        let Step { log, effects, .. } = step;
        if log.is_empty() && effects.is_empty() {
            return;
        }
        if log.is_empty() && self.writing.get() == 0 {
            return self.run_effects(cx, root, effects, &[]);
        }
        let Some(project) = self.folders.borrow().get(root).map(|f| f.project.clone()) else {
            return;
        };
        let groups = store::group(&project, &log);
        let orders: Vec<OrderId> = groups.iter().map(|(order, _, _)| order.clone()).collect();
        let (root_for_job, effects_for_job) = (root.to_path_buf(), effects.clone());
        self.writing.set(self.writing.get() + 1);
        let back = self.back();
        let queued = ide_core::lane::DISK.try_run(move || {
            let failed = store::append_all(&groups);
            back.deliver(Delivery::Logged {
                root: root_for_job,
                effects: effects_for_job,
                failed,
            });
        });
        if queued.is_err() {
            self.writing.set(self.writing.get().saturating_sub(1));
            ide_core::diag::report("writing an order log", "the disk lane took no more work");
            self.run_effects(cx, root, effects, &orders);
        }
    }

    fn run_effects(&self, cx: &Rc<AppContext>, root: &Path, effects: Vec<Effect>, failed: &[OrderId]) {
        for effect in effects {
            if effect.order().is_some_and(|order| failed.contains(order)) {
                continue;
            }
            self.effect(cx, root, effect);
        }
        for order in failed {
            self.input(cx, root, InputKind::LogFailed(order.clone()));
        }
    }

    /// Tells the views of `root` that it changed.
    fn changed(&self, root: &Path) {
        let listeners: Vec<Listener> = self.listeners.borrow().iter().map(|(_, f)| f.clone()).collect();
        for listener in listeners {
            listener(root);
        }
    }

    /// Whether any order of any folder runs, which keeps the ticks going.
    fn any_running(&self) -> bool {
        self.folders.borrow().values().any(|folder| {
            folder
                .factory
                .orders()
                .any(|o| !o.finished() && *o.condition() == ide_factory::Condition::Running)
        })
    }

    /// Starts the ticks while an order runs (E168); they stop by themselves once none does.
    fn keep_ticking(&self) {
        if self.shared.borrow().ticking || !self.any_running() {
            return;
        }
        self.shared.borrow_mut().ticking = true;
        self.back().deliver_after(TICK, Delivery::Tick);
    }

    fn tick(&self, cx: &Rc<AppContext>) {
        let count = {
            let mut shared = self.shared.borrow_mut();
            shared.ticking = false;
            shared.ticks += 1;
            shared.ticks
        };
        if self.shared.borrow().quitting {
            return;
        }
        power::poll_battery(self, cx);
        let roots: Vec<PathBuf> = self.folders.borrow().keys().cloned().collect();
        for root in &roots {
            self.input(cx, root, InputKind::Tick);
        }
        if count.is_multiple_of(RULES_EVERY) {
            self.reload_all_rules();
        }
        self.keep_ticking();
    }

    /// The folders with a factory.
    fn roots(&self) -> Vec<PathBuf> {
        self.folders.borrow().keys().cloned().collect()
    }

    /// Hands `kind` to every folder's factory.
    fn broadcast(&self, cx: &Rc<AppContext>, kind: &InputKind) {
        for root in self.roots() {
            self.input(cx, &root, kind.clone());
        }
    }
}

/// What is shown for an order in lists of what runs: the question before quitting, the activity view.
fn order_name(title: &str) -> String {
    splat_i18n::tf("Auftrag „{}“", &[title])
}
