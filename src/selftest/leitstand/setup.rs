//! The `leitstand-setup` scenario (E188, C67): the workflow file's setup command hangs (`sleep 600`). Aborting an order
//! while its worktree's setup runs ends the command; a second order pauses as its folder loses its last window, which
//! ends its setup command too, and resuming prepares the worktree again with a new run of the command.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_factory::event::Phase;
use ide_factory::options::NewOrder;
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::{Condition, OrderId, PauseReason};
use ide_process::Supervisor;
use ide_session::{End, Script, ScriptTurn, Step};

use super::super::helpers::{wait_within, window_of};
use super::super::{check, finish, give_up, give_up_saying};
use crate::leitstand::Command;

const SCENARIO: &str = "leitstand-setup";
const PROJECT: &str = "leitstand-setup-project";
const WORKFLOW: &str = "[worktree]\nsetup = \"sleep 600\"\n\n[[gates.command]]\nname = \"check\"\nrun = \"true\"\n";
/// How long an order may take to reach its setup: git and the sandbox, under the load of parallel runs.
const STATIONS: Duration = Duration::from_secs(120);
const SHORT: Duration = Duration::from_secs(20);

pub(crate) fn run() {
    let cx = crate::context::app();
    cx.leitstand().use_script(Some(Script::new(vec![ScriptTurn::new(
        vec![Step::Say("Done.".into())],
        End::Succeed,
    )])));
    cx.leitstand().use_agents(Some(BTreeMap::from([(
        Agent::Claude,
        AgentStatus::ready(&Agent::Claude),
    )])));
    let files = [("README.md", "# Setup\n"), (".dango/factory.toml", WORKFLOW)];
    super::folder::ready(SCENARIO, PROJECT, &files, |root| {
        start(root, "Einrichtung abbrechen", abort_during_setup);
    });
}

/// Whether the setup command of `id` runs now, as the supervisor knows it.
fn setup_runs(id: &OrderId) -> bool {
    let label = splat_i18n::tf("Einrichtung von „{}“", &[&id.0]);
    Supervisor::global().running().iter().any(|p| p.label == label)
}

fn state(root: &Path, id: &OrderId) -> String {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| {
            format!("{:?} {:?} {:?}", o.station(), o.condition(), o.phase())
        })
        .unwrap_or_else(|| "no such order".into())
}

fn order_is(root: &Path, id: &OrderId, test: impl FnOnce(&ide_factory::Order) -> bool) -> bool {
    crate::context::app()
        .leitstand()
        .with_order(root, id, test)
        .unwrap_or(false)
}

/// Starts the order titled `title` and calls `then` once its setup command runs.
fn start(root: PathBuf, title: &str, then: fn(PathBuf, OrderId)) {
    let cx = crate::context::app();
    let order = NewOrder {
        title: title.into(),
        goal: "Nothing to change.".into(),
        criteria: "- nothing changes\n".into(),
        ..NewOrder::default()
    };
    let Some(id) = cx.leitstand().command(&cx, &root, Command::New(order)) else {
        return give_up(SCENARIO, "the order comes in");
    };
    let probe = id.clone();
    wait_within(
        STATIONS,
        move || setup_runs(&probe),
        move |runs| {
            if !runs {
                return give_up_saying(SCENARIO, "the order's setup command runs", state(&root, &id));
            }
            then(root, id);
        },
    );
}

/// Aborting the order ends its setup command, and the order ends aborted.
fn abort_during_setup(root: PathBuf, id: OrderId) {
    let cx = crate::context::app();
    cx.leitstand().command(&cx, &root, Command::Abort(id.clone()));
    let probe = (root.clone(), id.clone());
    wait_within(
        SHORT,
        move || {
            !setup_runs(&probe.1)
                && order_is(&probe.0, &probe.1, |o| {
                    *o.condition() == Condition::Aborted && *o.phase() == Phase::Idle
                })
        },
        move |stopped| {
            check(
                "aborting an order ends its running setup command, and the order ends aborted",
                stopped,
                format!("setup runs: {}, {}", setup_runs(&id), state(&root, &id)),
            );
            start(root, "Einrichtung pausieren", pause_during_setup);
        },
    );
}

/// The folder loses its last window: the order pauses and its setup command ends.
fn pause_during_setup(root: PathBuf, id: OrderId) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let cx = crate::context::app();
    cx.leitstand().folder_closed(&cx, &controller);
    let probe = (root.clone(), id.clone());
    wait_within(
        SHORT,
        move || {
            !setup_runs(&probe.1)
                && order_is(&probe.0, &probe.1, |o| {
                    *o.condition() == Condition::Paused(PauseReason::NoWindow) && *o.phase() == Phase::Idle
                })
        },
        move |stopped| {
            check(
                "a pause without a window of the folder ends the setup command",
                stopped,
                format!("setup runs: {}, {}", setup_runs(&id), state(&root, &id)),
            );
            resume(root, id);
        },
    );
}

/// The folder shows again: the order resumes, prepares its worktree again and runs the setup command anew.
fn resume(root: PathBuf, id: OrderId) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let cx = crate::context::app();
    cx.leitstand().attached(&cx, &controller);
    let probe = (root.clone(), id.clone());
    wait_within(
        STATIONS,
        move || setup_runs(&probe.1) && order_is(&probe.0, &probe.1, |o| *o.condition() == Condition::Running),
        move |again| {
            check(
                "resuming prepares the worktree again with a new run of the setup command",
                again,
                state(&root, &id),
            );
            let cx = crate::context::app();
            cx.leitstand().command(&cx, &root, Command::Abort(id.clone()));
            let probe = id.clone();
            wait_within(
                SHORT,
                move || !setup_runs(&probe),
                move |ended| {
                    check("the last setup command ends with its order", ended, "");
                    let cx = crate::context::app();
                    cx.leitstand().use_script(None);
                    cx.leitstand().use_agents(None);
                    finish(SCENARIO);
                },
            );
        },
    );
}
