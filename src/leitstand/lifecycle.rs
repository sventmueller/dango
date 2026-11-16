//! When a folder's factory starts, what it hears of its windows, and how the app's end, sleep and quotas reach every
//! factory (E166, E167, E168): a factory starts when a window of a trusted folder opens or the user trusts it, from
//! the order logs of earlier runs; it stays when the folder's last window closes, and its orders pause then; quitting
//! pauses every running order with "app ended" before the supervisor ends their processes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use ide_factory::budget::DailyAccount;
use ide_factory::quota::{QuotaBook, QuotaObservation};
use ide_factory::routing::{Agent, AgentStatus, Readiness};
use ide_factory::workflow::rules::{Confirmations, Rules};
use ide_factory::{Factory, InputKind};
use ide_ui_common::alert::{Alert, Role};
use objc2::MainThreadMarker;
use objc2_app_kit::NSAlertSecondButtonReturn;
use splat_i18n::{t, tf};

use super::{Delivery, Folder, Leitstand, power, rules, store};
use crate::context::AppContext;
use crate::window::WindowController;

/// How long quitting waits for the last log entries to reach the disk.
const QUIT_WRITE: Duration = Duration::from_secs(2);

/// What holds across folders.
#[derive(Default)]
pub(super) struct Shared {
    pub(super) quitting: bool,
    pub(super) ticking: bool,
    pub(super) ticks: u64,
    /// The daily account across projects (E168), once read.
    pub(super) day: DailyAccount,
    /// The quotas across projects (C65), once read.
    pub(super) quotas: QuotaBook,
    read: bool,
    pub(super) power: power::Power,
}

/// What quitting waits for: the log entries that pause the running orders.
pub(crate) struct QuitWrite(Option<mpsc::Receiver<()>>);

impl QuitWrite {
    /// Waits until the entries are on disk, or a little while.
    pub(crate) fn wait(self) {
        if let Some(written) = self.0 {
            let _ = written.recv_timeout(QUIT_WRITE);
        }
    }
}

impl Leitstand {
    /// A window shows `controller`'s folder now; a trusted folder's factory starts, or hears that it has a window.
    pub(crate) fn attached(&self, cx: &Rc<AppContext>, controller: &WindowController) {
        if let Some(root) = controller.project_root()
            && controller.is_trusted()
        {
            self.open_folder(cx, &root);
        }
    }

    /// The user trusts `root` now (E22): its factory starts if a window shows it.
    pub(crate) fn trust_changed(&self, cx: &Rc<AppContext>, root: &Path) {
        let shown = cx
            .windows()
            .all()
            .iter()
            .any(|w| !w.is_closed() && w.project_root().as_deref() == Some(root));
        if shown && crate::project::is_trusted(root) {
            self.open_folder(cx, root);
        }
    }

    /// `controller`'s window lets its folder go, closing or showing another; without another window of the folder its
    /// orders pause (E166).
    pub(crate) fn folder_closed(&self, cx: &Rc<AppContext>, controller: &WindowController) {
        let Some(root) = controller.project_root() else { return };
        if !self.folders.borrow().contains_key(&root) || shown_elsewhere(cx, controller, &root) {
            return;
        }
        self.input(cx, &root, InputKind::Windows(false));
    }

    /// The names of the orders of `controller`'s folder whose sessions would pause if it lost this window: none when
    /// another window shows the folder.
    pub(crate) fn would_pause(&self, cx: &Rc<AppContext>, controller: &WindowController) -> Vec<String> {
        let Some(root) = controller.project_root() else {
            return Vec::new();
        };
        if shown_elsewhere(cx, controller, &root) {
            return Vec::new();
        }
        self.running_in(&root)
    }

    /// Whether `controller`'s window may close as far as orders go: none would pause, or the user agrees (E166, E56).
    pub(crate) fn may_close(&self, cx: &Rc<AppContext>, controller: &WindowController, mtm: MainThreadMarker) -> bool {
        let running = self.would_pause(cx, controller);
        if running.is_empty() {
            return true;
        }
        Alert::new(mtm, t("Schließen, während Aufträge laufen?"))
            .warning()
            .informative(&format!(
                "{}\n{}",
                tf("Läuft noch: {}", &[&running.join(", ")]),
                t("Ohne Fenster des Ordners pausieren seine Aufträge, bis er wieder offen ist.")
            ))
            .button(t("Abbrechen"), Role::CancelOnReturn)
            .button(t("Schließen"), Role::Destructive)
            .run_modal()
            == NSAlertSecondButtonReturn
    }

    /// Starts the factory of `root` from its logs, or tells the running one that it has a window again.
    fn open_folder(&self, cx: &Rc<AppContext>, root: &Path) {
        if self.folders.borrow().contains_key(root) {
            self.input(cx, root, InputKind::Windows(true));
            return;
        }
        if self.loading.borrow().iter().any(|r| r == root) {
            return;
        }
        self.loading.borrow_mut().push(root.to_path_buf());
        self.read_shared();
        power::watch_sleep(self);
        let project = store::project(&store::base(), root);
        let root = root.to_path_buf();
        let back = self.back();
        let queued = ide_core::lane::DISK.try_run(move || {
            let logs = store::read_logs(&project);
            let confirmations = store::load_confirmations(&project);
            back.deliver(Delivery::Loaded {
                root,
                logs,
                confirmations,
            });
        });
        if queued.is_err() {
            ide_core::diag::report("reading the order logs", "the disk lane took no more work");
        }
    }

    /// Reads the daily account and the quotas once.
    fn read_shared(&self) {
        let mut shared = self.shared.borrow_mut();
        if std::mem::replace(&mut shared.read, true) {
            return;
        }
        let base = store::base();
        shared.day = store::load_day(&base);
        shared.quotas = store::load_quotas(&base);
    }

    /// The logs of `root` were read: its factory comes back as they say, running orders paused (E167).
    pub(super) fn loaded(&self, cx: &Rc<AppContext>, root: PathBuf, logs: &[String], confirmations: Confirmations) {
        self.loading.borrow_mut().retain(|r| *r != root);
        if self.folders.borrow().contains_key(&root) || self.shared.borrow().quitting {
            return;
        }
        let defaults = rules::defaults();
        let first = Rules::new(&defaults, None, &confirmations);
        let restored = Factory::restore(rules::STAGE, first, logs, self.clock.now());
        for problem in &restored.problems {
            ide_core::diag::report(
                "restoring an order log",
                format!(
                    "log {}: cut {}, unreadable {}, stopped at line {}",
                    problem.log,
                    problem.cut,
                    problem.unreadable,
                    problem.stopped_at.unwrap_or_default()
                ),
            );
        }
        let project = store::project(&store::base(), &root);
        self.folders.borrow_mut().insert(
            root.clone(),
            Folder {
                factory: restored.factory,
                project,
                confirmations,
                revision: 0,
                refusals: Vec::new(),
                problems: restored.problems,
                work: super::effects::Work::default(),
                worktree_bytes: 0,
                agents: BTreeMap::new(),
                sections: BTreeMap::new(),
            },
        );
        self.after(cx, &root, restored.step);
        let (day, quotas) = {
            let shared = self.shared.borrow();
            (
                shared.day.clone(),
                shared.quotas.observations().cloned().collect::<Vec<_>>(),
            )
        };
        self.input(cx, &root, InputKind::Day(day));
        for observation in quotas {
            self.input(cx, &root, InputKind::Quota(observation));
        }
        if let Some(battery) = power::battery(self) {
            self.input(cx, &root, InputKind::Battery(battery));
        }
        self.read_agents(&root);
        self.reload_rules(&root);
        self.changed(&root);
    }

    /// Looks for the paths an order can take and hands what it found to the factory of `root` (E58, E170).
    fn read_agents(&self, root: &Path) {
        let root = root.to_path_buf();
        let back = self.back();
        let started = crate::util::spawn("order-agents", move || {
            let login = crate::environment::wait(Duration::from_secs(15)).unwrap_or_default();
            let path = ide_process::tool::var(login, "PATH");
            let status = |agent: &Agent, found: bool| {
                let mut status = AgentStatus::ready(agent);
                if !found {
                    status.readiness = Readiness::NotInstalled;
                }
                status
            };
            let agents = [
                (Agent::Claude, ide_ui_agent::claude_program(login).is_some()),
                (Agent::Codex, ide_process::tool::which("codex", path).is_some()),
                (Agent::Own, false),
            ]
            .into_iter()
            .map(|(agent, found)| {
                let status = status(&agent, found);
                (agent, status)
            })
            .collect();
            back.deliver(Delivery::Input(root, Box::new(InputKind::Agents(agents))));
        });
        if let Err(why) = started {
            ide_core::diag::report("looking for agents", why);
        }
    }

    /// The app quits: every running order pauses with "app ended", which is on its way to disk before the
    /// supervisor ends the processes (E167); then every session stops. What to wait for before the supervisor ends.
    pub(crate) fn quit(&self, cx: &Rc<AppContext>) -> QuitWrite {
        self.broadcast(cx, &InputKind::Quit);
        self.shared.borrow_mut().quitting = true;
        self.sessions.borrow_mut().stop_all();
        let roots = self.roots();
        for root in &roots {
            power::hold(self, root, false, 0);
        }
        let (sent, written) = mpsc::sync_channel(1);
        let queued = ide_core::lane::DISK.try_run(move || {
            let _ = sent.send(());
        });
        QuitWrite(queued.is_ok().then_some(written))
    }

    /// The Mac goes to sleep or woke up (E168).
    pub(super) fn sleep(&self, cx: &Rc<AppContext>, asleep: bool) {
        self.broadcast(cx, if asleep { &InputKind::Sleep } else { &InputKind::Wake });
    }

    /// Keeps a quota observation across projects and hands it to every folder (C65).
    pub(super) fn observe_quota(&self, cx: &Rc<AppContext>, observation: QuotaObservation) {
        let book = {
            let mut shared = self.shared.borrow_mut();
            shared.quotas.observe(observation.clone());
            shared.quotas.clone()
        };
        let queued = ide_core::lane::DISK.try_run(move || store::save_quotas(&store::base(), &book));
        if queued.is_err() {
            ide_core::diag::report("keeping the quotas", "the disk lane took no more work");
        }
        self.broadcast(cx, &InputKind::Quota(observation));
    }
}

/// Whether another open window than `controller`'s shows `root`.
fn shown_elsewhere(cx: &AppContext, controller: &WindowController, root: &Path) -> bool {
    cx.windows().all().iter().any(|window| {
        !std::ptr::eq(&**window, controller) && !window.is_closed() && window.project_root().as_deref() == Some(root)
    })
}
