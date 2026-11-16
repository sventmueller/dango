//! The `leitstand-metrics` scenario (B47, E180, E170): a folder whose data folder already holds ten finished orders,
//! five built by Claude on `model-a` with one review round back each and five on `model-b` that passed at once. The
//! Leitstand tab's figures section shows both builders with their first-pass rates and says that the figures rest on
//! ten orders; a new order then goes to `model-b`, ranked second in the workflow file, and its evidence notes that the
//! first-pass rate broke the tie.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_factory::criteria::Criterion;
use ide_factory::event::{BaseRef, Event, OrderOptions};
use ide_factory::evidence::Deviation;
use ide_factory::log::{LOG_VERSION, LogEntry};
use ide_factory::options::NewOrder;
use ide_factory::routing::{Agent, AgentStatus, RankEntry, Vendor};
use ide_factory::{OrderId, Role, SessionKey, Stage, Stamp, Station};
use ide_session::{End, Script, ScriptTurn, Step};
use ide_ui_factory::model::words;
use objc2::rc::Retained;
use splat_i18n::{t, tf};

use super::super::git::{append, fixture_git};
use super::super::helpers::{self, wait_for_window, wait_within};
use super::super::{check, finish, give_up, give_up_saying, snapshot};
use crate::leitstand::Command;
use crate::util::main_soon;
use crate::window::WindowController;

const SCENARIO: &str = "leitstand-metrics";
const PROJECT: &str = "leitstand-metrics-project";
/// Two builders of one path that only their models tell apart, so the stage stays 1 and they are equally ready.
const WORKFLOW: &str = "[[gates.command]]\nname = \"check\"\nrun = \"true\"\n\n\
                        [[roles.builder]]\nagent = \"claude\"\nmodel = \"model-a\"\n\n\
                        [[roles.builder]]\nagent = \"claude\"\nmodel = \"model-b\"\n";
/// The orders each builder finished before.
const EACH: u32 = 5;
const STATIONS: Duration = Duration::from_secs(120);

pub(crate) fn run() {
    let cx = crate::context::app();
    cx.leitstand().use_script(Some(script()));
    cx.leitstand().use_agents(Some(BTreeMap::from([(
        Agent::Claude,
        AgentStatus::ready(&Agent::Claude),
    )])));
    let files = [("README.md", "# Metrics\n"), (".dango/factory.toml", WORKFLOW)];
    let root = match helpers::make_project(PROJECT, &files) {
        Ok(root) => root.canonicalize().unwrap_or(root),
        Err(error) => return give_up_saying(SCENARIO, "the scenario's folder is made", error.to_string()),
    };
    if let Err(error) = cx.leitstand().seed_logs(&root, &history()) {
        return give_up_saying(SCENARIO, "the earlier orders' logs are written", error.to_string());
    }
    std::thread::spawn(move || {
        let made = commit(&root);
        main_soon(move || match made {
            Ok(()) => opened(root),
            Err(error) => give_up_saying(SCENARIO, "the test repository has its first commit", error),
        });
    });
}

/// The builder's one turn: it confirms the criteria and writes a file.
fn script() -> Script {
    Script::new(vec![ScriptTurn::new(
        vec![
            Step::Say("I confirm the criteria.".into()),
            Step::Write {
                path: "notes.txt".into(),
                contents: "notes\n".into(),
            },
        ],
        End::Succeed,
    )])
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

/// The ten earlier orders' logs: `EACH` on `model-a` back from the review once, `EACH` on `model-b` through at once.
fn history() -> Vec<(OrderId, String)> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    let start = now.saturating_sub(3_600_000);
    (0..EACH)
        .flat_map(|n| [("model-a", 0, true, n), ("model-b", 1, false, n)])
        .enumerate()
        .map(|(i, (model, index, rework, n))| {
            let at = start + u64::try_from(i).unwrap_or(0) * 60_000;
            seeded(&format!("seed-{model}-{n}"), model, index, rework, at)
        })
        .collect()
}

/// The log of one finished order built by Claude on `model`, the entry at `index` of the builders' list.
fn seeded(id: &str, model: &str, index: usize, rework: bool, at: u64) -> (OrderId, String) {
    let order = OrderId(id.to_string());
    let mut events = vec![
        Event::Created {
            title: format!("Earlier order {id}"),
            goal: "Earlier work.".into(),
            criteria: vec![Criterion::new("K1", "done")],
            options: OrderOptions::default(),
            base: BaseRef {
                branch: "main".into(),
                commit: "0".repeat(40),
            },
            stage: Stage::One,
        },
        Event::Entered {
            station: Station::Build,
        },
        Event::SessionStarted {
            role: Role::Builder,
            entry: RankEntry::new(Agent::Claude).model(model),
            index,
            vendor: Vendor::Anthropic,
            session: SessionKey(format!("{id}-builder")),
            resume: None,
            quota_unknown: false,
            same_vendor: false,
            tie_break: None,
        },
        Event::SessionEnded { role: Role::Builder },
        Event::Entered {
            station: Station::Review,
        },
    ];
    if rework {
        events.push(Event::Findings {
            text: "unwrap on user input".into(),
            round: true,
        });
        events.push(Event::Entered {
            station: Station::Build,
        });
        events.push(Event::Entered {
            station: Station::Review,
        });
    }
    for station in [Station::Acceptance, Station::TakeOver, Station::Done] {
        events.push(Event::Entered { station });
    }
    let text = events
        .into_iter()
        .enumerate()
        .map(|(seq, event)| {
            let seq = u64::try_from(seq).unwrap_or(0);
            LogEntry {
                v: LOG_VERSION,
                order: order.clone(),
                seq,
                at: Stamp::new(seq, at + seq * 1_000),
                event,
            }
            .line()
        })
        .collect();
    (order, text)
}

/// Opens the repository trusted and waits for its factory to read the workflow file and the figures of its logs.
fn opened(root: PathBuf) {
    let root = helpers::open_trusted(&root);
    let probe = root.clone();
    wait_within(
        STATIONS,
        move || {
            let cx = crate::context::app();
            cx.leitstand().has_gates(&probe) && cx.leitstand().metrics(&probe).is_some_and(|m| m.orders() == 2 * EACH)
        },
        move |ready| {
            if !ready {
                let figures = crate::context::app().leitstand().metrics(&root).map(|m| m.orders());
                return give_up_saying(
                    SCENARIO,
                    "the folder's factory reads the workflow file and the figures of its ten logs",
                    format!("orders in the figures: {figures:?}"),
                );
            }
            wait_for_window(&root.clone(), move |controller| match controller {
                Some(controller) => show_tab(root, &controller),
                None => give_up(SCENARIO, "the folder opens in a window"),
            });
        },
    );
}

/// Opens the Leitstand tab and its figures section, and waits until the section shows both builders.
fn show_tab(root: PathBuf, controller: &Retained<WindowController>) {
    let state = controller.context().factory_ui();
    let shown = ide_ui_factory::show_leitstand(controller.handle(), state)
        && ide_ui_factory::selftest::open_metrics(controller.handle(), state);
    let probe = controller.clone();
    let controller = controller.clone();
    wait_within(
        STATIONS,
        move || {
            ide_ui_factory::selftest::board(probe.handle(), probe.context().factory_ui())
                .is_some_and(|seen| seen.metrics.rows == 2)
        },
        move |found| {
            if !shown || !found {
                let seen = ide_ui_factory::selftest::board(controller.handle(), controller.context().factory_ui());
                return give_up_saying(SCENARIO, "the Leitstand tab shows the figures", format!("{seen:?}"));
            }
            figures(&root, &controller);
        },
    );
}

/// The section's lines and rows: ten orders, all of them eligible, and the two builders' rates and rounds.
fn figures(root: &Path, controller: &Retained<WindowController>) {
    let Some(seen) = ide_ui_factory::selftest::board(controller.handle(), controller.context().factory_ui()) else {
        return give_up(SCENARIO, "the Leitstand tab is there");
    };
    let metrics = &seen.metrics;
    let (all, each) = ((2 * EACH).to_string(), EACH.to_string());
    let rests = tf(
        "Die Kennzahlen beruhen auf {} Aufträgen; {} davon zählen für die Erstdurchlaufquote, ohne Eingriffe und ohne Agentenwechsel.",
        &[&all, &all],
    );
    check(
        "the figures say they rest on ten orders, all of them counted for the first-pass rate",
        metrics.lines.first() == Some(&rests) && metrics.title == tf("Kennzahlen, {} Zeilen", &["2"]),
        format!("{:?}, {:?}", metrics.title, metrics.lines),
    );
    check(
        "the figures' table shows eight sortable columns",
        metrics.table_shown && metrics.columns.len() == 8,
        format!("{:?}", metrics.columns),
    );
    let row = |model: &str| {
        let name = format!("claude · {model}");
        metrics.cells.iter().find(|cells| cells.get(1) == Some(&name)).cloned()
    };
    let expect = |rate: &str, rounds: &str| {
        vec![
            words::role(Role::Builder).to_string(),
            words::risk(ide_factory::RiskClass::Low).to_string(),
            each.clone(),
            tf("{} % aus {}", &[rate, &each]),
            rounds.to_string(),
        ]
    };
    let picked = |cells: Option<Vec<String>>| {
        cells.map(|c| {
            [0, 2, 3, 4, 5]
                .iter()
                .filter_map(|&i| c.get(i).cloned())
                .collect::<Vec<_>>()
        })
    };
    check(
        "model-a passed none of its five orders at once and went back five times",
        picked(row("model-a")) == Some(expect("0", &each)),
        format!("{:?}", row("model-a")),
    );
    check(
        "model-b passed all of its five orders at once",
        picked(row("model-b")) == Some(expect("100", "0")),
        format!("{:?}", row("model-b")),
    );
    if let Some(window) = controller.window() {
        window.displayIfNeeded();
        snapshot(&window, "leitstand-metrics");
    }
    start(root.to_path_buf());
}

/// Starts an order and waits until its builder's session started.
fn start(root: PathBuf) {
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
    wait_within(
        STATIONS,
        move || {
            crate::context::app()
                .leitstand()
                .with_order(&probe.0, &probe.1, |o| o.session(Role::Builder).is_some())
                .unwrap_or(false)
        },
        move |started| {
            if !started {
                return give_up(SCENARIO, "the new order's builder starts");
            }
            routed(&root, &id);
        },
    );
}

/// The builder of the new order and the note the tie-break left in its evidence.
fn routed(root: &Path, id: &OrderId) {
    let cx = crate::context::app();
    let seen = cx.leitstand().with_order(root, id, |o| {
        let model = o.session(Role::Builder).and_then(|s| s.entry().model.clone());
        let note = o
            .deviations()
            .iter()
            .find(|d| matches!(d, Deviation::TieBreak { .. }))
            .cloned();
        (model, note)
    });
    let Some((model, note)) = seen else {
        return give_up(SCENARIO, "the new order is there");
    };
    check(
        "the tie-break gives the order to model-b, ranked second, by its better first-pass rate",
        model.as_deref() == Some("model-b"),
        format!("{model:?}"),
    );
    let words = note.as_ref().map(words::deviation).unwrap_or_default();
    let expected = tf(
        "Gleichstand als {} nach Erstdurchlaufquote entschieden: {} ({} %) vor {} ({} %)",
        &[t("Builder"), "claude · model-b", "100", "claude · model-a", "0"],
    );
    check(
        "the evidence notes that the first-pass rate decided, with both rates",
        words == expected,
        format!("{words:?}"),
    );
    cx.leitstand().command(&cx, root, Command::Abort(id.clone()));
    cx.leitstand().use_script(None);
    cx.leitstand().use_agents(None);
    finish(SCENARIO);
}
