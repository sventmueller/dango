//! The `panels` scenario (I10): tabs open in panels of their own to the right and below, move by the drag code path,
//! a panel goes with its last tab, a terminal keeps its shell when it moves, and the layout comes back when the
//! folder opens again.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ide_process::Supervisor;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use splat_i18n::tf;

use super::{check, finish, snapshot};
use crate::util::main_after;
use crate::window::{self, Side, WindowController};

const SCENARIO: &str = "panels";
const FILES: [&str; 3] = ["a.rs", "b.rs", "c.rs"];
/// A pause for the window to draw before a picture or a step that needs its layout.
const PAUSE: Duration = Duration::from_millis(600);

/// The session of the AI tab, which comes back by its id.
const SESSION: &str = "51e1f7e5-0000-4000-8000-0000000000bb";

/// Opens an AI tab in the active panel that plays the start of the session `session`.
pub(super) fn replay_agent(controller: &WindowController, session: &str) -> bool {
    let Some(tab) = crate::agent::open_replay(controller) else {
        return false;
    };
    let cwd = controller.project_root().unwrap_or_default();
    let init = serde_json::json!({"type": "system", "subtype": "init", "cwd": cwd, "session_id": session,
        "tools": [], "mcp_servers": [], "model": "claude-opus-5-5", "permissionMode": "default",
        "slash_commands": [], "apiKeySource": "none", "claude_code_version": "2.1.319", "output_style": "default",
        "agents": [], "skills": [], "plugins": [], "capabilities": [], "uuid": "51e1f7e5-0000-4000-8000-0000000000b1"});
    tab.feed(&format!("{init}\n"));
    true
}

/// Whether an AI tab of the window shows the session `session`.
fn has_session(controller: &WindowController, session: &str) -> bool {
    crate::agent::of_window(controller)
        .iter()
        .any(|tab| tab.session_id().as_deref() == Some(session))
}

/// The terminal's number, its tab and its shell's process.
static TERMINAL: AtomicU64 = AtomicU64::new(0);
static TERMINAL_TAB: AtomicIsize = AtomicIsize::new(-1);
static SHELL: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// The panels' sizes when the window closed.
    static SHARES: std::cell::RefCell<Vec<f64>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn window_of(root: &Path) -> Option<Retained<WindowController>> {
    let controller = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_deref() == Some(root));
    if controller.is_none() {
        check("the folder gets its window", false, root.display().to_string());
        finish(SCENARIO);
    }
    controller
}

/// The place of the panel that holds the tab `id`.
fn panel_of(controller: &WindowController, id: isize) -> Option<usize> {
    controller.panel_tab_ids().iter().position(|ids| ids.contains(&id))
}

fn shown(controller: &WindowController) -> String {
    controller
        .panel_titles()
        .iter()
        .map(|titles| format!("[{}]", titles.join(", ")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn tab_of(controller: &WindowController, root: &Path, name: &str) -> isize {
    controller
        .tab_with_key(&root.join(name).to_string_lossy())
        .unwrap_or(-1)
}

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = ide_core::paths::home().join("panels-project");
    let _ = std::fs::create_dir_all(&folder);
    for name in FILES {
        let _ = std::fs::write(folder.join(name), format!("// {name}\nfn main() {{}}\n"));
    }
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder.clone());
    let root = folder.canonicalize().unwrap_or(folder);
    main_after(PAUSE, move || opened(root));
}

fn opened(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let welcome = controller.selected_tab();
    for name in FILES {
        controller.open_file(&root.join(name));
    }
    if let Some(welcome) = welcome {
        controller.close_tab(welcome);
    }
    crate::terminal::open_tab(&controller, None);
    TERMINAL_TAB.store(controller.selected_tab().unwrap_or(-1), Ordering::SeqCst);
    if let Some((id, _)) = crate::terminal::inspect::latest() {
        TERMINAL.store(id, Ordering::SeqCst);
    }
    wait_for_shell(root);
}

fn shell_now() -> Option<u32> {
    crate::terminal::inspect::view(TERMINAL.load(Ordering::SeqCst)).and_then(|view| view.shell())
}

fn wait_for_shell(root: PathBuf) {
    let deadline = Instant::now() + Duration::from_secs(30);
    fn ask(root: PathBuf, deadline: Instant) {
        main_after(Duration::from_millis(200), move || {
            if shell_now().is_some() || Instant::now() > deadline {
                split(root);
            } else {
                ask(root, deadline);
            }
        });
    }
    ask(root, deadline);
}

fn split(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let shell = shell_now();
    check("the terminal's shell starts", shell.is_some(), format!("{shell:?}"));
    SHELL.store(shell.unwrap_or(0), Ordering::SeqCst);
    let terminal = TERMINAL_TAB.load(Ordering::SeqCst);
    controller.split_tab(terminal, Side::Right);
    check(
        "a tab opens in a panel of its own on the right",
        controller.panel_tab_ids().len() == 2 && panel_of(&controller, terminal) == Some(1),
        shown(&controller),
    );
    let still = shell_now();
    let alive = crate::terminal::inspect::view(TERMINAL.load(Ordering::SeqCst)).is_some_and(|view| !view.has_exited());
    check(
        "the moved terminal keeps its shell",
        alive && still.is_some() && still == shell,
        format!("{shell:?} then {still:?}"),
    );
    let marks = controller.panel_marks();
    check(
        "the active panel's bar shows the accent, the other does not",
        marks.iter().filter(|on| **on).count() == 1,
        format!("{marks:?}"),
    );
    let b = tab_of(&controller, &root, "b.rs");
    controller.split_tab(b, Side::Below);
    check(
        "a tab opens in a panel of its own below",
        controller.panel_tab_ids().len() == 3 && panel_of(&controller, b) == Some(1),
        shown(&controller),
    );
    let labels = controller.panel_labels();
    check(
        "VoiceOver tells the panels apart",
        labels.first() == Some(&tf("Panel {} von {}", &["1", "3"])),
        labels.join(", "),
    );
    let before = controller.active_panel();
    controller.focus_next_panel();
    let after = controller.active_panel();
    check(
        "the focus moves on to the next panel",
        before.is_some() && after.is_some() && before != after,
        format!("{before:?} then {after:?}"),
    );
    main_after(PAUSE, move || dragged(root));
}

fn dragged(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "panels-split");
    }
    let terminal = TERMINAL_TAB.load(Ordering::SeqCst);
    let c = tab_of(&controller, &root, "c.rs");
    let Some(target) = panel_of(&controller, terminal) else {
        check("the terminal's panel is found", false, shown(&controller));
        finish(SCENARIO);
        return;
    };
    controller.drag_tab_onto(c, target, None, true);
    check(
        "a tab dropped on another panel's bar moves there",
        panel_of(&controller, c) == panel_of(&controller, terminal),
        shown(&controller),
    );
    controller.drag_tab_onto(c, target, Some(Side::Right), false);
    main_after(PAUSE, move || dropped(root, c, target));
}

fn dropped(root: PathBuf, c: isize, target: usize) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "panels-dragging");
    }
    let terminal = TERMINAL_TAB.load(Ordering::SeqCst);
    let panels = controller.panel_tab_ids().len();
    controller.drag_tab_onto(c, target, Some(Side::Right), true);
    check(
        "a tab dropped on a panel's right edge opens a new panel there",
        controller.panel_tab_ids().len() == panels + 1
            && panel_of(&controller, c) == panel_of(&controller, terminal).map(|p| p + 1),
        shown(&controller),
    );
    let b = tab_of(&controller, &root, "b.rs");
    let panels = controller.panel_tab_ids().len();
    controller.close_tab(b);
    check(
        "closing a panel's last tab closes the panel",
        controller.panel_tab_ids().len() == panels - 1,
        shown(&controller),
    );
    let still = shell_now();
    check(
        "the terminal kept its shell through every move",
        still.is_some() && still == Some(SHELL.load(Ordering::SeqCst)),
        format!("{still:?}"),
    );
    main_after(PAUSE, move || closing(root));
}

fn closing(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "panels-moved");
    }
    let expected = controller.panel_tab_ids().len();
    let c = tab_of(&controller, &root, "c.rs");
    controller.select_tab_by_id(c);
    let replayed = replay_agent(&controller, SESSION) && has_session(&controller, SESSION);
    let with_two = controller.panel_tab_ids().iter().position(|ids| ids.len() == 2);
    check(
        "an AI tab joins the active panel",
        replayed && with_two.is_some() && panel_of(&controller, c) == with_two,
        shown(&controller),
    );
    SHARES.with(|shares| *shares.borrow_mut() = controller.panel_shares());
    if let Some(window) = controller.window() {
        window.performClose(None);
    }
    main_after(PAUSE * 3, move || reopen(root, expected));
}

fn reopen(root: PathBuf, expected: usize) {
    let shell = SHELL.load(Ordering::SeqCst);
    check(
        "closing the window ends its terminal's shell",
        Supervisor::global()
            .running()
            .iter()
            .all(|process| process.pid != shell),
        format!("shell {shell}"),
    );
    let saved = ide_core::layout::load(&root);
    check(
        "closing the window saves its panels",
        saved.as_ref().is_some_and(|layout| {
            layout.tab_count() == 4
                && layout.has(&ide_core::layout::Tab::Agent {
                    session: SESSION.into(),
                })
        }),
        format!("{saved:?}"),
    );
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(mtm, root.clone());
    main_after(PAUSE, move || restored(root, expected));
}

fn restored(root: PathBuf, expected: usize) {
    let Some(controller) = window_of(&root) else { return };
    let titles = controller.panel_titles();
    check(
        "the panels come back when the folder opens again",
        titles.len() == expected
            && titles.first().is_some_and(|t| t == &["a.rs"])
            && titles.last().is_some_and(|t| t.len() == 2 && t[0] == "c.rs")
            && titles.get(1).is_some_and(|t| t.len() == 1),
        shown(&controller),
    );
    check(
        "the AI tab comes back as its session, played from a recording here",
        has_session(&controller, SESSION),
        shown(&controller),
    );
    let before = SHARES.with(|shares| shares.borrow().clone());
    let after = controller.panel_shares();
    check(
        "the panels get back their sizes",
        before.len() == after.len() && before.iter().zip(&after).all(|(a, b)| (a - b).abs() < 0.05),
        format!("{before:.2?} then {after:.2?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "panels-restored");
    }
    if let Some(window) = controller.window() {
        window.performClose(None);
    }
    wind_down(Instant::now());
}

/// Finishes once the window's processes ended, its shell and any language server: a quit from inside a main-queue
/// block cannot wait for running processes, since the app's answer to the quit comes through that same queue.
fn wind_down(since: Instant) {
    let running = Supervisor::global().running();
    if running.is_empty() || since.elapsed() > Duration::from_secs(8) {
        let left: Vec<String> = running
            .iter()
            .map(|process| format!("{:?} {}", process.kind, process.label))
            .collect();
        check(
            "closing the window ends its processes",
            running.is_empty(),
            left.join(", "),
        );
        finish(SCENARIO);
    } else {
        main_after(Duration::from_millis(200), move || wind_down(since));
    }
}
