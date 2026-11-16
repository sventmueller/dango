//! The `leitstand-hunks` scenario (C55): the order tab is opened while the builder's turn runs, before it changed
//! anything, so the hunks it reads then are none; at acceptance it offers the hunk of the builder's change all the
//! same, read anew for the diff the order holds now. Then the file changes in the worktree, and a rejection chosen from
//! the hunk shown before is refused instead of reversing another change: the order fails at "reject hunks" and the
//! file keeps what it says.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use ide_factory::event::Phase;
use ide_factory::options::NewOrder;
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::{Condition, FailReason, OrderId, WaitReason};
use ide_session::{End, Script, ScriptTurn, Step};

use super::super::helpers::wait_within;
use super::super::{check, finish, give_up, give_up_saying};
use crate::leitstand::Command;

const SCENARIO: &str = "leitstand-hunks";
const PROJECT: &str = "leitstand-hunks-project";
const WORKFLOW: &str = "[[gates.command]]\nname = \"check\"\nrun = \"true\"\n";
const FILE: &str = "README.md";
const BEFORE: &str = "# Hunks\n";
const BUILT: &str = "# Hunks\n\nChanged by the builder.\n";
const BY_HAND: &str = "# Hunks\n\nChanged by hand.\n";
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
    let files = [(FILE, BEFORE), (".dango/factory.toml", WORKFLOW)];
    super::folder::ready(SCENARIO, PROJECT, &files, start);
}

/// The builder's one turn: it waits a moment, so the order tab reads the diff before the change, then changes the
/// file.
fn script() -> Script {
    Script::new(vec![ScriptTurn::new(
        vec![
            Step::Say("I confirm the criteria.".into()),
            Step::Pause(Duration::from_secs(5)),
            Step::Write {
                path: FILE.into(),
                contents: BUILT.into(),
            },
            Step::Say("Done.".into()),
        ],
        End::Succeed,
    )])
}

fn state(root: &Path, id: &OrderId) -> String {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| {
            format!("{:?} {:?} {:?}", o.station(), o.condition(), o.phase())
        })
        .unwrap_or_else(|| "no such order".into())
}

/// The hunks the order tab lists for `id` now.
fn shown(root: &Path, id: &OrderId) -> Vec<(ide_factory::HunkId, String)> {
    crate::context::app()
        .leitstand()
        .detail(root, id)
        .map(|detail| detail.sections().iter().map(|s| (s.id().clone(), s.title())).collect())
        .unwrap_or_default()
}

/// Starts the order, opens its tab once the builder's turn runs, and waits until the order waits for acceptance.
fn start(root: PathBuf) {
    let cx = crate::context::app();
    let order = NewOrder {
        title: "Abschnitte prüfen".into(),
        goal: "Change README.md.".into(),
        criteria: "- README.md is changed\n".into(),
        ..NewOrder::default()
    };
    let Some(id) = cx.leitstand().command(&cx, &root, Command::New(order)) else {
        return give_up(SCENARIO, "the order comes in");
    };
    let opened = Rc::new(Cell::new(None::<usize>));
    let seen = Rc::clone(&opened);
    let probe = (root.clone(), id.clone());
    wait_within(
        STATIONS,
        move || {
            let cx = crate::context::app();
            let turn = cx
                .leitstand()
                .with_order(&probe.0, &probe.1, |o| matches!(o.phase(), Phase::Turn { .. }));
            if turn == Some(true) && seen.get().is_none() {
                seen.set(Some(shown(&probe.0, &probe.1).len()));
            }
            cx.leitstand()
                .with_order(&probe.0, &probe.1, |o| {
                    *o.condition() == Condition::Waiting(WaitReason::Acceptance)
                })
                .unwrap_or(false)
        },
        move |ready| {
            if !ready {
                return give_up_saying(SCENARIO, "the order waits for acceptance", state(&root, &id));
            }
            check(
                "the order tab was opened while the builder's turn ran, before the change",
                opened.get().is_some(),
                format!("hunks shown then: {:?}", opened.get()),
            );
            at_acceptance(root, id);
        },
    );
}

/// The hunk of the builder's change shows at acceptance, read for the diff the order holds now.
fn at_acceptance(root: PathBuf, id: OrderId) {
    let probe = (root.clone(), id.clone());
    wait_within(
        SHORT,
        move || !shown(&probe.0, &probe.1).is_empty(),
        move |_| {
            let hunks = shown(&root, &id);
            check(
                "at acceptance the order tab offers the hunk of the builder's change, though it read none before",
                hunks.len() == 1 && hunks[0].1.starts_with(FILE),
                format!("{hunks:?}"),
            );
            let Some((hunk, _)) = hunks.into_iter().next() else {
                return give_up(SCENARIO, "the order tab offers a hunk");
            };
            reject_stale(root, id, hunk);
        },
    );
}

/// Changes the file in the worktree, then rejects the hunk shown before: the order refuses and reverses nothing.
fn reject_stale(root: PathBuf, id: OrderId, hunk: ide_factory::HunkId) {
    let cx = crate::context::app();
    let Some(worktree) = cx.leitstand().worktree(&root, &id) else {
        return give_up(SCENARIO, "the order has its worktree");
    };
    if let Err(error) = std::fs::write(worktree.join(FILE), BY_HAND) {
        return give_up_saying(SCENARIO, "the file changes in the worktree", error.to_string());
    }
    let Some(fingerprint) = cx
        .leitstand()
        .with_order(&root, &id, |o| o.evidence().map(|e| e.fingerprint().to_string()))
        .flatten()
    else {
        return give_up(SCENARIO, "the order has evidence to accept");
    };
    let command = Command::Accept {
        order: id.clone(),
        fingerprint,
        rejected: vec![hunk],
    };
    cx.leitstand().command(&cx, &root, command);
    let probe = (root.clone(), id.clone());
    wait_within(
        STATIONS,
        move || {
            crate::context::app()
                .leitstand()
                .with_order(&probe.0, &probe.1, |o| matches!(o.condition(), Condition::Failed(_)))
                .unwrap_or(false)
        },
        move |failed| {
            let refused = crate::context::app().leitstand().with_order(&root, &id, |o| {
                matches!(o.condition(), Condition::Failed(FailReason::Effect { effect, .. }) if effect == "reject hunks")
            });
            check(
                "a rejection chosen from the diff before the file changed is refused",
                failed && refused == Some(true),
                state(&root, &id),
            );
            let kept = std::fs::read_to_string(worktree.join(FILE)).ok();
            check(
                "the refused rejection reverses nothing in the worktree",
                kept.as_deref() == Some(BY_HAND),
                format!("{kept:?}"),
            );
            let cx = crate::context::app();
            cx.leitstand().command(&cx, &root, Command::Abort(id));
            cx.leitstand().use_script(None);
            cx.leitstand().use_agents(None);
            finish(SCENARIO);
        },
    );
}
