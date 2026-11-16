//! The `leitstand-queue` scenario (B45b, E174, E168): four scripted orders in stage 3, with Claude and Codex counted
//! as ready so a planner and a reviewer of another vendor take part. Two orders whose path scopes overlap build one
//! after the other: the second waits with its reason shown until the first's diff is done. Two with disjoint scopes
//! build side by side. Accepting all four at once queues their take-overs into the base branch, one at a time, each
//! merging what was taken over before it and checked again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_factory::event::Phase;
use ide_factory::options::NewOrder;
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::{Condition, OrderId, Role, Station, WaitReason};
use ide_git::CommitOptions;
use ide_session::{End, Script, ScriptTurn, Step};

use super::super::git::{append, fixture_git};
use super::super::helpers::{self, wait_within};
use super::super::{check, finish, give_up, give_up_saying};
use crate::leitstand::Command;
use crate::util::main_soon;

const SCENARIO: &str = "leitstand-queue";
const PROJECT: &str = "leitstand-queue-project";
const WORKFLOW: &str = "[[gates.command]]\nname = \"check\"\nrun = \"true\"\n";
/// How long a phase of the scenario may take: planner, builder, gate and reviewer of two orders, under load.
const PHASE: Duration = Duration::from_secs(240);
const SHORT: Duration = Duration::from_secs(20);
const CRITERION: &str = "the file says hello";

/// The orders: title, path scope and the file its builder writes.
const OVERLAPPING: [(&str, &str, &str); 2] = [
    ("Gemeinsam A", "shared/**", "shared/a.txt"),
    ("Gemeinsam B", "shared/b.txt", "shared/b.txt"),
];
const DISJOINT: [(&str, &str, &str); 2] = [("Getrennt C", "c/**", "c/c.txt"), ("Getrennt D", "d/**", "d/d.txt")];

pub(crate) fn run() {
    let cx = crate::context::app();
    cx.leitstand().use_scripts(scripts());
    let ready = [Agent::Claude, Agent::Codex].map(|a| (a.clone(), AgentStatus::ready(&a)));
    cx.leitstand().use_agents(Some(BTreeMap::from(ready)));
    let files = [("README.md", "# Queue\n"), (".dango/factory.toml", WORKFLOW)];
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

/// A turn that says `text` and succeeds.
fn saying(text: &str) -> ScriptTurn {
    ScriptTurn::new(vec![Step::Say(text.into())], End::Succeed)
}

/// The planner's specification for every order, the reviewer's verdict for every round, and each builder's file.
fn scripts() -> BTreeMap<(String, Role), Script> {
    let spec = format!("# Spec\n\n## Criteria\n- {CRITERION}\n\n## Risk\nlow\n");
    let verdict = r#"{"criteria":[{"id":"K1","verdict":"met","reason":"the file says hello"}],"findings":[]}"#;
    let mut scripts = BTreeMap::from([
        ((String::new(), Role::Planner), Script::new(vec![saying(&spec)])),
        (
            (String::new(), Role::Reviewer),
            Script::new(vec![saying(verdict), saying(verdict), saying(verdict)]),
        ),
    ]);
    for (title, _, file) in OVERLAPPING.into_iter().chain(DISJOINT) {
        let turn = ScriptTurn::new(
            vec![
                Step::Say("I confirm the criteria.".into()),
                Step::Write {
                    path: file.into(),
                    contents: "hello\n".into(),
                },
                Step::Say("Done.".into()),
            ],
            End::Succeed,
        );
        scripts.insert((title.to_string(), Role::Builder), Script::new(vec![turn]));
    }
    scripts
}

/// Makes the folder a repository with everything in its first commit.
fn commit(root: &Path) -> Result<(), String> {
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

/// Opens the repository trusted, waits for its factory to read the workflow file and confirms the gate command.
fn opened(root: PathBuf) {
    let root = helpers::open_trusted(&root);
    let probe = root.clone();
    wait_within(
        PHASE,
        move || crate::context::app().leitstand().has_gates(&probe),
        move |ready| {
            if !ready {
                return give_up(SCENARIO, "the folder's factory reads the workflow file");
            }
            let cx = crate::context::app();
            let Some((hash, _)) = cx.leitstand().gates(&root) else {
                return give_up(SCENARIO, "the folder has a factory");
            };
            cx.leitstand().command(&cx, &root, Command::ConfirmGates(hash));
            let probe = root.clone();
            wait_within(
                SHORT,
                move || {
                    let gates = crate::context::app().leitstand().gates(&probe);
                    gates.is_some_and(|(_, confirmed)| confirmed)
                },
                move |confirmed| match confirmed {
                    true => overlapping(root),
                    false => give_up(SCENARIO, "the gate command is confirmed"),
                },
            );
        },
    );
}

/// Starts the orders of `orders` and hands their ids to `then` once each waits for G1.
fn start(root: PathBuf, orders: [(&'static str, &'static str, &'static str); 2], then: fn(PathBuf, Vec<OrderId>)) {
    let cx = crate::context::app();
    let mut ids = Vec::new();
    for (title, scope, _) in orders {
        let order = NewOrder {
            title: title.into(),
            goal: format!("Write hello into the file of {title}."),
            criteria: format!("- {CRITERION}\n"),
            scope: vec![scope.into()],
            ..NewOrder::default()
        };
        let Some(id) = cx.leitstand().command(&cx, &root, Command::New(order)) else {
            return give_up(SCENARIO, "the orders come in");
        };
        ids.push(id);
    }
    let (probe, waited) = ((root.clone(), ids.clone()), ids);
    wait_within(
        PHASE,
        move || {
            probe
                .1
                .iter()
                .all(|id| condition(&probe.0, id) == Some(Condition::Waiting(WaitReason::SpecApproval)))
        },
        move |ready| {
            if !ready {
                let states: Vec<String> = waited.iter().map(|id| state(&root, id)).collect();
                return give_up_saying(SCENARIO, "every order waits for G1", states.join("; "));
            }
            then(root, waited);
        },
    );
}

fn condition(root: &Path, id: &OrderId) -> Option<Condition> {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| o.condition().clone())
}

/// The order's station, condition and phase, for a report.
fn state(root: &Path, id: &OrderId) -> String {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| {
            format!("{id}: {:?} {:?} {:?}", o.station(), o.condition(), o.phase())
        })
        .unwrap_or_else(|| format!("{id}: no such order"))
}

/// When the order first entered `station`, wall clock.
fn entered(root: &Path, id: &OrderId, station: Station) -> Option<u64> {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| {
            o.visits().iter().find(|v| v.station == station).map(|v| v.at.wall)
        })
        .flatten()
}

/// Approves every order's specification in one go, so none builds before the factory saw them all.
fn approve(root: &Path, ids: &[OrderId]) {
    let cx = crate::context::app();
    for id in ids {
        cx.leitstand().command(&cx, root, Command::Approve(id.clone()));
    }
}

/// Waits until every order of `ids` waits for acceptance, then goes on with `then`.
fn until_acceptance(root: PathBuf, ids: Vec<OrderId>, then: fn(PathBuf, Vec<OrderId>)) {
    let probe = (root.clone(), ids.clone());
    wait_within(
        PHASE,
        move || {
            probe
                .1
                .iter()
                .all(|id| condition(&probe.0, id) == Some(Condition::Waiting(WaitReason::Acceptance)))
        },
        move |ready| {
            if !ready {
                let states: Vec<String> = ids.iter().map(|id| state(&root, id)).collect();
                return give_up_saying(SCENARIO, "the orders wait for acceptance", states.join("; "));
            }
            then(root, ids);
        },
    );
}

fn overlapping(root: PathBuf) {
    start(root, OVERLAPPING, |root, ids| {
        approve(&root, &ids);
        let held = crate::context::app()
            .leitstand()
            .with_order(&root, &ids[1], |o| (o.station(), o.phase().clone()));
        check(
            "of two orders whose path scopes overlap, the first builds and the second waits in its slot",
            entered(&root, &ids[0], Station::Build).is_some() && held == Some((Station::Spec, Phase::Slot)),
            format!("{}; {}", state(&root, &ids[0]), state(&root, &ids[1])),
        );
        let words = crate::context::app()
            .leitstand()
            .detail(&root, &ids[1])
            .map(|d| d.condition().to_string());
        check(
            "the waiting order names the order whose path scope it overlaps",
            words.as_deref().is_some_and(|w| w.contains(OVERLAPPING[0].0)),
            format!("{words:?}"),
        );
        until_acceptance(root, ids, |root, ids| {
            let (first, second) = (
                entered(&root, &ids[0], Station::Acceptance),
                entered(&root, &ids[1], Station::Build),
            );
            check(
                "the second order builds only once the first's diff is done",
                matches!((first, second), (Some(done), Some(began)) if began >= done),
                format!("first at acceptance {first:?}, second built from {second:?}"),
            );
            disjoint(root);
        });
    });
}

/// With the first two waiting for acceptance, which holds no scope, the next two may build at once.
fn disjoint(root: PathBuf) {
    start(root, DISJOINT, |root, ids| {
        approve(&root, &ids);
        let building: Vec<Option<Station>> = ids
            .iter()
            .map(|id| crate::context::app().leitstand().with_order(&root, id, |o| o.station()))
            .collect();
        check(
            "two orders with disjoint path scopes both enter the build",
            building.iter().all(|s| *s == Some(Station::Build)),
            format!("{building:?}"),
        );
        until_acceptance(root, ids, |root, ids| {
            let overlap = (0..2).all(|i| {
                let built = entered(&root, &ids[i], Station::Build);
                let other_done = entered(&root, &ids[1 - i], Station::Acceptance);
                matches!((built, other_done), (Some(b), Some(d)) if b <= d)
            });
            check(
                "the two orders built side by side",
                overlap,
                ids.iter().map(|id| state(&root, id)).collect::<Vec<_>>().join("; "),
            );
            take_over_all(root);
        });
    });
}

fn take_over_all(root: PathBuf) {
    let cx = crate::context::app();
    let Some(board) = cx.leitstand().board(&root) else {
        return give_up(SCENARIO, "the folder has a board");
    };
    let mut ids: Vec<OrderId> = board.rows().iter().map(|r| r.id().clone()).collect();
    ids.sort();
    for id in &ids {
        let fingerprint = cx
            .leitstand()
            .with_order(&root, id, |o| o.evidence().map(|e| e.fingerprint().to_string()))
            .flatten();
        let Some(fingerprint) = fingerprint else {
            return give_up_saying(SCENARIO, "every order has evidence to accept", state(&root, id));
        };
        let command = Command::Accept {
            order: id.clone(),
            fingerprint,
            rejected: Vec::new(),
        };
        cx.leitstand().command(&cx, &root, command);
    }
    let queued = ids
        .iter()
        .filter(|id| cx.leitstand().with_order(&root, id, |o| *o.phase() == Phase::Queue) == Some(true))
        .count();
    check(
        "accepting four orders at once takes one over and queues the other three",
        queued == ids.len().saturating_sub(1),
        ids.iter().map(|id| state(&root, id)).collect::<Vec<_>>().join("; "),
    );
    let probe = (root.clone(), ids.clone());
    wait_within(
        PHASE * 2,
        move || {
            let cx = crate::context::app();
            let leitstand = cx.leitstand();
            probe
                .1
                .iter()
                .all(|id| leitstand.with_order(&probe.0, id, |o| o.station() == Station::Done) == Some(true))
        },
        move |done| {
            if !done {
                let states: Vec<String> = ids.iter().map(|id| state(&root, id)).collect();
                return give_up_saying(SCENARIO, "every order is taken over", states.join("; "));
            }
            taken_over(&root);
        },
    );
}

fn taken_over(root: &Path) {
    let files: Vec<(&str, bool)> = OVERLAPPING
        .into_iter()
        .chain(DISJOINT)
        .map(|(_, _, file)| (file, root.join(file).exists()))
        .collect();
    check(
        "every order's file is in the working copy after the take-overs, one merged after the other",
        files.iter().all(|(_, there)| *there),
        format!("{files:?}"),
    );
    let cx = crate::context::app();
    cx.leitstand().use_scripts(BTreeMap::new());
    cx.leitstand().use_agents(None);
    finish(SCENARIO);
}
