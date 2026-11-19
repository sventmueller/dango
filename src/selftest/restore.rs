//! The `restore` scenario, run twice in one home: the first launch sets up two folder windows, one with documents
//! of every kind that comes back, a frame of its own, a wider sidebar and the Git panel, the other with its sidebar
//! folded away, and quits as the user does; the second checks that both come back that way, and that a folder opened
//! for the first time takes the frame of the window saved last.

use std::path::{Path, PathBuf};

use objc2::MainThreadMarker;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use serde::{Deserialize, Serialize};

use super::helpers::{self, open_beside, wait_until, window_of};
use super::{check, finish, give_up};
use crate::window::Panel;

const SCENARIO: &str = "restore";
/// The session of the AI tab, which comes back by its id.
const SESSION: &str = "51e1f7e5-0000-4000-8000-0000000000cc";
const FRAME: NSRect = NSRect::new(NSPoint::new(120.0, 140.0), NSSize::new(1100.0, 720.0));
const SIDEBAR: f64 = 380.0;
/// The model, effort and mode picked in the AI thread before the quit.
const PICKS: (&str, &str, &str) = ("opus", "high", "acceptEdits");

/// What the first launch left, for the second to compare.
#[derive(Serialize, Deserialize)]
struct Before {
    first: Vec<Vec<String>>,
    second: Vec<Vec<String>>,
    frame: [f64; 4],
    second_frame: [f64; 4],
    sidebar: f64,
}

fn folders() -> (PathBuf, PathBuf, PathBuf) {
    let made = |name: &str| helpers::make_project(name, &[]).unwrap_or_else(|_| helpers::root(name));
    (made("restore-a"), made("restore-b"), made("restore-c"))
}

fn record() -> PathBuf {
    ide_core::paths::home().join("restore-before.json")
}

fn rect(frame: NSRect) -> [f64; 4] {
    [frame.origin.x, frame.origin.y, frame.size.width, frame.size.height]
}

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let (a, b, c) = folders();
    if std::env::var("IDE_SELFTEST_LAUNCH").as_deref() == Ok("quit") {
        let _ = std::fs::write(a.join("main.rs"), "fn main() {}\n");
        let _ = std::fs::write(a.join("notes.md"), "# Notizen\n");
        let _ = std::fs::write(b.join("lib.rs"), "pub fn lib() {}\n");
        for folder in [&a, &b, &c] {
            crate::project::trust(folder);
        }
        crate::app::open_project(mtm, a.clone());
        open_beside(mtm, &b);
        let probe = (a.clone(), b.clone());
        wait_until(
            move || window_of(&probe.0).is_some() && window_of(&probe.1).is_some(),
            move |_| arrange(a, b),
        );
    } else {
        let saved = saved_frame(&a);
        let probe = (a.clone(), b.clone());
        let (probe_a, probe_b) = probe.clone();
        wait_until(
            move || window_of(&probe.0).is_some() && window_of(&probe.1).is_some(),
            move |_| {
                wait_until(
                    move || came_back_whole(&probe_a, &probe_b),
                    move |_| came_back(a, b, c, saved),
                )
            },
        );
    }
}

fn arrange(a: PathBuf, b: PathBuf) {
    let (Some(first), Some(second)) = (window_of(&a), window_of(&b)) else {
        return give_up(SCENARIO, "both folders get their windows");
    };
    first.open_file(&a.join("main.rs"));
    first.open_file(&a.join("notes.md"));
    crate::terminal::open_tab(&first, Some(&a));
    let replayed = crate::selftest::replay_agent(&first, SESSION);
    if let Some(tab) = ide_ui_agent::of_window(&first).first() {
        tab.pick_all(PICKS.0, PICKS.1, PICKS.2);
    }
    first.activate_panel(Panel::Git);
    first.set_sidebar_width(SIDEBAR);
    if let Some(window) = first.window() {
        window.setFrame_display(FRAME, true);
    }
    second.open_file(&b.join("lib.rs"));
    second.toggle_sidebar();
    check("the AI thread plays its recording before the quit", replayed, "");
    let probe = (a.clone(), b.clone());
    wait_until(
        move || {
            let (Some(first), Some(second)) = (window_of(&probe.0), window_of(&probe.1)) else {
                return false;
            };
            first.window_tabs().len() >= 4
                && (first.inspect().sidebar_width - SIDEBAR).abs() < 2.0
                && second.sidebar_folded()
        },
        move |_| quit(&a, &b),
    );
}

fn quit(a: &Path, b: &Path) {
    let (Some(first), Some(second)) = (window_of(a), window_of(b)) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    for controller in [&first, &second] {
        if let Some(content) = controller.window().and_then(|w| w.contentView()) {
            content.layoutSubtreeIfNeeded();
        }
    }
    let before = Before {
        first: first.window_tabs(),
        second: second.window_tabs(),
        frame: first.window().map_or([0.0; 4], |w| rect(w.frame())),
        second_frame: second.window().map_or([0.0; 4], |w| rect(w.frame())),
        sidebar: first.inspect().sidebar_width,
    };
    check(
        "the first window has its documents, frame and sidebar before the quit, the second its sidebar folded",
        before.first.len() >= 4 && (before.sidebar - SIDEBAR).abs() < 2.0 && second.sidebar_folded(),
        format!(
            "{:?}, sidebar {}, folded {}",
            before.first,
            before.sidebar,
            second.sidebar_folded()
        ),
    );
    if let Ok(json) = serde_json::to_string(&before) {
        let _ = std::fs::write(record(), json);
    }
    let folders = [a.to_path_buf(), b.to_path_buf()];
    wait_until(
        move || recorded_both(&folders),
        move |recorded| {
            check(
                "the record of open windows holds both folders, the first with all its documents",
                recorded,
                format!("{:?}", crate::session::recorded()),
            );
            finish(SCENARIO);
        },
    );
}

/// Whether the record of open windows holds a window of each of `folders`, the first with both its documents; only
/// the welcome tab and editors are recorded, the rest comes back through its own state.
fn recorded_both(folders: &[PathBuf; 2]) -> bool {
    let records = crate::session::recorded();
    let canonical = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let of = |folder: &Path| {
        let folder = canonical(folder);
        records
            .iter()
            .find(|r| r.project.as_deref().is_some_and(|p| canonical(p) == folder))
    };
    let documents = ["main.rs", "notes.md"].map(|name| canonical(&folders[0].join(name)));
    let holds_documents = of(&folders[0]).is_some_and(|r| {
        documents.iter().all(|document| {
            r.tabs.iter().any(|tab| {
                matches!(tab, crate::session::TabRecord::Editor { path: Some(path), .. } if canonical(path) == *document)
            })
        })
    });
    holds_documents && of(&folders[1]).is_some()
}

/// Whether both windows came back with their tabs and the second its sidebar folded, as the record says.
fn came_back_whole(a: &Path, b: &Path) -> bool {
    let before: Option<Before> = std::fs::read(record())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    match (before, window_of(a), window_of(b)) {
        (Some(before), Some(first), Some(second)) => {
            first.window_tabs() == before.first && second.window_tabs() == before.second && second.sidebar_folded()
        }
        _ => false,
    }
}

/// The frame the quit saved for `folder`, read before this launch saves again.
fn saved_frame(folder: &Path) -> Option<[f64; 4]> {
    let key = crate::util::ns(&format!("NSWindow Frame ProjectWindow {}", folder.display()));
    let saved = objc2_foundation::NSUserDefaults::standardUserDefaults().stringForKey(&key)?;
    let numbers: Vec<f64> = saved
        .to_string()
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    Some([*numbers.first()?, *numbers.get(1)?, *numbers.get(2)?, *numbers.get(3)?])
}

fn came_back(a: PathBuf, b: PathBuf, c: PathBuf, saved: Option<[f64; 4]>) {
    let before: Option<Before> = std::fs::read(record())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let (Some(before), Some(first)) = (before, window_of(&a)) else {
        return give_up(SCENARIO, "both folder windows come back");
    };
    // A file tab behind others loads its file, and with it its preview, when it comes to the front.
    let notes = a.join("notes.md");
    let canonical = notes.canonicalize().unwrap_or_else(|_| notes.clone());
    for key in [notes, canonical] {
        if first.select_tab(&key.to_string_lossy()) {
            break;
        }
    }
    let wanted = (before.first.clone(), before.second.clone());
    let (at, bt) = (a.clone(), b.clone());
    let tabs = move || {
        window_of(&at)
            .zip(window_of(&bt))
            .map(|(f, s)| (f.window_tabs(), s.window_tabs()))
    };
    let shown = tabs.clone();
    wait_until(
        move || tabs().is_some_and(|tabs| tabs == wanted),
        move |_| {
            check(
                "each folder window comes back with its tabs in their order, the Markdown file with its preview",
                shown().is_some_and(|tabs| tabs == (before.first.clone(), before.second.clone())),
                format!("{:?}", shown()),
            );
            whole(a, b, c, saved, &before);
        },
    );
}

/// The rest of the second launch's checks, once the tabs are back.
fn whole(a: PathBuf, b: PathBuf, c: PathBuf, saved: Option<[f64; 4]>, before: &Before) {
    let (Some(first), Some(second)) = (window_of(&a), window_of(&b)) else {
        return give_up(SCENARIO, "both folder windows come back");
    };
    ai_sessions(&first, &a);
    let frame = first.window().map(|w| rect(w.frame()));
    let same = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.5);
    check(
        "the window comes back exactly where the quit left it, as large as the user made it",
        frame.zip(saved).is_some_and(|(frame, saved)| same(frame, saved))
            && frame.is_some_and(|frame| frame.iter().zip(before.frame).all(|(now, was)| (now - was).abs() < 4.0)),
        format!("{frame:?}, saved {saved:?}, set {:?}", before.frame),
    );
    let seen = first.inspect();
    check(
        "the sidebar comes back as wide, with the Git panel",
        (seen.sidebar_width - before.sidebar).abs() < 2.0 && seen.panel == Some(Panel::Git),
        format!("{} pt, {:?}", seen.sidebar_width, seen.panel),
    );
    check(
        "a sidebar folded away stays folded",
        second.sidebar_folded() && !first.sidebar_folded(),
        format!("{} / {}", second.sidebar_folded(), first.sidebar_folded()),
    );
    newcomer(&c, before);
    finish(SCENARIO);
}

/// The AI thread that came back: its session, the panel's one row for it, the agent of another agent's session, the
/// picks and the composer.
fn ai_sessions(first: &crate::window::WindowController, a: &Path) {
    let sessions = ide_ui_agent::of_window(first)
        .iter()
        .filter_map(|tab| tab.session_id())
        .collect::<Vec<_>>();
    check(
        "the AI thread comes back as its session, played from a recording here",
        sessions.iter().any(|session| session == SESSION),
        format!("{sessions:?}"),
    );
    let twice = continued_once(first, a);
    check(
        "a session an AI tab continues shows once in the AI panel, not also as one saved from a terminal",
        twice
            .as_ref()
            .is_some_and(|rows| !rows.iter().any(|(_, _, title)| title == "doppelt")),
        format!("{twice:?}"),
    );
    let agent = other_agents_session(a);
    check(
        "a session another agent ran comes back with that agent, not with Claude",
        agent.as_deref() == Some("antigravity-acp"),
        format!("{agent:?}"),
    );
    let wanted = (
        Some(PICKS.0.to_string()),
        Some(PICKS.1.to_string()),
        PICKS.2.to_string(),
    );
    let restored = ide_ui_agent::of_window(first)
        .iter()
        .find(|tab| tab.session_id().as_deref() == Some(SESSION))
        .map(|tab| tab.picks());
    let fresh = ide_ui_agent::open_replay(first).map(|tab| tab.picks());
    check(
        "the AI thread that came back and a new one start with the model, effort and mode picked last",
        restored.as_ref() == Some(&wanted) && fresh.as_ref() == Some(&wanted),
        format!("{restored:?} / {fresh:?}"),
    );
    let hits = ide_ui_agent::of_window(first)
        .iter()
        .find(|tab| tab.session_id().as_deref() == Some(SESSION))
        .map(|tab| {
            tab.select();
            tab.composer_clicks()
        });
    check(
        "a click on the composer of the AI thread that came back, also on its placeholder, focuses its text",
        hits.as_ref()
            .is_some_and(|hits| !hits.is_empty() && hits.iter().all(|&hit| hit)),
        format!("{hits:?}"),
    );
}

/// The AI panel's rows once the CLI saved a transcript of the session the AI tab that came back continues.
fn continued_once(
    controller: &crate::window::WindowController,
    folder: &Path,
) -> Option<Vec<(&'static str, bool, String)>> {
    let project = ide_ui_agent::agent_panel::claude_config_dir()
        .join("projects")
        .join(ide_claude::history::project_folder(folder));
    std::fs::create_dir_all(&project).ok()?;
    std::fs::write(
        project.join(format!("{SESSION}.jsonl")),
        "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"doppelt\"}}\n",
    )
    .ok()?;
    // As a tab whose session the index does not know yet: only the tab itself then tells the two apart.
    ide_ui_agent::of_window(controller)
        .into_iter()
        .find(|tab| tab.session_id().as_deref() == Some(SESSION))?;
    let mut index = ide_thread::ThreadIndex::open(&ide_thread::ThreadIndex::default_dir());
    let saved: Vec<ide_thread::ThreadId> = index
        .list(&ide_thread::Filter::default())
        .into_iter()
        .filter(|entry| entry.session.as_deref() == Some(SESSION))
        .map(|entry| entry.id.clone())
        .collect();
    for id in &saved {
        let _ = index.remove(id);
    }
    let shown = controller.inspect().panel;
    controller.activate_panel(Panel::Agents);
    let rows = ide_ui_agent::agent_panel::inspect(controller).map(|seen| seen.rows);
    if let Some(panel) = shown {
        controller.activate_panel(panel);
    }
    rows
}

/// The agent the window's layout would resume an Antigravity session of `folder` with, by its id.
fn other_agents_session(folder: &Path) -> Option<String> {
    let session = "51e1f7e5-0000-4000-8000-0000000000ff";
    let mut thread = ide_thread::Thread::new(ide_thread::ThreadId::new(), "antigravity-acp", folder);
    thread.apply(ide_thread::Update::Session(session.into()));
    let recorded = ide_thread::ThreadIndex::open(&ide_thread::ThreadIndex::default_dir())
        .record(&thread, std::time::SystemTime::now());
    if let Err(error) = recorded {
        return Some(format!("not recorded: {error}"));
    }
    crate::window::layout::past_session(folder, session).map(|(agent, _)| agent.id().to_string())
}

/// A folder without a frame of its own opens as large as the window saved last, a step aside of it.
fn newcomer(c: &Path, before: &Before) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    open_beside(mtm, c);
    let frame = window_of(c).and_then(|w| w.window()).map(|w| rect(w.frame()));
    let last = before.second_frame;
    check(
        "a folder opened for the first time takes the size of the window saved last, a step aside",
        frame.is_some_and(|f| (f[2] - last[2]).abs() < 1.0 && (f[3] - last[3]).abs() < 1.0 && f[0] > last[0]),
        format!("{frame:?} after {last:?}"),
    );
}
