//! The `leitstand-gaps` scenario (E183, E178): the dialog "Software Factory für <Ordner>" names the origin of each value, a
//! role list and a risk pattern from `.dango/factory.toml`, the rest "Vorgabe"; "Auf Vorgabe zurücksetzen" takes the
//! risk pattern out, and saving keeps the file's comment and its other values. Then a scripted builder objects to the
//! criteria in its first turn: the order waits, its tab shows the objection's text with "Weiter" and "Kriterien ändern
//! …" ready, and "Weiter" overrules it, so the builder's next turn carries the order on to acceptance.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_factory::options::NewOrder;
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::{Condition, OrderId, Role, WaitReason};
use ide_session::{End, Script, ScriptTurn, Step};
use ide_ui_factory::model::detail::Command as OrderCommand;
use ide_ui_factory::model::settings::{Origin, ProjectValue, role_title};
use ide_ui_factory::selftest::ProjectRowSeen;
use objc2::rc::Retained;

use super::super::git::{append, fixture_git};
use super::super::helpers::{self, wait_for_window, wait_within};
use super::super::{check, finish, give_up, give_up_saying, snapshot};
use crate::leitstand::Command;
use crate::util::main_soon;
use crate::window::WindowController;

const SCENARIO: &str = "leitstand-gaps";
const PROJECT: &str = "leitstand-gaps-project";
const COMMENT: &str = "# Rules of the self-test.";
const WORKFLOW: &str = "# Rules of the self-test.\n[[gates.command]]\nname = \"check\"\nrun = \"true\"\n\n\
                        [[roles.reviewer]]\nagent = \"claude\"\n\n[risk]\nhigh = [\"src/pay/**\"]\n";
const REASON: &str = "- [K1] needs a sample file to be checked";
const STATIONS: Duration = Duration::from_secs(120);
const SHORT: Duration = Duration::from_secs(20);

pub(crate) fn run() {
    let cx = crate::context::app();
    cx.leitstand().use_script(Some(script()));
    cx.leitstand().use_agents(Some(BTreeMap::from([(
        Agent::Claude,
        AgentStatus::ready(&Agent::Claude),
    )])));
    let files = [("README.md", "# Gaps\n"), (".dango/factory.toml", WORKFLOW)];
    let root = match helpers::make_project(PROJECT, &files) {
        Ok(root) => root.canonicalize().unwrap_or(root),
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

/// The builder objects in its first turn; after "Weiter" it writes the file.
fn script() -> Script {
    Script::new(vec![
        ScriptTurn::new(
            vec![Step::Say(format!(
                "I will not start yet.\n\nCRITERIA OBJECTION\n{REASON}"
            ))],
            End::Succeed,
        ),
        ScriptTurn::new(
            vec![
                Step::Write {
                    path: "notes.txt".into(),
                    contents: "notes\n".into(),
                },
                Step::Say("Done.".into()),
            ],
            End::Succeed,
        ),
    ])
}

fn commit(root: &Path) -> Result<(), String> {
    let git = fixture_git()?;
    let repo = git.init(root).map_err(|e| e.to_string())?;
    append(
        &repo.git_dir().join("config"),
        "[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n[commit]\n\tgpgsign = false\n",
    )?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", ide_git::CommitOptions::default())
        .map_err(|e| e.to_string())
        .map(|_| ())
}

/// Opens the repository trusted, confirms its gate command and waits for its window.
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
            let cx = crate::context::app();
            let Some((hash, _)) = cx.leitstand().gates(&root) else {
                return give_up(SCENARIO, "the folder has a factory");
            };
            cx.leitstand().command(&cx, &root, Command::ConfirmGates(hash));
            wait_for_window(&root.clone(), move |controller| match controller {
                Some(controller) => dialog(root, &controller),
                None => give_up(SCENARIO, "the folder opens in a window"),
            });
        },
    );
}

/// The row of `value` in what the dialog shows.
fn row(rows: &[ProjectRowSeen], value: ProjectValue) -> Option<&ProjectRowSeen> {
    let title = value.title();
    rows.iter().find(|row| row.title == title)
}

/// Opens "Software Factory für <Ordner>" and waits until it shows its rows.
fn dialog(root: PathBuf, controller: &Retained<WindowController>) {
    ide_ui_factory::project_settings(controller.handle(), controller.context().factory_ui());
    let probe = controller.clone();
    let controller = controller.clone();
    wait_within(
        SHORT,
        move || ide_ui_factory::selftest::project(probe.handle(), probe.context().factory_ui()).is_some(),
        move |open| {
            if !open {
                return give_up(SCENARIO, "the dialog Software Factory für <Ordner> opens");
            }
            origins(root, &controller);
        },
    );
}

/// Each value's origin, then the reset of the risk pattern.
fn origins(root: PathBuf, controller: &Retained<WindowController>) {
    let (window, state) = (controller.handle(), controller.context().factory_ui());
    let Some(rows) = ide_ui_factory::selftest::project(window, state) else {
        return give_up(SCENARIO, "the dialog is open");
    };
    let (file, default) = (Origin::File.words(), Origin::Default.words());
    let reviewer = row(&rows, ProjectValue::Role(Role::Reviewer));
    check(
        "the reviewer's list comes from factory.toml and can be reset",
        reviewer.is_some_and(|r| r.origin == file && r.resettable && r.title == role_title(Role::Reviewer)),
        format!("{reviewer:?}"),
    );
    let planner = row(&rows, ProjectValue::Role(Role::Planner));
    let rounds = row(&rows, ProjectValue::Limit(ide_factory::limits::Limit::ReviewRounds));
    check(
        "values the file does not set show the default and offer no reset",
        [planner, rounds, row(&rows, ProjectValue::Tests)]
            .iter()
            .all(|r| r.is_some_and(|r| r.origin == default && !r.resettable)),
        format!("{planner:?}, {rounds:?}"),
    );
    let high = row(&rows, ProjectValue::RiskHigh).cloned();
    check(
        "the risk pattern shows in an editable row with its origin",
        high.as_ref()
            .is_some_and(|r| r.value == "src/pay/**" && r.origin == file && r.resettable),
        format!("{high:?}"),
    );
    if let Some(sheet) = controller.window().and_then(|w| w.attachedSheet()) {
        sheet.displayIfNeeded();
        snapshot(&sheet, "leitstand-gaps-dialog");
    }
    let pressed = ide_ui_factory::selftest::reset_project_value(window, state, ProjectValue::RiskHigh);
    let after =
        ide_ui_factory::selftest::project(window, state).and_then(|rows| row(&rows, ProjectValue::RiskHigh).cloned());
    check(
        "\"Auf Vorgabe zurücksetzen\" takes the risk pattern back to the default",
        pressed
            && after
                .as_ref()
                .is_some_and(|r| r.value.is_empty() && r.origin == default && !r.resettable),
        format!("{after:?}"),
    );
    if !ide_ui_factory::selftest::save_project(window, state) {
        return give_up(SCENARIO, "the dialog saves");
    }
    let path = root.join(".dango/factory.toml");
    let probe = path.clone();
    let controller = controller.clone();
    wait_within(
        SHORT,
        move || std::fs::read_to_string(&probe).is_ok_and(|text| !text.contains("src/pay/**")),
        move |saved| {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            check(
                "saving writes the file without the reset key and keeps its comment and its other values",
                saved
                    && text.contains(COMMENT)
                    && text.contains("[[roles.reviewer]]")
                    && text.contains("run = \"true\""),
                text.clone(),
            );
            start(root, &controller);
        },
    );
}

/// Starts an order whose builder objects to its criteria.
fn start(root: PathBuf, controller: &Retained<WindowController>) {
    let cx = crate::context::app();
    let order = NewOrder {
        title: "Notizen schreiben".into(),
        goal: "Write notes into notes.txt.".into(),
        criteria: "- notes.txt holds notes\n".into(),
        ..NewOrder::default()
    };
    let Some(id) = cx.leitstand().command(&cx, &root, Command::New(order)) else {
        return give_up(SCENARIO, "the order comes in");
    };
    let probe = (root.clone(), id.clone());
    let controller = controller.clone();
    wait_within(
        STATIONS,
        move || {
            crate::context::app()
                .leitstand()
                .with_order(&probe.0, &probe.1, |o| {
                    matches!(o.condition(), Condition::Waiting(WaitReason::CriteriaObjection { .. }))
                })
                .unwrap_or(false)
        },
        move |waits| {
            if !waits {
                return give_up_saying(
                    SCENARIO,
                    "the order waits on the builder's objection",
                    state(&root, &id),
                );
            }
            objection_tab(root, id, &controller);
        },
    );
}

fn state(root: &Path, id: &OrderId) -> String {
    crate::context::app()
        .leitstand()
        .with_order(root, id, |o| {
            format!("{:?} {:?} {:?}", o.station(), o.condition(), o.phase())
        })
        .unwrap_or_else(|| "no such order".into())
}

/// Opens the order's tab and waits until it shows the objection.
fn objection_tab(root: PathBuf, id: OrderId, controller: &Retained<WindowController>) {
    let (window, ui) = (controller.handle(), controller.context().factory_ui());
    if !ide_ui_factory::open_order(window, ui, &id) {
        return give_up(SCENARIO, "the order's tab opens");
    }
    let probe = (controller.clone(), id.clone());
    let controller = controller.clone();
    wait_within(
        SHORT,
        move || {
            ide_ui_factory::selftest::order(probe.0.handle(), probe.0.context().factory_ui(), &probe.1)
                .is_some_and(|seen| seen.texts.iter().any(|text| text == REASON))
        },
        move |shown| {
            let seen = ide_ui_factory::selftest::order(controller.handle(), controller.context().factory_ui(), &id);
            check(
                "the order's tab shows the builder's objection in its words",
                shown,
                format!("{seen:?}"),
            );
            let ready = |command: OrderCommand| {
                seen.as_ref()
                    .is_some_and(|s| s.buttons.iter().any(|(title, on)| title == command.title() && *on))
            };
            check(
                "\"Weiter\", \"Kriterien ändern …\" and \"Auftrag abbrechen …\" are ready while the objection waits",
                ready(OrderCommand::Continue) && ready(OrderCommand::ChangeCriteria) && ready(OrderCommand::Abort),
                format!("{:?}", seen.map(|s| s.buttons)),
            );
            if let Some(window) = controller.window() {
                window.displayIfNeeded();
                snapshot(&window, "leitstand-gaps-objection");
            }
            overrule(root, id);
        },
    );
}

/// "Weiter" overrules the objection; the builder's next turn carries the order on to acceptance.
fn overrule(root: PathBuf, id: OrderId) {
    let cx = crate::context::app();
    cx.leitstand().command(&cx, &root, Command::Continue(id.clone()));
    let probe = (root.clone(), id.clone());
    wait_within(
        STATIONS,
        move || {
            crate::context::app()
                .leitstand()
                .with_order(&probe.0, &probe.1, |o| {
                    *o.condition() == Condition::Waiting(WaitReason::Acceptance)
                })
                .unwrap_or(false)
        },
        move |accepted| {
            check(
                "after \"Weiter\" the builder carries the criteria as they are on to acceptance",
                accepted
                    && crate::context::app()
                        .leitstand()
                        .with_order(&root, &id, |o| !o.contract_due())
                        .unwrap_or(false),
                state(&root, &id),
            );
            let cx = crate::context::app();
            cx.leitstand().command(&cx, &root, Command::Abort(id));
            cx.leitstand().use_script(None);
            cx.leitstand().use_agents(None);
            finish(SCENARIO);
        },
    );
}
