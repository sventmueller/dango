//! The `restore` scenario, run twice in one home: the first launch sets up two folder windows, one with documents
//! of every kind that comes back, CLI threads of stand-ins included, a frame of its own, a wider sidebar and the Git
//! panel, the other with its sidebar folded away, and quits as the user does; the second checks that both come back
//! that way, the CLIs continuing their sessions, and that a folder opened for the first time takes the frame of the
//! window saved last.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use serde::{Deserialize, Serialize};

use super::helpers::{open_beside, wait_until, window_of};
use super::{check, finish};
use crate::util::main_after;
use crate::window::Panel;

const SCENARIO: &str = "restore";
/// The session of the AI tab, which comes back by its id.
const SESSION: &str = "51e1f7e5-0000-4000-8000-0000000000cc";
/// A pause for the windows to settle before the next step.
const PAUSE: Duration = Duration::from_millis(800);
const FRAME: NSRect = NSRect::new(NSPoint::new(120.0, 140.0), NSSize::new(1100.0, 720.0));
const SIDEBAR: f64 = 380.0;
/// The model, effort and mode picked in the AI thread before the quit.
const PICKS: (&str, &str, &str) = ("opus", "high", "acceptEdits");
/// The session of the Claude Code stand-in, as a transcript in the CLI's folder names it.
const CLI_SESSION: &str = "51e1f7e5-0000-4000-8000-0000000000dd";
/// A stand-in for a coding agent's CLI: it says how it was started and waits.
const STAND_IN: &str = "#!/bin/sh\necho \"stand-in {name} args: $*\"\nwhile IFS= read -r line; do :; done\n";

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
    let home = ide_core::paths::home();
    let canonical = |name: &str| {
        let folder = home.join(name);
        let _ = std::fs::create_dir_all(&folder);
        folder.canonicalize().unwrap_or(folder)
    };
    (canonical("restore-a"), canonical("restore-b"), canonical("restore-c"))
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
        for name in ["claude", "codex"] {
            let path = ide_core::paths::home().join(format!("stand-in-{name}"));
            let written = std::fs::write(&path, STAND_IN.replace("{name}", name)).and_then(|()| {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            });
            check("a stand-in CLI is written", written.is_ok(), format!("{written:?}"));
        }
        for folder in [&a, &b, &c] {
            crate::project::trust(folder);
        }
        crate::app::open_project(mtm, a.clone());
        open_beside(mtm, &b);
        main_after(PAUSE, move || arrange(a, b));
    } else {
        let saved = saved_frame(&a);
        let probe = (a.clone(), b.clone());
        wait_until(
            move || window_of(&probe.0).is_some() && window_of(&probe.1).is_some(),
            move |_| main_after(PAUSE, move || came_back(a, b, c, saved)),
        );
    }
}

fn arrange(a: PathBuf, b: PathBuf) {
    let (Some(first), Some(second)) = (window_of(&a), window_of(&b)) else {
        check("both folders get their windows", false, "");
        return finish(SCENARIO);
    };
    first.open_file(&a.join("main.rs"));
    first.open_file(&a.join("notes.md"));
    crate::terminal::open_tab(&first, Some(&a));
    let replayed = crate::selftest::replay_agent(&first, SESSION);
    if let Some(tab) = crate::agent::of_window(&first).first() {
        tab.pick_all(PICKS.0, PICKS.1, PICKS.2);
    }
    for name in ["claude", "codex"] {
        let program = ide_core::paths::home().join(format!("stand-in-{name}"));
        crate::terminal::open_cli_program(&first, name, program);
    }
    claude_transcript(&a);
    first.activate_panel(Panel::Git);
    first.set_sidebar_width(SIDEBAR);
    if let Some(window) = first.window() {
        window.setFrame_display(FRAME, true);
    }
    second.open_file(&b.join("lib.rs"));
    second.toggle_sidebar();
    check("the AI thread plays its recording before the quit", replayed, "");
    main_after(PAUSE, move || quit(&a, &b));
}

/// The transcript Claude Code would begin in `folder`, in the CLI's folder of this run's home only.
fn claude_transcript(folder: &Path) {
    let config = crate::agent_panel::claude_config_dir();
    if !config.starts_with(ide_core::paths::home()) {
        return check("the CLI's folder is this run's", false, config.display().to_string());
    }
    let project = config
        .join("projects")
        .join(ide_claude::history::project_folder(folder));
    let written = std::fs::create_dir_all(&project).and_then(|()| {
        std::fs::write(
            project.join(format!("{CLI_SESSION}.jsonl")),
            "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"hallo\"}}\n",
        )
    });
    check(
        "the stand-in's transcript is written",
        written.is_ok(),
        format!("{written:?}"),
    );
}

fn quit(a: &Path, b: &Path) {
    let (Some(first), Some(second)) = (window_of(a), window_of(b)) else {
        return finish(SCENARIO);
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
    finish(SCENARIO);
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
    let (Some(before), Some(first), Some(second)) = (before, window_of(&a), window_of(&b)) else {
        check("both folder windows come back", false, "");
        return finish(SCENARIO);
    };
    check(
        "each folder window comes back with its tabs in their order, the Markdown file with its preview",
        first.window_tabs() == before.first && second.window_tabs() == before.second,
        format!("{:?} / {:?}", first.window_tabs(), second.window_tabs()),
    );
    let sessions = crate::agent::of_window(&first)
        .iter()
        .filter_map(|tab| tab.session_id())
        .collect::<Vec<_>>();
    check(
        "the AI thread comes back as its session, played from a recording here",
        sessions.iter().any(|session| session == SESSION),
        format!("{sessions:?}"),
    );
    let wanted = (
        Some(PICKS.0.to_string()),
        Some(PICKS.1.to_string()),
        PICKS.2.to_string(),
    );
    let restored = crate::agent::of_window(&first)
        .iter()
        .find(|tab| tab.session_id().as_deref() == Some(SESSION))
        .map(|tab| tab.picks());
    let fresh = crate::agent::open_replay(&first).map(|tab| tab.picks());
    check(
        "the AI thread that came back and a new one start with the model, effort and mode picked last",
        restored.as_ref() == Some(&wanted) && fresh.as_ref() == Some(&wanted),
        format!("{restored:?} / {fresh:?}"),
    );
    let hits = crate::agent::of_window(&first)
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
    newcomer(&c, &before);
    let wanted = [
        ("claude", format!("stand-in claude args: --resume {CLI_SESSION}")),
        ("codex", "stand-in codex args: resume --last".to_string()),
    ];
    let probe = a.clone();
    let shown = move |name: &str, line: &str| {
        let controller = window_of(&probe);
        let id = controller.as_ref().and_then(|c| super::helpers::tab(c, name));
        controller
            .zip(id)
            .and_then(|(c, id)| crate::terminal::inspect::of_tab(&c, id))
            .is_some_and(|view| view.visible_lines().iter().any(|l| l == line))
    };
    let lines = wanted.clone();
    wait_until(
        move || lines.iter().all(|(name, line)| shown(name, line)),
        move |ok| {
            check(
                "the CLI threads come back continuing their sessions: Claude Code its own, Codex its latest",
                ok,
                format!("{wanted:?}"),
            );
            finish(SCENARIO);
        },
    );
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
