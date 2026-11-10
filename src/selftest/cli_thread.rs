//! The `cli-thread` scenario (I5) with a stand-in for a coding agent's CLI, a shell script that works for a moment on
//! each line and rings the bell when done: the "+" menu in a trusted and an untrusted folder, the tab with its name
//! and status dot, the status through start, work and waiting, the activity view and the AI panel, image paste, the
//! end by closing the tab (Ctrl+C), and a CLI that quits by itself.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_process::Supervisor;
use ide_terminal::PasteAction;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSEventModifierFlags, NSMenu, NSPasteboard, NSPasteboardTypePNG};
use objc2_foundation::NSData;

use super::{check, finish, snapshot};
use crate::terminal::{CliStatus, TerminalView, inspect};
use crate::util::{constant, main_after};
use crate::window::{self, WindowController};

const NAME: &str = "cli-thread";

/// The stand-in: `{marker}` is where it notes that Ctrl+C reached it. It prints dots while it starts, until the first
/// line comes; then each line starts work, dots with a spinner in the title until the next line, and the bell after
/// it; `quit` ends it. Its phases follow the keys it gets, so it is seen working however late anyone looks.
const STAND_IN: &str = r#"#!/bin/sh
trap 'echo interrupted > "{marker}"; exit 130' INT
printf '\033]0;stand-in\007'
echo "stand-in starting"
( while :; do printf .; sleep 0.2; done ) &
ticker=$!
IFS= read -r first
kill $ticker 2>/dev/null
[ "$first" = quit ] && exit 0
echo
echo "stand-in ready"
while IFS= read -r line; do
  [ "$line" = quit ] && exit 0
  printf '\033]0;\342\240\213 working\007'
  ( while :; do printf .; sleep 0.1; done ) &
  worker=$!
  IFS= read -r stop
  kill $worker 2>/dev/null
  echo
  echo "done: $line"
  printf '\033]0;stand-in\007\a'
done
"#;

/// The stand-in's script and the file it writes on Ctrl+C.
struct Stage {
    root: PathBuf,
    script: PathBuf,
    marker: PathBuf,
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let home = ide_core::paths::home();
    let folder = home.join("cli-project");
    let _ = std::fs::create_dir_all(&folder);
    let _ = std::fs::write(folder.join("README.md"), "# CLI\n");
    let root = folder.canonicalize().unwrap_or(folder);
    let bin = home.join("stand-in");
    let _ = std::fs::create_dir_all(&bin);
    let script = bin.join("claude");
    let marker = home.join("interrupted");
    let written = std::fs::write(&script, STAND_IN.replace("{marker}", &marker.to_string_lossy())).and_then(|()| {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
    });
    check("the stand-in CLI is written", written.is_ok(), format!("{written:?}"));
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    let stage = Stage { root, script, marker };
    // The menu needs the login shell's PATH.
    std::thread::spawn(move || {
        let _ = crate::environment::wait(Duration::from_secs(30));
        crate::util::main_soon(move || menus(stage));
    });
}

fn controller_of(root: &Path) -> Option<Retained<WindowController>> {
    window::open_windows()
        .into_iter()
        .find(|controller| controller.project().as_deref() == Some(root))
}

/// The CLI-Thread menu's entries: title, whether enabled.
fn menu_entries(controller: &WindowController) -> Vec<(String, bool)> {
    let menu = NSMenu::new(controller.mtm());
    crate::terminal::fill_cli_menu(controller, &menu);
    menu.itemArray()
        .iter()
        .map(|item| (item.title().to_string(), item.isEnabled()))
        .collect()
}

fn menus(stage: Stage) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(controller) = controller_of(&stage.root) else {
        check("the folder opens", false, "");
        return finish(NAME);
    };
    let trusted = menu_entries(&controller);
    let names = ["claude", "codex", "agy"];
    check(
        "in a trusted folder the menu offers the CLIs, enabled where installed",
        names.iter().all(|name| trusted.iter().any(|(title, _)| title == name))
            && trusted
                .iter()
                .all(|(title, _)| names.contains(&title.as_str()) || title == "gemini"),
        format!("{trusted:?}"),
    );
    let other = ide_core::paths::home().join("untrusted-project");
    let _ = std::fs::create_dir_all(&other);
    let other = other.canonicalize().unwrap_or(other);
    super::helpers::open_beside(mtm, &other);
    let untrusted = controller_of(&other).map(|c| menu_entries(&c)).unwrap_or_default();
    check(
        "in an untrusted folder the menu says why and offers nothing",
        untrusted
            .first()
            .is_some_and(|(title, enabled)| title.contains("vertrauten") && !enabled)
            && untrusted.iter().all(|(_, enabled)| !enabled),
        format!("{untrusted:?}"),
    );
    open_stand_in(stage);
}

/// Calls `then` with whether `done` came true within `limit`, asking every 100 ms.
fn wait(
    id: u64,
    limit: Duration,
    done: impl Fn(&TerminalView) -> bool + Send + 'static,
    then: impl FnOnce(bool) + Send + 'static,
) {
    poll(
        Instant::now() + limit,
        move || inspect::view(id).is_some_and(|view| done(&view)),
        then,
    );
}

fn poll(deadline: Instant, done: impl Fn() -> bool + Send + 'static, then: impl FnOnce(bool) + Send + 'static) {
    let held = done();
    if held || Instant::now() >= deadline {
        then(held);
        return;
    }
    main_after(Duration::from_millis(100), move || poll(deadline, done, then));
}

fn status(id: u64) -> Option<CliStatus> {
    inspect::view(id).and_then(|view| view.cli_status())
}

fn activity(controller: &WindowController) -> String {
    let (lines, _) = crate::activity::inspect(controller);
    crate::activity::close(controller);
    lines.join(" | ")
}

/// Whether the stand-in started and prints its dots.
fn starting(view: &TerminalView) -> bool {
    let lines = view.visible_lines();
    lines.iter().any(|line| line == "stand-in starting") && lines.iter().any(|line| line.starts_with(".."))
}

fn open_stand_in(stage: Stage) {
    let Some(controller) = controller_of(&stage.root) else {
        return finish(NAME);
    };
    let tabs = controller.tab_count();
    crate::terminal::open_cli_program(&controller, "claude", stage.script.clone());
    let Some((id, _)) = inspect::latest() else {
        return finish(NAME);
    };
    let titles = controller.tab_titles();
    check(
        "the CLI opens in a tab of its own, named after it",
        controller.tab_count() == tabs + 1 && titles.iter().any(|title| title == "claude"),
        titles.join(", "),
    );
    wait(id, Duration::from_secs(10), starting, move |_| {
        // The stand-in prints until it gets a line, so it must show working now, however late this runs.
        let printing = status(id);
        if let Some(view) = inspect::view(id) {
            view.type_text("\r");
        }
        let waiting = |view: &TerminalView| {
            view.cli_status() == Some(CliStatus::Waiting)
                && view.visible_lines().iter().any(|line| line == "stand-in ready")
        };
        wait(id, Duration::from_secs(15), waiting, move |waits| {
            check(
                "a CLI that started and went quiet waits for input",
                printing == Some(CliStatus::Working) && waits,
                format!("{printing:?} while it printed, then {:?}", status(id)),
            );
            listed(id, stage);
        });
    });
}

/// The activity view and the AI panel list the waiting CLI.
fn listed(id: u64, stage: Stage) {
    let Some(controller) = controller_of(&stage.root) else {
        return finish(NAME);
    };
    let lines = activity(&controller);
    check(
        "the activity view lists the CLI with its status",
        lines.contains(&format!("claude — {}", CliStatus::Waiting.text())),
        lines,
    );
    let panel = crate::agent_panel::inspect(&controller)
        .map(|seen| seen.sessions)
        .unwrap_or_default();
    let group = panel
        .iter()
        .position(|row| row == splat_i18n::t("Im Terminal gestartet"));
    check(
        "the AI panel lists the CLI under the sessions started in a terminal",
        group.is_some_and(|at| panel.get(at + 1).is_some_and(|row| row.trim() == "claude")),
        panel.join(" | "),
    );
    work(id, stage);
}

/// A line to work on: working while the dots come, waiting after the bell.
fn work(id: u64, stage: Stage) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    view.type_text("go\r");
    let working = |view: &TerminalView| view.cli_status() == Some(CliStatus::Working);
    wait(id, Duration::from_secs(10), working, move |working| {
        let busy = stage_controller(&stage).map(|c| activity(&c)).unwrap_or_default();
        // The next line ends the work.
        if let Some(view) = inspect::view(id) {
            view.type_text("\r");
        }
        let done = |view: &TerminalView| {
            view.visible_lines().iter().any(|line| line == "done: go") && view.cli_status() == Some(CliStatus::Waiting)
        };
        wait(id, Duration::from_secs(10), done, move |waits| {
            check(
                "a CLI at work shows working, and waiting once it rings the bell",
                working && waits && busy.contains(CliStatus::Working.text()),
                format!("{busy}; then {:?}", status(id)),
            );
            if let Some(window) = inspect::view(id).and_then(|view| view.window()) {
                snapshot(&window, "cli-thread");
                if hold(&window) {
                    return;
                }
            }
            paste(id, stage);
        });
    });
}

/// With `IDE_SELFTEST_HOLD` set, writes the window's number next to the report and stays, for a real capture
/// (`screencapture -l`) of what pictures drawn from the views miss, such as the tinted status dot.
fn hold(window: &objc2_app_kit::NSWindow) -> bool {
    let (Some(_), Some(report)) = (
        std::env::var_os("IDE_SELFTEST_HOLD"),
        std::env::var_os("IDE_SELFTEST_REPORT"),
    ) else {
        return false;
    };
    let file = PathBuf::from(report).with_file_name("cli-thread.window");
    std::fs::write(file, window.windowNumber().to_string()).is_ok()
}

fn stage_controller(stage: &Stage) -> Option<Retained<WindowController>> {
    controller_of(&stage.root)
}

/// An image pasted into the CLI goes in as in any terminal.
fn paste(id: u64, stage: Stage) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    let pasteboard = NSPasteboard::pasteboardWithUniqueName();
    pasteboard.clearContents();
    pasteboard.setData_forType(
        Some(&NSData::with_bytes(&super::terminal::PICTURE)),
        constant!(NSPasteboardTypePNG),
    );
    let action = view.paste_image_from(&pasteboard);
    let name = match &action {
        Some(PasteAction::InsertPath(path)) => path.file_name().map(|n| n.to_string_lossy().into_owned()),
        _ => None,
    };
    let shown = move |view: &TerminalView| {
        name.as_ref()
            .is_some_and(|name| view.visible_lines().iter().any(|line| line.contains(name.as_str())))
    };
    wait(id, Duration::from_secs(3), shown, move |shown| {
        check(
            "an image pasted into the CLI goes in as in any terminal",
            shown,
            format!("{action:?}"),
        );
        if let Some(view) = inspect::view(id) {
            view.press(0x20, "\u{15}", "u", NSEventModifierFlags::Control);
        }
        close(id, stage);
    });
}

/// Closing the tab sends Ctrl+C, which the stand-in notes, and ends it.
fn close(id: u64, stage: Stage) {
    let (Some(view), Some(controller)) = (inspect::view(id), stage_controller(&stage)) else {
        return finish(NAME);
    };
    let pid = view.shell();
    if let Some(tab) = view.tab() {
        controller.close_tab(tab);
    }
    drop(view);
    let marker = stage.marker.clone();
    let ended =
        move || marker.exists() && pid.is_some_and(|pid| Supervisor::global().running().iter().all(|p| p.pid != pid));
    poll(Instant::now() + Duration::from_secs(8), ended, move |ended| {
        let lines = stage_controller(&stage).map(|c| activity(&c)).unwrap_or_default();
        check(
            "closing the tab ends the CLI with Ctrl+C and takes it off the activity view",
            ended && !lines.contains("claude"),
            format!("marker {}, {lines}", stage.marker.exists()),
        );
        quits(stage);
    });
}

/// A CLI that quits by itself is finished and keeps its tab.
fn quits(stage: Stage) {
    let Some(controller) = stage_controller(&stage) else {
        return finish(NAME);
    };
    crate::terminal::open_cli_program(&controller, "claude", stage.script.clone());
    let Some((id, _)) = inspect::latest() else {
        return finish(NAME);
    };
    let tabs = controller.tab_count();
    wait(id, Duration::from_secs(10), starting, move |_| {
        if let Some(view) = inspect::view(id) {
            view.type_text("quit\r");
        }
        let finished = |view: &TerminalView| view.cli_status() == Some(CliStatus::Finished);
        wait(id, Duration::from_secs(5), finished, move |finished| {
            let kept = stage_controller(&stage).map_or(0, |c| c.tab_count());
            let lines = stage_controller(&stage).map(|c| activity(&c)).unwrap_or_default();
            check(
                "a CLI that quits is finished and keeps its tab",
                finished && kept == tabs && lines.contains(CliStatus::Finished.text()),
                format!("{tabs} then {kept} tabs; {lines}"),
            );
            finish(NAME);
        });
    });
}
