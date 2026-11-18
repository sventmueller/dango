//! The `leitstand-order` scenario (B42, F61, R12): one work order runs through every station on a git repository of
//! its own with the scripted session in place of an agent and a gate command that passes (`true`). Only Claude counts
//! as ready, so the order runs in stage 1: the specification is skipped, the builder writes a file in the order's
//! worktree, the factory commits it, the path scope, the secrets scan and the gate pass, the reviewer agent is skipped,
//! and the evidence waits for acceptance. The order comes in through "Ablage ▸ Neuer Auftrag …" and the sheet. At
//! acceptance the order's tab offers the report the path scope names and a terminal in the worktree beside its next
//! step, and both open; G2 from the tab takes it over into the base branch of the working copy, the worktree goes and
//! the report is kept in the project folder. The done order stays in the panel until "Auftrag löschen …" deletes it,
//! which removes its log and its kept report. The order holds the wake lock while it runs and lets it go while it
//! waits; no notification reaches the user and no activity keeps the Mac awake.

mod acceptance;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::{Condition, OrderId, Station, WaitReason};
use ide_git::CommitOptions;
use ide_session::{End, Script, ScriptTurn, Step};
use ide_ui_factory::model::detail::Command as OrderCommand;
use ide_ui_factory::selftest::{SheetControl, SheetTyped};
use objc2::sel;
use objc2_app_kit::NSAlertFirstButtonReturn;

use super::super::git::{append, fixture_git};
use super::super::helpers::{self, wait_for_within, wait_within};
use super::super::{check, finish, give_up, give_up_saying, snapshot};
use super::drive;
use crate::leitstand::Command;
use crate::util::main_soon;
use crate::window::WindowController;

const SCENARIO: &str = "leitstand-order";
const PROJECT: &str = "leitstand-order-project";
/// The order's title, which the sheet takes and its alerts name.
const TITLE: &str = "Gruß schreiben";
/// The file the scripted builder writes.
const WRITTEN: &str = "hello.txt";
const CONTENT: &str = "hello\n";
/// The Markdown file the scripted builder writes, which the path scope names.
const REPORT: &str = "REPORT.md";
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
            Step::Write {
                path: REPORT.into(),
                contents: "# Report\n".into(),
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
            new_order(root);
        },
    );
}

/// Opens "Ablage ▸ Neuer Auftrag …" as the user does and waits for the sheet.
fn new_order(root: PathBuf) {
    let Some(controller) = helpers::window_of(&root) else {
        return give_up(SCENARIO, "the folder has a window");
    };
    let Some(item) = drive::menu_item(sel!(ideFactoryMenuNewOrder:), None) else {
        return give_up(SCENARIO, "the File menu has \"Neuer Auftrag …\"");
    };
    check(
        "\"Neuer Auftrag …\" in the File menu is enabled for a folder with a Leitstand",
        drive::enabled(&item),
        item.title().to_string(),
    );
    drive::choose(&item);
    let probe = controller.clone();
    wait_within(
        SHORT,
        move || ide_ui_factory::selftest::order_sheet(probe.handle()).is_some(),
        move |open| {
            if !open {
                return give_up(SCENARIO, "\"Neuer Auftrag …\" opens the sheet");
            }
            start(root, &controller);
        },
    );
}

/// The only order of the folder, once the factory took it in.
fn the_order(root: &Path) -> Option<OrderId> {
    let panel = crate::context::app().leitstand().panel(root)?;
    let mut ids = (panel.groups().iter()).flat_map(|(_, rows)| rows.iter().map(|row| row.id().clone()));
    ids.next()
}

/// Types the order into the sheet and presses "Starten"; the sheet closes and the order comes in.
fn start(root: PathBuf, controller: &WindowController) {
    let notices = ide_ui_common::notify::posted();
    let window = controller.handle();
    let typed = SheetTyped {
        title: TITLE.into(),
        goal: "Write a greeting into hello.txt.".into(),
        criteria: "- hello.txt says hello\n".into(),
        scope: format!("{WRITTEN}, {REPORT}"),
        budget: String::new(),
    };
    let filled = ide_ui_factory::selftest::type_into_order_sheet(window, &typed);
    let seen = ide_ui_factory::selftest::order_sheet(window);
    check(
        "the sheet takes the typed order and can start it",
        filled && seen.as_ref().is_some_and(|s| s.start && s.title == TITLE),
        format!("{seen:?}"),
    );
    if !ide_ui_factory::selftest::press_order_sheet(window, SheetControl::Start) {
        return give_up(SCENARIO, "the sheet has \"Starten\"");
    }
    let probe = root.clone();
    wait_for_within(
        SHORT,
        move || the_order(&probe),
        move |id| {
            let Some(id) = id else {
                return give_up(SCENARIO, "\"Starten\" brings the order in");
            };
            let closed = helpers::window_of(&root)
                .and_then(|c| c.window())
                .is_some_and(|w| w.attachedSheet().is_none());
            check("\"Starten\" closes the sheet", closed, "");
            run_to_acceptance(root, id, notices);
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

/// Waits until the order's evidence waits for acceptance.
fn run_to_acceptance(root: PathBuf, id: OrderId, notices: usize) {
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
    if seen.evidence.is_none() {
        return give_up(SCENARIO, "the order has evidence to accept");
    }
    acceptance_tab(root.to_path_buf(), id.clone());
}

/// The order's tab at acceptance offers "Bericht öffnen" and "Im Terminal öffnen" in its bar; the panel's row, the
/// menu "Ablage ▸ Auftrag" and "Zurück an den Builder …" act on the order (`acceptance`), the report opens in an
/// editor from the menu and the terminal in the worktree from the tab.
fn acceptance_tab(root: PathBuf, id: OrderId) {
    let Some(controller) = helpers::window_of(&root) else {
        return give_up(SCENARIO, "the folder has a window");
    };
    if !ide_ui_factory::open_order(controller.handle(), controller.context().factory_ui(), &id) {
        return give_up(SCENARIO, "the order's tab opens");
    }
    let bar = |controller: &crate::window::WindowController, id: &OrderId| {
        ide_ui_factory::selftest::order(controller.handle(), controller.context().factory_ui(), id)
            .map(|seen| seen.bar)
            .unwrap_or_default()
    };
    let probe = (controller.clone(), id.clone());
    let offers = |bar: &[String]| {
        [OrderCommand::OpenReport, OrderCommand::OpenTerminal]
            .iter()
            .all(|command| bar.iter().any(|title| title == command.title()))
    };
    wait_within(
        SHORT,
        move || offers(&bar(&probe.0, &probe.1)),
        move |offered| {
            check(
                "the order's tab offers the report and a terminal in the worktree beside its next step at acceptance",
                offered,
                format!("{:?}", bar(&controller, &id)),
            );
            if let Some(window) = controller.window() {
                window.displayIfNeeded();
                snapshot(&window, "leitstand-order-acceptance");
            }
            let Some(tree) = crate::context::app().leitstand().worktree(&root, &id) else {
                return give_up(SCENARIO, "the order has a worktree at acceptance");
            };
            acceptance::row(&controller);
            acceptance::menus(&controller, &id, &tree);
            open_terminal(&tree, &id, &controller);
            acceptance::back_to_builder(root, id, accept);
        },
    );
}

fn open_terminal(tree: &Path, id: &OrderId, controller: &crate::window::WindowController) {
    let (window, ui) = (controller.handle(), controller.context().factory_ui());
    let pressed = ide_ui_factory::selftest::press_order(window, ui, id, OrderCommand::OpenTerminal);
    let opened = crate::terminal::inspect::latest().map(|(_, view)| view.cwd());
    let same = |path: &Path| path.canonicalize().ok();
    check(
        "\"Im Terminal öffnen\" opens a terminal in the order's worktree",
        pressed
            && opened
                .as_deref()
                .and_then(same)
                .is_some_and(|cwd| Some(cwd) == same(tree)),
        format!("pressed: {pressed}, {opened:?} for {}", tree.display()),
    );
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

/// G2 from the order's tab: accepts the evidence whole and waits until the order is taken over and its worktree is
/// gone.
fn accept(root: PathBuf, id: OrderId) {
    let pressed = helpers::window_of(&root).is_some_and(|controller| {
        let (window, ui) = (controller.handle(), controller.context().factory_ui());
        ide_ui_factory::selftest::press_order(window, ui, &id, OrderCommand::Accept)
    });
    if !pressed {
        return give_up(SCENARIO, "the order's tab gives G2");
    }
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
    // The factory lets the lock go with the effect of the step after the take-over.
    let let_go = || {
        let lock = crate::context::app().leitstand().wake_lock();
        !lock.requested && !lock.activity
    };
    let (root, id) = (root.to_path_buf(), id.clone());
    wait_within(Duration::from_secs(5), let_go, move |released| {
        check(
            "the wake lock stays let go once the order is done",
            released,
            format!("{:?}", crate::context::app().leitstand().wake_lock()),
        );
        kept_and_listed(&root, &id);
        delete(root, id);
    });
}

/// The done order keeps its report in the project folder, which git ignores, and stays in the panel's group "Erledigt".
fn kept_and_listed(root: &Path, id: &OrderId) {
    let cx = crate::context::app();
    let folder = ide_factory::reports::folder(root, id);
    let ignore = std::fs::read_to_string(ide_factory::reports::root_folder(root).join(".gitignore")).ok();
    check(
        "the take-over keeps the order's report in the project folder in a folder git ignores",
        folder.join(REPORT).is_file() && ignore.as_deref() == Some("*\n"),
        format!(
            "{}: {}, .gitignore {ignore:?}",
            folder.display(),
            folder.join(REPORT).is_file()
        ),
    );
    let done: Vec<OrderId> = cx
        .leitstand()
        .panel(root)
        .map(|panel| {
            panel
                .groups()
                .iter()
                .filter(|(station, _)| *station == Station::Done)
                .flat_map(|(_, rows)| rows.iter().map(|row| row.id().clone()))
                .collect()
        })
        .unwrap_or_default();
    check(
        "the done order stays in the panel's group of done orders",
        done == [id.clone()],
        format!("{done:?}"),
    );
}

/// "Auftrag löschen …" in the done order's tab asks first, naming the order; its first button deletes it.
fn delete(root: PathBuf, id: OrderId) {
    let pressed = helpers::window_of(&root).is_some_and(|controller| {
        let (window, ui) = (controller.handle(), controller.context().factory_ui());
        ide_ui_factory::selftest::press_order(window, ui, &id, OrderCommand::Delete)
    });
    if !pressed {
        return give_up(SCENARIO, "the done order's tab is open");
    }
    drive::when_alert(root, move |root, shown| {
        let texts = drive::alert_texts(&root);
        check(
            "\"Auftrag löschen …\" asks first, naming the order",
            shown && texts.iter().any(|text| text.contains(TITLE)),
            format!("{texts:?}"),
        );
        drive::answer(root, NSAlertFirstButtonReturn, move |root, answered| {
            if !answered {
                return give_up(SCENARIO, "the alert \"Auftrag löschen …\" takes its answer");
            }
            deleted(root, id);
        });
    });
}

/// Deleting the done order takes it out of the factory and removes its log and its kept report.
fn deleted(root: PathBuf, id: OrderId) {
    let log = ide_core::paths::data_dir()
        .join("leitstand")
        .join(ide_core::paths::project_key(&root))
        .join("orders")
        .join(format!("{}.jsonl", id.0));
    let folder = ide_factory::reports::folder(&root, &id);
    let probe = (root.clone(), id.clone(), log.clone(), folder.clone());
    wait_within(
        SHORT,
        move || {
            crate::context::app()
                .leitstand()
                .with_order(&probe.0, &probe.1, |_| ())
                .is_none()
                && !probe.2.exists()
                && !probe.3.exists()
        },
        move |gone| {
            check(
                "deleting the done order removes it, its log and its kept report",
                gone,
                format!(
                    "log {}: {}, report {}: {}",
                    log.display(),
                    log.exists(),
                    folder.display(),
                    folder.exists()
                ),
            );
            let cx = crate::context::app();
            cx.leitstand().use_script(None);
            cx.leitstand().use_agents(None);
            finish(SCENARIO);
        },
    );
}
