//! Carrying out what a factory asks for (`ide_factory::Effect`): sessions on the main thread, git on the
//! Leitstand's own lane, gates and the preparation of worktrees on threads of their own, files on the disk lane,
//! publishing to GitHub or GitLab in `publish`. Every
//! result comes back as an input with the order's id (E143); a result for an order whose folder is gone finds
//! nothing and is dropped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use ide_core::Cancel;
use ide_factory::budget::Spend;
use ide_factory::effect::{SandboxRun, ScanRange};
use ide_factory::event::OpResult;
use ide_factory::evidence::{Manifest, ManifestEntry};
use ide_factory::{Effect, InputKind, OperationId, OrderId, RunId};

use super::launching::Wanted;
use super::sessions::Stop;
use super::{Back, Delivery, GIT, Leitstand, freeze, gates, git, manifest, notices, power, publish, siblings, store};
use crate::context::AppContext;

/// How long a job waits for the login shell's environment, read once at launch.
pub(super) const ENVIRONMENT_WAIT: Duration = Duration::from_secs(15);

/// What runs for a folder's orders besides their factory.
#[derive(Default)]
pub(super) struct Work {
    /// The main repository's refs before each order's running turn (E173).
    pub(super) snapshots: BTreeMap<OrderId, ide_git::RefSnapshot>,
    /// What stops each order's running gate (C67).
    gates: BTreeMap<OrderId, Cancel>,
    /// The number of the last run of a gate or setup command, for its own folder.
    runs: u64,
}

/// What an effect needs to know of its order, read on the main thread.
#[derive(Clone)]
struct Info {
    title: String,
    branch: String,
    base_branch: String,
    worktree: Option<PathBuf>,
    frozen: Option<PathBuf>,
}

/// The login shell's environment, waited for.
pub(super) fn login() -> Vec<(String, String)> {
    crate::environment::wait(ENVIRONMENT_WAIT)
        .map(<[_]>::to_vec)
        .unwrap_or_default()
}

/// A failed operation's result.
fn done_failed(order: OrderId, op: OperationId, error: &impl ide_core::explain::Explain) -> InputKind {
    InputKind::Done {
        order,
        op,
        result: git::failed(error),
    }
}

impl Leitstand {
    pub(super) fn effect(&self, cx: &Rc<AppContext>, root: &Path, effect: Effect) {
        if publish::handles(&effect) {
            return self.publish_effect(root, effect);
        }
        match effect {
            Effect::StartSession {
                order,
                role,
                entry,
                resume,
                worktree,
                frozen,
                network,
                ..
            } => self.start_session(
                root,
                Wanted {
                    order,
                    role,
                    entry,
                    resume,
                    worktree: PathBuf::from(worktree),
                    frozen: (!frozen.is_empty()).then(|| PathBuf::from(frozen)),
                    network,
                },
            ),
            Effect::SendPrompt {
                session,
                prompt,
                text,
                schema,
                ..
            } => self.send_prompt(cx, root, &session, prompt, text, schema),
            Effect::StopTurn { session, .. } => self.stop_session(&session, Stop::Turn),
            Effect::StopBackground { session, .. } => self.stop_session(&session, Stop::Background),
            Effect::EndSession { session, .. } => self.stop_session(&session, Stop::Session),
            Effect::AnswerRequest {
                session,
                request,
                answer,
                ..
            } => {
                self.answer(&session, &request, &answer);
            }
            Effect::Notify { order, notice } => {
                let title = self.info(root, &order).map(|i| i.title).unwrap_or_default();
                notices::post(&order, &title, notice);
            }
            Effect::WakeLock { hold, running } => power::hold(self, root, hold, running),
            Effect::AddToDay { service, spend, .. } => self.add_to_day(cx, service.as_deref(), &spend),
            Effect::Confirm { hash } => self.confirm(root, hash),
            Effect::StopGate { order } => {
                if let Some(folder) = self.folders.borrow().get(root)
                    && let Some(cancel) = folder.work.gates.get(&order)
                {
                    cancel.cancel();
                }
            }
            Effect::WriteSpec { order, markdown } => self.write_spec(root, &order, markdown),
            other => self.repository_effect(root, other),
        }
    }

    fn info(&self, root: &Path, order: &OrderId) -> Option<Info> {
        let folders = self.folders.borrow();
        let order = folders.get(root)?.factory.order(order)?;
        Some(Info {
            title: order.title().to_string(),
            branch: order.id().branch(),
            base_branch: order.base().branch.clone(),
            worktree: order.worktree().map(PathBuf::from),
            frozen: order.frozen().map(PathBuf::from),
        })
    }

    fn project(&self, root: &Path) -> Option<PathBuf> {
        self.folders.borrow().get(root).map(|f| f.project.clone())
    }

    /// The next run's own folder of `order`.
    fn next_run(&self, root: &Path, order: &OrderId) -> Option<PathBuf> {
        let mut folders = self.folders.borrow_mut();
        let folder = folders.get_mut(root)?;
        folder.work.runs += 1;
        Some(store::run_folder(&folder.project, order, folder.work.runs))
    }

    /// Runs `job` with the folder's repository on the Leitstand's git lane, once the login shell's environment is
    /// known, and hands what it gives to the factory.
    pub(super) fn on_git(
        &self,
        root: &Path,
        job: impl FnOnce(ide_git::Result<git::Repos>, &[(String, String)]) -> Option<InputKind> + Send + 'static,
    ) {
        let root = root.to_path_buf();
        let back = self.back();
        self.with_environment(move |login| {
            let queued = GIT.try_run(move || {
                let main = git::open(&root, login);
                if let Some(input) = job(main, login) {
                    back.deliver(Delivery::Input(root, Box::new(input)));
                }
            });
            if queued.is_err() {
                ide_core::diag::report("an order's git", "the lane took no more work");
            }
        });
    }

    /// Effects on the repository and the worktree.
    fn repository_effect(&self, root: &Path, effect: Effect) {
        let info = effect.order().and_then(|order| self.info(root, order));
        match effect {
            Effect::PrepareWorktree {
                order,
                op,
                branch,
                copy,
                setup,
                ..
            } => self.prepare(root, order, op, branch, copy, setup),
            Effect::FreezeConfig { order, paths } => self.freeze(root, order, paths),
            Effect::RunGate {
                order,
                run_id,
                index,
                name,
                run,
                images,
            } => self.run_gate(root, order, run_id, index, &name, run, images),
            Effect::SnapshotRefs { order } => {
                let (folder, back) = (root.to_path_buf(), self.back());
                self.on_git(root, move |main, _| {
                    if let Ok(snapshot) = main.and_then(|main| git::refs(&main)) {
                        back.deliver(Delivery::Refs(folder, order, snapshot));
                    }
                    None
                });
            }
            Effect::CompareRefs { order } => self.compare_refs(root, order, info.map(|i| i.branch).unwrap_or_default()),
            Effect::ReadManifest { order, copied } => {
                let folder = root.to_path_buf();
                self.on_git(root, move |main, _| {
                    Some(InputKind::Manifest(
                        order,
                        manifest::read(&folder, &copied, main.ok().as_ref().map(|r| &r.git)),
                    ))
                });
            }
            other => self.worktree_effect(root, other, info),
        }
    }

    /// Effects that work in an order's worktree or remove it.
    fn worktree_effect(&self, root: &Path, effect: Effect, info: Option<Info>) {
        let worktree = info.as_ref().and_then(|i| i.worktree.clone()).unwrap_or_default();
        let frozen = info.as_ref().and_then(|i| i.frozen.clone()).unwrap_or_default();
        let branch = info.as_ref().map(|i| i.branch.clone()).unwrap_or_default();
        let folder = root.to_path_buf();
        match effect {
            Effect::ScanSecrets { order, run_id, range } => self.on_git(root, move |main, _| {
                let since = match &range {
                    ScanRange::Pending => "HEAD".to_string(),
                    ScanRange::Diff { base, .. } => base.clone(),
                };
                let scan = main.and_then(|main| git::scan(&main, &worktree, &since, &freeze::frozen_rules(&frozen)));
                Some(InputKind::Scanned(
                    order,
                    run_id,
                    scan.unwrap_or(ide_factory::gates::SecretScan::NotInstalled),
                ))
            }),
            Effect::Commit {
                order,
                op,
                message,
                parent,
            } => self.on_git(root, move |main, _| {
                let result = match main.and_then(|main| git::open_in(&main, &worktree)) {
                    Ok(copy) => git::commit(&copy, &branch, &op, &message, &parent),
                    Err(error) => git::failed(&error),
                };
                Some(InputKind::Done { order, op, result })
            }),
            Effect::DiffNames {
                order,
                run_id,
                base,
                head,
            } => self.on_git(root, move |main, _| {
                let changes = main
                    .and_then(|main| git::diff_names(&main, &base, &head))
                    .unwrap_or_default();
                Some(InputKind::Diff(order, run_id, changes))
            }),
            Effect::CollectReviewInput { order, base, head } => self.on_git(root, move |main, _| {
                let input = main
                    .and_then(|main| git::open_in(&main, &worktree))
                    .and_then(|copy| git::review_input(&copy, &base, &head));
                let (diff, commits) = input.unwrap_or_default();
                Some(InputKind::ReviewInput { order, diff, commits })
            }),
            Effect::RejectHunks { order, op, hunks } => {
                let base = self.base_of(root, &order);
                self.on_git(root, move |main, _| {
                    let rejected = main
                        .and_then(|main| git::open_in(&main, &worktree))
                        .and_then(|copy| git::reject_hunks(&copy, &base, &hunks));
                    let result = match rejected {
                        Ok(()) => OpResult::Reversed,
                        Err(error) => git::failed(&error),
                    };
                    Some(InputKind::Done { order, op, result })
                });
            }
            other => self.merge_effect(&folder, other),
        }
    }

    fn base_of(&self, root: &Path, order: &OrderId) -> String {
        let folders = self.folders.borrow();
        folders
            .get(root)
            .and_then(|f| f.factory.order(order))
            .map(|o| o.base().commit.clone())
            .unwrap_or_default()
    }

    /// Merging, taking over, removing and verifying.
    fn merge_effect(&self, root: &Path, effect: Effect) {
        let (folder, back) = (root.to_path_buf(), self.back());
        match effect {
            Effect::MergeTarget { order, op, .. } => self.on_git(root, move |main, _| {
                let result = match (main, git::tree_of(&folder, &order)) {
                    (Ok(main), Some(tree)) => git::merge_target(&main, &tree),
                    (Err(error), _) => git::failed(&error),
                    (Ok(_), None) => no_worktree(),
                };
                Some(InputKind::Done { order, op, result })
            }),
            Effect::CompareDiffs {
                order,
                run_id,
                old_base,
                old_head,
                new_base,
                new_head,
            } => self.on_git(root, move |main, _| {
                let same = main.and_then(|main| git::same_diff(&main, (&old_base, &old_head), (&new_base, &new_head)));
                Some(InputKind::Compared(order, run_id, same.unwrap_or(false)))
            }),
            Effect::TakeOver { order, op, head, .. } => self.on_git(root, move |main, _| {
                let result = match (main, git::tree_of(&folder, &order)) {
                    (Ok(main), Some(tree)) => git::take_over(&main, &tree, &head),
                    (Err(error), _) => git::failed(&error),
                    (Ok(_), None) => no_worktree(),
                };
                Some(InputKind::Done { order, op, result })
            }),
            Effect::RemoveCheckpoints { order, op, threads } => self.on_git(root, move |main, _| {
                let worktree = git::tree_of(&folder, &order).map(|t| t.path);
                let result = match main {
                    Ok(main) => git::remove_checkpoints(&main, worktree.as_deref(), &threads),
                    Err(error) => git::failed(&error),
                };
                Some(InputKind::Done { order, op, result })
            }),
            Effect::RemoveWorktree {
                order,
                op,
                path,
                branch,
                keep_branch,
            } => self.on_git(root, move |main, _| {
                let result = match main {
                    Ok(main) => git::remove_worktree(&main, &folder, &order, &path, &branch, keep_branch),
                    Err(error) => git::failed(&error),
                };
                measure_space(&back, &folder);
                Some(InputKind::Done { order, op, result })
            }),
            Effect::Verify { ref order, ref op, .. } if self.setup_runs(root, order, op) => {}
            Effect::Verify {
                order,
                op,
                action,
                target,
            } => self.on_git(root, move |main, _| {
                let reached = main.and_then(|main| git::verify(&main, &folder, &order, &action, &target));
                match reached {
                    Ok(reached) => Some(InputKind::Verified { order, op, reached }),
                    Err(error) => Some(done_failed(order, op, &error)),
                }
            }),
            _ => {}
        }
    }

    fn compare_refs(&self, root: &Path, order: OrderId, branch: String) {
        let before = self
            .folders
            .borrow_mut()
            .get_mut(root)
            .and_then(|f| f.work.snapshots.remove(&order));
        self.on_git(root, move |main, _| {
            let after = main.and_then(|main| git::refs(&main));
            let changes = match (before, after) {
                (Some(before), Ok(after)) => git::ref_changes(&before, &after, &branch),
                _ => Vec::new(),
            };
            Some(InputKind::Refs(order, changes))
        });
    }

    /// Makes the order's worktree, copies the listed files from the working copy and runs the setup command (E188).
    fn prepare(
        &self,
        root: &Path,
        order: OrderId,
        op: OperationId,
        branch: String,
        copy: Vec<String>,
        setup: Option<SandboxRun>,
    ) {
        let Some(info) = self.info(root, &order) else { return };
        let run_folder = self.next_run(root, &order);
        let cancel = self.setup_started(root, &order, &op);
        let id = order.clone();
        let folder = root.to_path_buf();
        let back = self.back();
        let started = crate::util::spawn("order-worktree", move || {
            let root = folder;
            let login = login();
            let result = match git::open(&root, &login) {
                Ok(main) => prepared(
                    &main,
                    &root,
                    &order,
                    &branch,
                    &info.base_branch,
                    &copy,
                    setup,
                    run_folder,
                    &login,
                    cancel.clone(),
                    info.frozen.as_deref(),
                ),
                Err(error) => git::failed(&error),
            };
            measure_space(&back, &root);
            let input = super::setups::ended(order.clone(), op.clone(), result, &cancel);
            back.deliver(Delivery::SetupEnded(root, order, op, Box::new(input)));
        });
        if let Err(why) = started {
            self.setup_gone(root, &id);
            ide_core::diag::report("preparing an order's worktree", why);
        }
    }

    /// Freezes the agent configuration of the working copy for the order (E172).
    fn freeze(&self, root: &Path, order: OrderId, patterns: Vec<String>) {
        let Some(project) = self.project(root) else { return };
        let into = store::frozen(&project, &order);
        let root = root.to_path_buf();
        let back = self.back();
        let queued = ide_core::lane::DISK.try_run(move || {
            let frozen = freeze::freeze(&root, &patterns, &into)
                .map(|dir| dir.to_string_lossy().into_owned())
                .map_err(|error| ide_core::explain::Explain::explain(&error));
            back.deliver(Delivery::Input(root, Box::new(InputKind::Frozen(order, frozen))));
        });
        if queued.is_err() {
            ide_core::diag::report("freezing an order's configuration", "the disk lane took no more work");
        }
    }

    /// Runs a gate command in the worktree under the gate sandbox (E187), stoppable through `StopGate`.
    #[expect(clippy::too_many_arguments, reason = "the fields of the effect, one by one")]
    fn run_gate(
        &self,
        root: &Path,
        order: OrderId,
        run_id: RunId,
        index: usize,
        name: &str,
        run: SandboxRun,
        images: Option<String>,
    ) {
        let Some(info) = self.info(root, &order) else { return };
        let Some(folder) = self.next_run(root, &order) else {
            return;
        };
        let cancel = Cancel::new();
        if let Some(f) = self.folders.borrow_mut().get_mut(root) {
            f.work.gates.insert(order.clone(), cancel.clone());
        }
        let label = splat_i18n::tf("Prüfstation {} von „{}“", &[name, &info.title]);
        let root = root.to_path_buf();
        let back = self.back();
        let started = crate::util::spawn("order-gate", move || {
            let login = login();
            let main = git::open(&root, &login).ok();
            let worktree = info.worktree.clone().unwrap_or_default();
            let copy = main.as_ref().and_then(|main| git::open_in(main, &worktree).ok());
            let job = gates::Job {
                env: ide_process::order_env::for_orders(&login, &[]),
                rules: gates::rules(info.frozen.as_deref(), &root),
                common_dir: copy.as_ref().map(|c| c.common_dir().to_path_buf()),
                dependencies: siblings::targets(&root),
                links: siblings::links(&root, &worktree),
                images: images.map(|dir| gates::images_folder(&dir, &ide_core::paths::home())),
                worktree,
                folder,
                run,
                label,
                cancel,
            };
            let outcome = gates::run(&job, copy.as_ref());
            back.deliver(Delivery::Input(
                root,
                Box::new(InputKind::GateDone {
                    order,
                    run_id,
                    index,
                    outcome,
                }),
            ));
        });
        if let Err(why) = started {
            ide_core::diag::report("running an order's gate", why);
        }
    }

    fn write_spec(&self, root: &Path, order: &OrderId, markdown: String) {
        let Some(project) = self.project(root) else { return };
        let file = store::spec_file(&project, order);
        let queued = ide_core::lane::DISK.try_run(move || {
            if let Err(error) = store::write_atomically(&file, markdown.as_bytes()) {
                ide_core::diag::report("writing an order's specification", error);
            }
        });
        if queued.is_err() {
            ide_core::diag::report("writing an order's specification", "the disk lane took no more work");
        }
    }

    /// Adds `spend` to the daily account across projects and tells every folder (E168).
    fn add_to_day(&self, cx: &Rc<AppContext>, service: Option<&str>, spend: &Spend) {
        let day = {
            let mut shared = self.shared.borrow_mut();
            shared.day = super::clock::today(&shared.day, self.clock.now().wall);
            let today = shared.day.day().to_string();
            shared.day.add(&today, service, spend);
            shared.day.clone()
        };
        self.keep_day(cx, day);
    }

    /// Starts the daily account afresh once its day is over, and tells every folder, even while nothing spends: orders
    /// paused for yesterday's budget go on (E168).
    pub(super) fn roll_day(&self, cx: &Rc<AppContext>) {
        let rolled = {
            let mut shared = self.shared.borrow_mut();
            let day = super::clock::today(&shared.day, self.clock.now().wall);
            (day != shared.day).then(|| {
                shared.day = day.clone();
                day
            })
        };
        if let Some(day) = rolled {
            self.keep_day(cx, day);
        }
    }

    /// Keeps `day` as the daily account and hands it to every folder.
    fn keep_day(&self, cx: &Rc<AppContext>, day: ide_factory::budget::DailyAccount) {
        let kept = day.clone();
        let queued = ide_core::lane::DISK.try_run(move || store::save_day(&store::base(), &kept));
        if queued.is_err() {
            ide_core::diag::report("keeping the daily account", "the disk lane took no more work");
        }
        self.broadcast(cx, &InputKind::Day(day));
    }
}

fn no_worktree() -> OpResult {
    OpResult::Failed {
        detail: splat_i18n::t("Der Worktree des Auftrags ist nicht mehr da.").into(),
    }
}

/// Measures the space the folder's worktrees take and hands it to the fleet line (R68).
fn measure_space(back: &Back, root: &Path) {
    let bytes = manifest::size(&ide_git::thread_trees::folder(root));
    back.deliver(Delivery::Space(root.to_path_buf(), bytes));
}

/// The worktree made, the files copied and the setup command run.
#[expect(
    clippy::too_many_arguments,
    reason = "what one preparation needs, gathered on the main thread"
)]
fn prepared(
    main: &git::Repos,
    root: &Path,
    order: &OrderId,
    branch: &str,
    base_branch: &str,
    copy: &[String],
    setup: Option<SandboxRun>,
    run_folder: Option<PathBuf>,
    login: &[(String, String)],
    cancel: Cancel,
    frozen: Option<&Path>,
) -> OpResult {
    let tree = match git::make_worktree(main, root, order, branch, base_branch) {
        Ok(tree) => tree,
        Err(failed) => return *failed,
    };
    siblings::link(root, &tree.path);
    let mut copied = Vec::new();
    for path in copy {
        if let Err(error) = freeze::copy_listed(root, &tree.path, path, copy) {
            return git::failed(&error);
        }
        copied.push(ManifestEntry::Copied {
            path: path.clone(),
            hash: manifest::hash(&root.join(path)),
        });
    }
    let setup = setup.map(|run| {
        let copy = git::open_in(main, &tree.path).ok();
        let job = gates::Job {
            env: ide_process::order_env::for_orders(login, &[]),
            rules: gates::rules(frozen, root),
            common_dir: copy.as_ref().map(|c| c.common_dir().to_path_buf()),
            dependencies: siblings::targets(root),
            links: siblings::links(root, &tree.path),
            images: None,
            worktree: tree.path.clone(),
            folder: run_folder.unwrap_or_else(|| tree.path.with_extension("setup")),
            run,
            label: splat_i18n::tf("Einrichtung von „{}“", &[&tree.title]),
            cancel,
        };
        gates::run(&job, copy.as_ref())
    });
    let base_tree = main
        .branch_head(branch)
        .ok()
        .flatten()
        .map(|head| head.tree)
        .unwrap_or_default();
    let prepared = OpResult::Prepared {
        path: tree.path.to_string_lossy().into_owned(),
        tree: base_tree,
        setup,
        copied: Manifest::new(copied).entries().to_vec(),
    };
    match git::mark_prepared(main, &tree.path, &prepared) {
        Ok(()) => prepared,
        Err(error) => git::failed(&error),
    }
}
