//! The `leitstand-order` scenario (B42, F61, R12): one work order runs through every station on a git repository of
//! its own with the scripted session in place of an agent and a gate command that passes (`true`). Only Claude counts
//! as ready, so the order runs in stage 1: the specification is skipped, the builder writes a file in the order's
//! worktree, the factory commits it, the path scope, the secrets scan and the gate pass, the reviewer agent is skipped,
//! and the evidence waits for acceptance; G2 takes it over into the base branch of the working copy and the worktree
//! goes. The order holds the wake lock while it runs and lets it go while it waits; no notification reaches the user
//! and no activity keeps the Mac awake.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use ide_factory::options::NewOrder;
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::{Condition, OrderId, Station, WaitReason};
use ide_git::CommitOptions;
use ide_session::{End, Script, ScriptTurn, Step};

use super::super::git::{append, fixture_git};
use super::super::helpers::{self, wait_within};
use super::super::{check, finish, give_up, give_up_saying};
use crate::leitstand::Command;
use crate::util::main_soon;

const SCENARIO: &str = "leitstand-order";
const PROJECT: &str = "leitstand-order-project";
/// The file the scripted builder writes.
const WRITTEN: &str = "hello.txt";
const CONTENT: &str = "hello\n";
const WORKFLOW: &str = "[[gates.command]]\nname = \"check\"\nrun = \"true\"\n";
/// How long a run of stations may take: git, the gate's sandbox and the disk lane, under the load of parallel runs.
const STATIONS: Duration = Duration::from_secs(120);
const SHORT: Duration = Duration::from_secs(20);

pub(crate) fn run() {
    let cx = crate::context::app();
    cx.leitstand().use_script(Some(script()));
    cx.leitstand().use_agents(Some(BTreeMap::from([(
        Agent::Claude,
        AgentStatus::ready(&Agent::Claude),
    )])));
    let files = [("README.md", "# Order\n"), (".dango/factory.toml", WORKFLOW)];
    let root = match helpers::make_project(PROJECT, &files) {
        Ok(root) => root,
        Err(error) => return give_up_saying(SCENARIO, "the scenario's folder is made", error.to_string()),
    };
    std::thread::spawn(move || {
        let made = commit(&root);
        main_soon(move || match made {
            Ok(()) => opened(root),
            Err(error) => give_up_saying(SCENARIO, "the test repository has its first commit", error),
        });
    });
}

/// The builder's one turn: it confirms the criteria and writes the file.
fn script() -> Script {
    Script::new(vec![ScriptTurn::new(
        vec![
            Step::Say("I confirm the criteria.".into()),
            Step::Write {
                path: WRITTEN.into(),
                contents: CONTENT.into(),
            },
            Step::Say("Done.".into()),
        ],
        End::Succeed,
    )])
}

/// Makes the folder a repository with everything in its first commit.
pub(super) fn commit(root: &Path) -> Result<(), String> {
    let git = fixture_git()?;
    let repo = git.init(root).map_err(|e| e.to_string())?;
    append(
        &repo.git_dir().join("config"),
        "[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n[commit]\n\tgpgsign = false\n",
    )?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", CommitOptions::default())
        .map_err(|e| e.to_string())
        .map(|_| ())
}

/// Opens the repository trusted and waits for its factory to read the workflow file.
fn opened(root: PathBuf) {
    let root = helpers::open_trusted(&root);
    let probe = root.clone();
    wait_within(
        STATIONS,
        move || crate::context::app().leitstand().has_gates(&probe),
        move |ready| {
            if !ready {
                return give_up(SCENARIO, "the folder's factory reads the workflow file");
            }
            confirm(root);
        },
    );
}

/// Confirms the gate command, as the Leitstand's dialog does (E190).
fn confirm(root: PathBuf) {
    let cx = crate::context::app();
    let Some((hash, _)) = cx.leitstand().gates(&root) else {
        return give_up(SCENARIO, "the folder has a factory");
    };
    cx.leitstand().command(&cx, &root, Command::ConfirmGates(hash));
    let probe = root.clone();
    wait_within(
        SHORT,
        move || {
            crate::context::app()
                .leitstand()
                .gates(&probe)
                .is_some_and(|(_, confirmed)| confirmed)
        },
        move |confirmed| {
            if !confirmed {
                return give_up(SCENARIO, "the gate command is confirmed");
            }
            start(root);
        },
    );
}

/// The order's station, condition and phase, for a report.
fn state(root: &Path, id: &OrderId) -> String {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| {
            format!(
                "{:?} {:?} {:?}, visits {:?}",
                o.station(),
                o.condition(),
                o.phase(),
                o.visits().iter().map(|v| v.station).collect::<Vec<_>>()
            )
        })
        .unwrap_or_else(|| "no such order".into())
}

/// Starts the order from the sheet's fields and waits until its evidence waits for acceptance.
fn start(root: PathBuf) {
    let cx = crate::context::app();
    let notices = ide_ui_common::notify::posted();
    let order = NewOrder {
        title: "Gruß schreiben".into(),
        goal: "Write a greeting into hello.txt.".into(),
        criteria: "- hello.txt says hello\n".into(),
        ..NewOrder::default()
    };
    let Some(id) = cx.leitstand().command(&cx, &root, Command::New(order)) else {
        return give_up(SCENARIO, "the order comes in");
    };
    let (probe, waited) = ((root.clone(), id.clone()), id.clone());
    let held = Rc::new(Cell::new(false));
    let seen_held = Rc::clone(&held);
    wait_within(
        STATIONS,
        move || {
            let cx = crate::context::app();
            let condition = cx.leitstand().with_order(&probe.0, &probe.1, |o| o.condition().clone());
            if condition == Some(Condition::Running) && cx.leitstand().wake_lock().requested {
                seen_held.set(true);
            }
            condition == Some(Condition::Waiting(WaitReason::Acceptance))
        },
        move |ready| {
            if !ready {
                return give_up_saying(SCENARIO, "the order waits for acceptance", state(&root, &waited));
            }
            check(
                "the order holds the wake lock while it runs",
                held.get(),
                "no look at the running order found it held",
            );
            at_acceptance(&root, &waited, notices);
        },
    );
}

/// What the order shows at acceptance: its stations, its gates, its evidence and the builder's file.
struct Seen {
    stations: Vec<Station>,
    spec_skipped: bool,
    gates: Vec<(String, bool, bool)>,
    evidence: Option<(String, bool)>,
}

fn at_acceptance(root: &Path, id: &OrderId, notices: usize) {
    let cx = crate::context::app();
    let seen = cx.leitstand().with_order(root, id, |o| Seen {
        stations: o
            .visits()
            .iter()
            .map(|v| v.station)
            .filter(|s| matches!(s, Station::Build | Station::Review | Station::Acceptance))
            .collect(),
        spec_skipped: o.g1_skipped(),
        gates: o
            .gates()
            .iter()
            .map(|g| (g.name.clone(), g.failure.is_none(), g.skipped))
            .collect(),
        evidence: o
            .evidence()
            .map(|e| (e.fingerprint().to_string(), e.invalid().is_none())),
    });
    let Some(seen) = seen else {
        return give_up(SCENARIO, "the order is there at acceptance");
    };
    check(
        "the order went through build and review to acceptance, its specification skipped in stage 1",
        seen.stations == [Station::Build, Station::Review, Station::Acceptance] && seen.spec_skipped,
        format!("{:?}, specification skipped: {}", seen.stations, seen.spec_skipped),
    );
    check(
        "every gate passed, the gate command among them, and the reviewer agent was skipped",
        seen.gates.iter().all(|(_, passed, _)| *passed)
            && seen.gates.iter().any(|(name, _, skipped)| name == "check" && !skipped)
            && seen
                .gates
                .iter()
                .any(|(name, _, skipped)| name == "reviewer" && *skipped),
        format!("{:?}", seen.gates),
    );
    let worktree = cx.leitstand().worktree(root, id);
    let built = worktree
        .as_ref()
        .and_then(|tree| std::fs::read_to_string(tree.join(WRITTEN)).ok());
    check(
        "the builder's file is in the order's worktree and not yet in the working copy",
        built.as_deref() == Some(CONTENT) && !root.join(WRITTEN).exists(),
        format!("{worktree:?}: {built:?}"),
    );
    check(
        "the evidence is bound to a fingerprint and valid",
        seen.evidence
            .as_ref()
            .is_some_and(|(print, valid)| !print.is_empty() && *valid),
        format!("{:?}", seen.evidence),
    );
    quiet(notices);
    let Some((fingerprint, _)) = seen.evidence else {
        return give_up(SCENARIO, "the order has evidence to accept");
    };
    accept(root.to_path_buf(), id.clone(), fingerprint);
}

/// Neither a notification nor the activity that keeps the Mac awake reaches past the self-test (R12).
fn quiet(notices: usize) {
    let posted = ide_ui_common::notify::posted();
    check(
        "the order's notifications are counted but none reaches the user during the self-test",
        !ide_ui_common::notify::may_post() && posted > notices,
        format!("{} notifications counted", posted - notices),
    );
    let lock = crate::context::app().leitstand().wake_lock();
    check(
        "the wake lock is let go while the order waits for acceptance, and no activity kept the Mac awake",
        lock.asked && !lock.requested && !lock.activity,
        format!("{lock:?}"),
    );
}

/// G2: accepts the evidence whole and waits until the order is taken over and its worktree is gone.
fn accept(root: PathBuf, id: OrderId, fingerprint: String) {
    let cx = crate::context::app();
    let command = Command::Accept {
        order: id.clone(),
        fingerprint,
        rejected: Vec::new(),
    };
    cx.leitstand().command(&cx, &root, command);
    let probe = (root.clone(), id.clone());
    wait_within(
        STATIONS,
        move || {
            crate::context::app()
                .leitstand()
                .with_order(&probe.0, &probe.1, |o| o.station() == Station::Done)
                .unwrap_or(false)
        },
        move |done| {
            if !done {
                return give_up_saying(SCENARIO, "the order is taken over and done", state(&root, &id));
            }
            taken_over(&root, &id);
        },
    );
}

fn taken_over(root: &Path, id: &OrderId) {
    let cx = crate::context::app();
    let over = cx.leitstand().with_order(root, id, |o| o.taken_over()).unwrap_or(false);
    let file = std::fs::read_to_string(root.join(WRITTEN)).ok();
    check(
        "accepting takes the order over into the working copy's base branch",
        over && file.as_deref() == Some(CONTENT),
        format!("taken over: {over}, {WRITTEN}: {file:?}"),
    );
    let on_disk: Vec<PathBuf> = ide_git::thread_trees::list(root)
        .into_iter()
        .filter(|tree| tree.order.as_deref() == Some(id.0.as_str()))
        .map(|tree| tree.path)
        .collect();
    check(
        "the order's worktree is gone after the take-over",
        cx.leitstand().worktree(root, id).is_none() && on_disk.is_empty(),
        format!("{:?}, on disk {on_disk:?}", cx.leitstand().worktree(root, id)),
    );
    let lock = cx.leitstand().wake_lock();
    check(
        "the wake lock stays let go once the order is done",
        !lock.requested && !lock.activity,
        format!("{lock:?}"),
    );
    cx.leitstand().use_script(None);
    cx.leitstand().use_agents(None);
    finish(SCENARIO);
}
