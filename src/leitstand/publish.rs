//! Publishing an order on the user's click (E177): the outgoing text scanned with the order's frozen gitleaks rules,
//! the push, the merge request and the issue's comment, and the checks a restart makes of them on the service.
//!
//! The push is the one git call of the Leitstand that does not run unattended (E192): the user clicked it, so it runs
//! as their own push from the Git panel would, through the app's git with the login shell's environment, their
//! credential helpers, SSH agent and hooks, from the main repository whose hooks no builder can reach. It pushes the
//! accepted commit itself, so a branch moved since G2 changes nothing. The service calls go through `ide_ci` with the
//! CI panel's remote and token for the folder (E77, E12). Everything reaching out waits until the folder is trusted.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_factory::event::{OpResult, TargetState};
use ide_factory::gates::SecretScan;
use ide_factory::publish::ChangeRef;
use ide_factory::{Effect, InputKind, OperationId, OrderId, RunId};
use ide_ui_factory::model::detail::ForgeAccess;

use super::{Delivery, Leitstand, freeze};

/// How long what the app found of a folder's service holds before the order tab reads it again.
const FORGE_EVERY: Duration = Duration::from_secs(60);

/// What the app last found of a folder's service for publishing.
#[derive(Default)]
pub(super) struct Forge {
    access: ForgeAccess,
    /// The branch the remote's HEAD points at.
    default_branch: Option<String>,
    read: Option<Instant>,
    reading: bool,
}

/// Whether `effect` is publishing's, which [`Leitstand::publish_effect`] carries out.
pub(super) fn handles(effect: &Effect) -> bool {
    match effect {
        Effect::ScanOutgoing { .. } | Effect::Push { .. } | Effect::OpenChange { .. } | Effect::CommentIssue { .. } => {
            true
        }
        Effect::Verify { target, .. } => target.on_service(),
        _ => false,
    }
}

impl Leitstand {
    /// Carries out one of publishing's effects; its result comes back as an input of the order.
    pub(super) fn publish_effect(&self, root: &Path, effect: Effect) {
        match effect {
            Effect::ScanOutgoing { order, run_id, text } => self.scan_outgoing(root, order, run_id, text),
            Effect::Push {
                order,
                op,
                branch,
                commit,
            } => self.push(root, order, op, branch, commit),
            Effect::OpenChange {
                order,
                op,
                branch,
                base,
                title,
                body,
                marker,
            } => self.on_service(root, order, op, move |client| {
                open_change(client, &branch, &base, &title, &body, &marker)
            }),
            Effect::CommentIssue {
                order,
                op,
                issue,
                body,
                marker,
            } => self.on_service(root, order, op, move |client| comment(client, issue, &body, &marker)),
            Effect::Verify { order, op, target, .. } => self.verify_on_service(root, order, op, target),
            _ => {}
        }
    }

    /// Scans everything that is to go out with the gitleaks rules frozen for the order (C52, F75).
    fn scan_outgoing(&self, root: &Path, order: OrderId, run_id: RunId, text: String) {
        let frozen = self
            .with_order(root, &order, |o| o.frozen().map(PathBuf::from))
            .flatten()
            .unwrap_or_default();
        self.on_git(root, move |main, _| {
            let rules = freeze::frozen_rules(&frozen);
            let scan = main.and_then(|main| main.git.scan_text_frozen(&text, &rules));
            Some(InputKind::Scanned(order, run_id, scanned(scan)))
        });
    }

    /// Pushes `commit` as `branch` to the remote of the folder's service, on a thread of its own, since a push takes
    /// as long as the network does.
    fn push(&self, root: &Path, order: OrderId, op: OperationId, branch: String, commit: String) {
        let (folder, back) = (root.to_path_buf(), self.back());
        let started = crate::util::spawn("order-push", move || {
            let result = pushed(&folder, &branch, &commit);
            back.deliver(Delivery::Input(folder, Box::new(InputKind::Done { order, op, result })));
        });
        if let Err(why) = started {
            ide_core::diag::report("pushing an order's branch", why);
        }
    }

    /// Runs `call` with the client of the folder's service on the CI lane and hands its result to the order.
    fn on_service(
        &self,
        root: &Path,
        order: OrderId,
        op: OperationId,
        call: impl FnOnce(&ide_ci::Client) -> OpResult + Send + 'static,
    ) {
        let (folder, back) = (root.to_path_buf(), self.back());
        let trusted = crate::project::is_trusted(root);
        let deliver = {
            let (folder, order, op, back) = (folder.clone(), order.clone(), op.clone(), back.clone());
            move |result: OpResult| {
                let done = InputKind::Done { order, op, result };
                back.deliver(Delivery::Input(folder, Box::new(done)));
            }
        };
        let queued = crate::ci_panel::on_lane(
            move || {
                if !trusted {
                    return untrusted();
                }
                match crate::ci_panel::client(&folder) {
                    Ok(client) => call(&client),
                    Err(error) => failed(&error),
                }
            },
            move |result| deliver(result.unwrap_or_else(not_run)),
        );
        if !queued {
            let done = InputKind::Done {
                order,
                op,
                result: not_run(),
            };
            back.deliver(Delivery::Input(root.to_path_buf(), Box::new(done)));
        }
    }

    /// After a restart: whether a step of publishing happened, as the service tells (C66).
    fn verify_on_service(&self, root: &Path, order: OrderId, op: OperationId, target: TargetState) {
        let (folder, done_root, back) = (root.to_path_buf(), root.to_path_buf(), self.back());
        let trusted = crate::project::is_trusted(root);
        let fallback = (order.clone(), op.clone(), back.clone());
        let queued = crate::ci_panel::on_lane(
            move || {
                if !trusted {
                    return Err(None);
                }
                let client = crate::ci_panel::client(&folder).map_err(Some)?;
                reached(&client, &target).map_err(Some)
            },
            move |checked| {
                let input = match checked {
                    Some(Ok(reached)) => InputKind::Verified { order, op, reached },
                    Some(Err(error)) => InputKind::Done {
                        order,
                        op,
                        result: error.as_ref().map_or_else(untrusted, failed),
                    },
                    None => InputKind::Done {
                        order,
                        op,
                        result: not_run(),
                    },
                };
                back.deliver(Delivery::Input(done_root, Box::new(input)));
            },
        );
        if !queued {
            let (order, op, back) = fallback;
            let result = not_run();
            back.deliver(Delivery::Input(
                root.to_path_buf(),
                Box::new(InputKind::Done { order, op, result }),
            ));
        }
    }

    /// What the app knows of `root`'s service for the order tab, with the branch its remote's HEAD points at; read
    /// again on the CI lane once it is older than [`FORGE_EVERY`].
    pub(super) fn forge_access(&self, root: &Path) -> (ForgeAccess, Option<String>) {
        let (access, stale) = {
            let folders = self.folders.borrow();
            let Some(forge) = folders.get(root).map(|f| &f.forge) else {
                return (ForgeAccess::Unknown, None);
            };
            let stale = !forge.reading && forge.read.is_none_or(|at| at.elapsed() > FORGE_EVERY);
            ((forge.access.clone(), forge.default_branch.clone()), stale)
        };
        if stale && crate::project::is_trusted(root) {
            self.read_forge(root);
        }
        access
    }

    fn read_forge(&self, root: &Path) {
        if let Some(folder) = self.folders.borrow_mut().get_mut(root) {
            folder.forge.reading = true;
        }
        let (folder, done_root, back) = (root.to_path_buf(), root.to_path_buf(), self.back());
        let queued = crate::ci_panel::on_lane(
            move || {
                (
                    crate::ci_panel::access(&folder),
                    crate::ci_panel::default_branch(&folder),
                )
            },
            move |read| {
                let (access, default) = read.unwrap_or_default();
                back.deliver(Delivery::Forge(done_root, access, default));
            },
        );
        if !queued && let Some(folder) = self.folders.borrow_mut().get_mut(root) {
            folder.forge.reading = false;
        }
    }

    /// What the CI lane found of `root`'s service; the views hear of it when it changed.
    pub(super) fn forge_read(&self, root: &Path, access: ForgeAccess, default_branch: Option<String>) {
        let changed = {
            let mut folders = self.folders.borrow_mut();
            let Some(folder) = folders.get_mut(root) else { return };
            let changed = folder.forge.access != access || folder.forge.default_branch != default_branch;
            folder.forge = Forge {
                access,
                default_branch,
                read: Some(Instant::now()),
                reading: false,
            };
            changed
        };
        if changed {
            self.changed(root);
        }
    }
}

/// The scan as the factory takes it; a scan that could not run counts as gitleaks missing, so nothing goes out.
fn scanned(scan: ide_git::Result<ide_git::SecretScan>) -> SecretScan {
    match scan {
        Ok(ide_git::SecretScan::Clean) => SecretScan::Clean,
        Ok(ide_git::SecretScan::Found(findings)) => SecretScan::Found {
            count: u32::try_from(findings.len()).unwrap_or(u32::MAX),
            places: findings.iter().map(|f| f.line.to_string()).collect(),
        },
        Ok(ide_git::SecretScan::NotInstalled) | Err(_) => SecretScan::NotInstalled,
    }
}

/// The push of `commit` as `branch` to the remote of the folder's service, as the user's own push.
fn pushed(root: &Path, branch: &str, commit: &str) -> OpResult {
    if !crate::project::is_trusted(root) {
        return untrusted();
    }
    let remote = match crate::ci_panel::remote_client(root) {
        Ok((remote, _)) => remote,
        Err(error) => return failed(&error),
    };
    let Some(git) = ide_ui_git::git() else {
        return failed_with(splat_i18n::t("Die Umgebung der Login-Shell fehlt.").into());
    };
    let repo = match git.discover(root) {
        Ok(repo) => repo,
        Err(error) => return super::git::failed(&error),
    };
    let options = ide_git::PushOptions {
        remote: Some(remote),
        refspec: Some(format!("{commit}:refs/heads/{branch}")),
        set_upstream: false,
    };
    match repo.push(&options, &mut |_| {}, &ide_git::Cancel::new()) {
        Ok(_) => OpResult::Pushed,
        Err(error) => super::git::failed(&error),
    }
}

/// The merge request of `branch` with `marker`, opened unless the service has it already.
fn open_change(client: &ide_ci::Client, branch: &str, base: &str, title: &str, body: &str, marker: &str) -> OpResult {
    let change = |item: ide_ci::Item, reused| OpResult::ChangeOpened {
        change: ChangeRef::new(item.number, &item.url, reused),
    };
    match client.find_change(branch, marker) {
        Ok(Some(item)) => change(item, true),
        Ok(None) => match client.open_change(branch, base, title, body) {
            Ok(item) => change(item, false),
            Err(error) => failed(&error),
        },
        Err(error) => failed(&error),
    }
}

/// The comment with `marker` on the issue, made unless the issue has it already.
fn comment(client: &ide_ci::Client, issue: u64, body: &str, marker: &str) -> OpResult {
    match client.find_comment(issue, marker) {
        Ok(Some(url)) => OpResult::Commented { url },
        Ok(None) => match client.comment_issue(issue, body) {
            Ok(url) => OpResult::Commented { url },
            Err(error) => failed(&error),
        },
        Err(error) => failed(&error),
    }
}

/// What a step achieved if its target state holds on the service.
fn reached(client: &ide_ci::Client, target: &TargetState) -> ide_ci::Result<Option<OpResult>> {
    Ok(match target {
        TargetState::RemoteBranchAt { branch, commit } => client
            .branch_head(branch)?
            .filter(|head| head == commit)
            .map(|_| OpResult::Pushed),
        TargetState::ChangeFor { branch, marker } => {
            client.find_change(branch, marker)?.map(|item| OpResult::ChangeOpened {
                change: ChangeRef::new(item.number, &item.url, true),
            })
        }
        TargetState::IssueComment { issue, marker } => client
            .find_comment(*issue, marker)?
            .map(|url| OpResult::Commented { url }),
        _ => None,
    })
}

/// A service's refusal in the CI panel's words, such as a token without the scope `api`.
fn failed(error: &ide_ci::Error) -> OpResult {
    failed_with(crate::ci_panel::problem(error))
}

fn failed_with(detail: String) -> OpResult {
    OpResult::Failed { detail }
}

/// A step that could not run, as the lane took no more work or the call panicked.
fn not_run() -> OpResult {
    failed_with(splat_i18n::t("Der Schritt konnte nicht laufen; versuch es noch einmal.").into())
}

fn untrusted() -> OpResult {
    failed_with(splat_i18n::t("Der Ordner ist nicht vertraut; nichts wurde gesendet.").into())
}

#[cfg(feature = "selftest")]
mod selftest_probe;

#[cfg(test)]
#[path = "publish_tests.rs"]
mod tests;
