//! The `leitstand-pause` scenario (E166, E167, E168, E169), run twice in one home with the scripted session and a gate
//! command that hangs (`sleep 600`). The first launch lets an order run into its gate, where no session works: the
//! order holds the wake lock; the Mac's sleep pauses it and lets the lock go, waking resumes both; closing the
//! folder's last window asks first, naming the order, and then pauses it; opening the folder again resumes it; then it
//! quits as the user does. The second launch finds the order paused with "app ended", written before the first one
//! ended.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_factory::event::{Event, Phase};
use ide_factory::options::NewOrder;
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::{Condition, OrderId, PauseReason};
use ide_session::{End, Script, ScriptTurn, Step};
use objc2_app_kit::NSAlertSecondButtonReturn;
use splat_i18n::{t, tf};

use super::super::helpers::{self, wait_within, window_of};
use super::super::{check, finish, give_up, give_up_saying};
use crate::leitstand::Command;
use crate::util::{main_on_run_loop, main_soon};

const SCENARIO: &str = "leitstand-pause";
const PROJECT: &str = "leitstand-pause-project";
const TITLE: &str = "Pause prüfen";
const WORKFLOW: &str = "[[gates.command]]\nname = \"hang\"\nrun = \"sleep 600\"\n";
/// How long the order may take to reach its gate: git, the sandbox and the disk lane, under the load of parallel runs.
const STATIONS: Duration = Duration::from_secs(120);
const SHORT: Duration = Duration::from_secs(20);

pub(crate) fn run() {
    let cx = crate::context::app();
    cx.leitstand().use_agents(Some(BTreeMap::from([(
        Agent::Claude,
        AgentStatus::ready(&Agent::Claude),
    )])));
    if std::env::var("IDE_SELFTEST_LAUNCH").as_deref() == Ok("quit") {
        first_launch();
    } else {
        second_launch();
    }
}

/// The builder's turns, each writing the same file; a rework after a stopped gate takes the next one.
fn script() -> Script {
    let turn = || {
        ScriptTurn::new(
            vec![
                Step::Write {
                    path: "pause.txt".into(),
                    contents: "paused\n".into(),
                },
                Step::Say("Done.".into()),
            ],
            End::Succeed,
        )
    };
    Script::new(vec![turn(), turn(), turn()])
}

fn first_launch() {
    crate::context::app().leitstand().use_script(Some(script()));
    let files = [("README.md", "# Pause\n"), (".dango/factory.toml", WORKFLOW)];
    let root = match helpers::make_project(PROJECT, &files) {
        Ok(root) => root,
        Err(error) => return give_up_saying(SCENARIO, "the scenario's folder is made", error.to_string()),
    };
    std::thread::spawn(move || {
        let made = super::order::commit(&root);
        main_soon(move || match made {
            Ok(()) => opened(helpers::open_trusted(&root)),
            Err(error) => give_up_saying(SCENARIO, "the test repository has its first commit", error),
        });
    });
}

/// Waits for the folder's factory to read the workflow file and confirms the gate command, as the Leitstand's dialog
/// does (E190).
fn opened(root: PathBuf) {
    let probe = root.clone();
    wait_within(
        STATIONS,
        move || crate::context::app().leitstand().has_gates(&probe),
        move |ready| {
            let cx = crate::context::app();
            let Some((hash, _)) = cx.leitstand().gates(&root).filter(|_| ready) else {
                return give_up(SCENARIO, "the folder's factory reads the workflow file");
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
        },
    );
}

/// Starts the order and waits until it runs its gate.
fn start(root: PathBuf) {
    let cx = crate::context::app();
    let order = NewOrder {
        title: TITLE.into(),
        goal: "Write pause.txt.".into(),
        criteria: "- pause.txt says paused\n".into(),
        ..NewOrder::default()
    };
    let Some(id) = cx.leitstand().command(&cx, &root, Command::New(order)) else {
        return give_up(SCENARIO, "the order comes in");
    };
    let probe = (root.clone(), id.clone());
    wait_within(
        STATIONS,
        move || {
            order_is(&probe.0, &probe.1, |o| {
                matches!(o.phase(), Phase::Gate { .. }) && *o.condition() == Condition::Running
            })
        },
        move |in_gate| {
            if !in_gate {
                return give_up_saying(SCENARIO, "the order runs its gate", state(&root, &id));
            }
            sleep_and_wake(root, id);
        },
    );
}

/// Whether the order `id` of `root` is there and `test` holds for it.
fn order_is(root: &Path, id: &OrderId, test: impl FnOnce(&ide_factory::Order) -> bool) -> bool {
    crate::context::app()
        .leitstand()
        .with_order(root, id, test)
        .unwrap_or(false)
}

fn state(root: &Path, id: &OrderId) -> String {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| {
            format!("{:?} {:?} {:?}", o.station(), o.condition(), o.phase())
        })
        .unwrap_or_else(|| "no such order".into())
}

/// With no session working, the order in its gate still holds the wake lock; sleep pauses it and lets the lock go,
/// waking resumes it and takes the lock again.
fn sleep_and_wake(root: PathBuf, id: OrderId) {
    let cx = crate::context::app();
    let lock = cx.leitstand().wake_lock();
    check(
        "an order running its gate holds the wake lock, without an activity in the self-test",
        lock.requested && !lock.activity,
        format!("{lock:?}"),
    );
    cx.leitstand().simulate_sleep(&cx, true);
    let probe = (root.clone(), id.clone());
    wait_within(
        SHORT,
        move || {
            paused_for(&probe.0, &probe.1, PauseReason::Sleep)
                && !crate::context::app().leitstand().wake_lock().requested
        },
        move |asleep| {
            check(
                "the Mac's sleep pauses the order and lets the wake lock go",
                asleep,
                state(&root, &id),
            );
            let cx = crate::context::app();
            cx.leitstand().simulate_sleep(&cx, false);
            let probe = (root.clone(), id.clone());
            wait_within(
                SHORT,
                move || {
                    order_is(&probe.0, &probe.1, |o| *o.condition() == Condition::Running)
                        && crate::context::app().leitstand().wake_lock().requested
                },
                move |awake| {
                    check(
                        "waking resumes the order and takes the wake lock again",
                        awake,
                        state(&root, &id),
                    );
                    main_on_run_loop(move || close_last_window(root, id));
                },
            );
        },
    );
}

fn paused_for(root: &Path, id: &OrderId, reason: PauseReason) -> bool {
    order_is(root, id, |o| *o.condition() == Condition::Paused(reason))
}

/// Closes the folder's only window while the order runs its gate: the question names the order, "Schließen" closes
/// it, and the order pauses.
fn close_last_window(root: PathBuf, id: OrderId) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    let cx = crate::context::app();
    let name = tf("Auftrag „{}“", &[TITLE]);
    let named = cx.leitstand().would_pause(&cx, &controller);
    check(
        "closing the folder's last window would pause the order in its gate, though no session works",
        named.contains(&name) && crate::quit::running(&cx).contains(&name),
        format!("{named:?}"),
    );
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let texts = std::rc::Rc::clone(&seen);
    let asking = helpers::when_alert_shows(None, move |_| {
        *texts.borrow_mut() = helpers::alert_texts();
        helpers::answer_alert(NSAlertSecondButtonReturn);
    });
    if let Some(window) = controller.window() {
        window.performClose(None);
    }
    asking.stop();
    let seen = seen.take();
    check(
        "closing the last window asks first and names the order",
        seen.iter().any(|text| text == t("Schließen, während Aufträge laufen?"))
            && seen.iter().any(|text| text.contains(&name)),
        format!("{seen:?}"),
    );
    let probe = (root.clone(), id.clone());
    wait_within(
        SHORT,
        move || {
            window_of(&probe.0).is_none()
                && paused_for(&probe.0, &probe.1, PauseReason::NoWindow)
                && !crate::context::app().leitstand().wake_lock().requested
        },
        move |paused| {
            check(
                "without a window of its folder the order pauses and lets the wake lock go",
                paused,
                state(&root, &id),
            );
            reopen(root, id);
        },
    );
}

/// Opens the folder again, which resumes the order; then the app quits as the user does, with the order running.
fn reopen(root: PathBuf, id: OrderId) {
    helpers::open_trusted(&root);
    let probe = (root.clone(), id.clone());
    wait_within(
        SHORT,
        move || order_is(&probe.0, &probe.1, |o| *o.condition() == Condition::Running),
        move |running| {
            check("opening the folder again resumes its order", running, state(&root, &id));
            finish(SCENARIO);
        },
    );
}

/// Finds the order after the restart, in the folder's window that came back or, failing that, one opened again.
fn second_launch() {
    let root = helpers::root(PROJECT);
    let probe = root.clone();
    wait_within(
        SHORT,
        move || window_of(&probe).is_some(),
        move |shown| {
            if !shown {
                helpers::open_trusted(&root);
            }
            let probe = root.clone();
            wait_within(
                STATIONS,
                move || !crate::context::app().leitstand().order_ids(&probe).is_empty(),
                move |found| {
                    let id = crate::context::app().leitstand().order_ids(&root).into_iter().next();
                    let Some(id) = id.filter(|_| found) else {
                        return give_up(SCENARIO, "the order comes back after the restart");
                    };
                    came_back(&root, &id);
                },
            );
        },
    );
}

fn came_back(root: &Path, id: &OrderId) {
    let cx = crate::context::app();
    check(
        "after the restart the order stays paused with \"app ended\"",
        paused_for(root, id, PauseReason::AppEnded),
        state(root, id),
    );
    let started = cx.leitstand().started_at();
    let written: Vec<u64> = cx
        .leitstand()
        .order_log(root, id)
        .into_iter()
        .filter(|entry| {
            matches!(
                &entry.event,
                Event::Condition {
                    condition: Condition::Paused(PauseReason::AppEnded)
                }
            )
        })
        .map(|entry| entry.at.wall)
        .collect();
    check(
        "quitting wrote \"paused (app ended)\" into the order's log before the app ended",
        written.iter().any(|at| *at < started),
        format!("written at {written:?}, this run started at {started}"),
    );
    let lock = cx.leitstand().wake_lock();
    check(
        "a paused order holds no wake lock after the restart",
        !lock.requested && !lock.activity,
        format!("{lock:?}"),
    );
    cx.leitstand().use_agents(None);
    finish(SCENARIO);
}
