//! Automatic intake from issues (E176, E190, R63): while the app runs, the open issues of each trusted folder whose
//! workflow file names an intake label and allowed authors, and whose loosening the user confirmed on this Mac, are
//! read every few minutes through the CI panel's client and token. Each new issue of an allowed author with the label
//! becomes an order once: the issues taken are kept beside the folder's logs before their orders are handed over, so
//! after a restart none is taken twice. A rate limit holds the folder's reads until the service says. Nothing is
//! written to GitHub or GitLab (O2).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_factory::intake::{IssueOrigin, issue_order, wanted};
use ide_factory::options::{NewOrder, check_new_order};
use serde::{Deserialize, Serialize};

use super::{Back, Command, Leitstand, store};

/// How often the issues are read; modest, since every read counts against the service's rate limit.
const EVERY: Duration = Duration::from_secs(5 * 60);

/// The first read after the app starts, once the folders' factories have read their rules.
const FIRST: Duration = Duration::from_secs(30);

/// Where intake stands for each folder, while the app runs.
#[derive(Default)]
pub(super) struct Intake {
    ticking: bool,
    folders: BTreeMap<PathBuf, FolderIntake>,
}

#[derive(Default)]
struct FolderIntake {
    reading: bool,
    /// No read before this time, in seconds since 1970, after a rate limit.
    not_before: i64,
    /// The last failure reported, so one that lasts is reported once.
    failure: Option<String>,
}

/// The issues taken in as orders, by their page, kept per project on this Mac.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Taken {
    taken: BTreeSet<String>,
}

fn taken_file(project: &Path) -> PathBuf {
    project.join("intake.json")
}

fn load_taken(project: &Path) -> BTreeSet<String> {
    std::fs::read(taken_file(project))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Taken>(&bytes).ok())
        .map(|t| t.taken)
        .unwrap_or_default()
}

fn save_taken(project: &Path, taken: BTreeSet<String>) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(&Taken { taken }).map_err(std::io::Error::other)?;
    store::write_atomically(&taken_file(project), &bytes)
}

/// What one read of a folder's issues found, with the issues taken before.
struct Read {
    issues: ide_ci::Result<Vec<ide_ci::Issue>>,
    taken: BTreeSet<String>,
}

impl Leitstand {
    /// Starts the reads of issues, once, for as long as the app runs.
    pub(super) fn start_intake(&self) {
        if std::mem::replace(&mut self.intake.borrow_mut().ticking, true) {
            return;
        }
        let back = self.back();
        crate::util::main_after(FIRST, move || intake_tick(&back));
    }

    /// Reads the issues of every folder whose intake applies and that is not held back.
    fn poll_intake(&self) {
        let now = ide_ci::now();
        for (root, label, project) in self.intake_folders() {
            let mut intake = self.intake.borrow_mut();
            let state = intake.folders.entry(root.clone()).or_default();
            if state.reading || now < state.not_before || !crate::project::is_trusted(&root) {
                continue;
            }
            state.reading = true;
            drop(intake);
            let back = self.back();
            let (folder, done_root) = (root.clone(), root.clone());
            let queued = crate::ci_panel::on_lane(
                move || Read {
                    issues: crate::ci_panel::client(&folder).and_then(|client| client.labelled_issues(&label)),
                    taken: load_taken(&project),
                },
                move |read| {
                    back.with(|cx| cx.leitstand().intake_read(&done_root, read));
                },
            );
            if !queued && let Some(state) = self.intake.borrow_mut().folders.get_mut(&root) {
                state.reading = false;
            }
        }
    }

    /// The folders with a confirmed intake and a valid workflow file: root, label and the folder of what is kept.
    fn intake_folders(&self) -> Vec<(PathBuf, String, PathBuf)> {
        self.folders
            .borrow()
            .iter()
            .filter(|(_, f)| f.factory.errors().is_empty())
            .filter_map(|(root, f)| {
                let intake = f.factory.rules().intake()?;
                Some((root.clone(), intake.label.clone(), f.project.clone()))
            })
            .collect()
    }

    /// Takes in the new issues `read` found for `root`; the issues it takes are kept before their orders start.
    fn intake_read(&self, root: &Path, read: Option<Read>) {
        let failure = {
            let mut intake = self.intake.borrow_mut();
            let state = intake.folders.entry(root.to_path_buf()).or_default();
            state.reading = false;
            let issues = match read.as_ref().map(|r| &r.issues) {
                Some(Ok(_)) => None,
                Some(Err(ide_ci::Error::RateLimited { until })) => {
                    state.not_before = until.unwrap_or_else(|| ide_ci::now() + 15 * 60);
                    None
                }
                Some(Err(error)) => Some(error.to_string()),
                None => Some("the read panicked".to_string()),
            };
            let report = issues.is_some() && issues != state.failure;
            state.failure.clone_from(&issues);
            issues.filter(|_| report)
        };
        if let Some(failure) = failure {
            ide_core::diag::report("reading issues for the Leitstand", failure);
        }
        let Some(Read {
            issues: Ok(issues),
            mut taken,
        }) = read
        else {
            return;
        };
        let orders = self.new_issue_orders(root, &issues, &taken);
        if orders.is_empty() {
            return;
        }
        let Some(project) = self.folders.borrow().get(root).map(|f| f.project.clone()) else {
            return;
        };
        taken.extend(orders.iter().filter_map(|o| o.issue.as_ref().map(|i| i.url.clone())));
        let (root, back) = (root.to_path_buf(), self.back());
        let queued = ide_core::lane::DISK.try_run(move || match save_taken(&project, taken) {
            Ok(()) => crate::util::main_soon(move || {
                back.with(|cx| {
                    for order in orders {
                        cx.leitstand().command(cx, &root, Command::New(order));
                    }
                });
            }),
            Err(error) => ide_core::diag::report("keeping the issues taken in", error),
        });
        if queued.is_err() {
            ide_core::diag::report("keeping the issues taken in", "the disk lane took no more work");
        }
    }

    /// The orders `issues` make for `root`: those with the label by an allowed author, not taken before, not the
    /// origin of an order the folder has, and that the sheet would let start.
    fn new_issue_orders(&self, root: &Path, issues: &[ide_ci::Issue], taken: &BTreeSet<String>) -> Vec<NewOrder> {
        let folders = self.folders.borrow();
        let Some(folder) = folders.get(root) else {
            return Vec::new();
        };
        let factory = &folder.factory;
        let Some(intake) = factory.rules().intake() else {
            return Vec::new();
        };
        let known: BTreeSet<&str> = factory
            .orders()
            .filter_map(|o| o.options().issue.as_ref().map(|i| i.url.as_str()))
            .collect();
        issues
            .iter()
            .filter(|issue| wanted(intake, issue.author(), issue.labels()))
            .filter(|issue| !taken.contains(issue.url()) && !known.contains(issue.url()))
            .map(|issue| {
                let origin = IssueOrigin::freeze(issue.url(), issue.number(), issue.author(), issue.body());
                issue_order(origin, issue.title())
            })
            .filter(|order| check_new_order(order, factory.rules(), factory.stage(), false, &folder.agents).ok())
            .collect()
    }
}

/// One round of reads, then the next after [`EVERY`], while the app runs.
fn intake_tick(back: &Back) {
    let running = back.with(|cx| {
        let leitstand = cx.leitstand();
        if leitstand.shared.borrow().quitting {
            return false;
        }
        leitstand.poll_intake();
        true
    });
    if running == Some(true) {
        let back = back.clone();
        crate::util::main_after(EVERY, move || intake_tick(&back));
    }
}
