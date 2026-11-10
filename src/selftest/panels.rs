//! The `panels` scenario (E57): every document opens in a native tab of its own, the tabs of a folder share one tab
//! bar, the sidebar follows the tab in front without changing, a document opens beside another in its tab and closing
//! one side leaves the other, a terminal keeps its shell through all of it, and the tabs come back when the folder
//! opens again.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ide_process::Supervisor;
use objc2::MainThreadMarker;
use objc2::rc::Retained;

use super::{check, finish, snapshot};
use crate::util::main_after;
use crate::window::{self, WindowController};

const SCENARIO: &str = "panels";
const FILES: [&str; 4] = ["a.rs", "b.rs", "c.rs", "d.rs"];
/// A pause for the window to draw before a picture or a step that needs its layout.
const PAUSE: Duration = Duration::from_millis(600);
/// The share of the left document that the tab of a.rs keeps.
const SHARE: f64 = 0.35;

/// The session of the AI tab, which comes back by its id.
const SESSION: &str = "51e1f7e5-0000-4000-8000-0000000000bb";

/// Opens an AI tab that plays the start of the session `session`.
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

/// The terminal's number, its document and its shell's process.
static TERMINAL: AtomicU64 = AtomicU64::new(0);
static TERMINAL_TAB: AtomicIsize = AtomicIsize::new(-1);
static SHELL: AtomicU32 = AtomicU32::new(0);

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

fn shown(controller: &WindowController) -> String {
    controller
        .window_tabs()
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

/// Whether the sidebar is in the window of the tab in front.
fn sidebar_in_front(controller: &WindowController) -> bool {
    match (controller.sidebar_window(), controller.window()) {
        (Some(sidebar), Some(front)) => std::ptr::eq(&*sidebar, &*front),
        _ => false,
    }
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
    for name in &FILES[..3] {
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
    let tabs = controller.window_tabs();
    let grouped = controller
        .tab_windows()
        .first()
        .and_then(|window| window.tabbedWindows())
        .map_or(0, |tabbed| tabbed.count());
    check(
        "each document opens in a native tab of its own, all in the window's one tab bar",
        tabs.len() == 4 && tabs.iter().all(|docs| docs.len() == 1) && grouped == 4 && controller.tab_bar_visible(),
        format!("{}, {grouped} in the group", shown(&controller)),
    );
    check("the tab in front has the sidebar", sidebar_in_front(&controller), "");
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
                switching(root);
            } else {
                ask(root, deadline);
            }
        });
    }
    ask(root, deadline);
}

/// Bringing another tab to the front moves the sidebar along as it was.
fn switching(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let shell = shell_now();
    check("the terminal's shell starts", shell.is_some(), format!("{shell:?}"));
    SHELL.store(shell.unwrap_or(0), Ordering::SeqCst);
    let before = controller.inspect();
    let rows = controller.tree_rows();
    controller.select_tab_by_id(tab_of(&controller, &root, "a.rs"));
    let after = controller.inspect();
    check(
        "another tab in front gets the sidebar as it was: same panel, width and rows",
        sidebar_in_front(&controller)
            && before.panel == after.panel
            && (before.sidebar_width - after.sidebar_width).abs() < 1.0
            && rows == controller.tree_rows(),
        format!(
            "{:?} {:.0} {rows} then {:?} {:.0} {}",
            before.panel,
            before.sidebar_width,
            after.panel,
            after.sidebar_width,
            controller.tree_rows()
        ),
    );
    let ids = controller.window_tab_ids();
    let a = tab_of(&controller, &root, "a.rs");
    let next = ids
        .iter()
        .position(|docs| docs.contains(&a))
        .and_then(|index| ids.get((index + 1) % ids.len()))
        .and_then(|docs| docs.first().copied());
    controller.select_next_tab();
    check(
        "⇧⌘] brings the next tab to the front, with the sidebar",
        next.is_some() && controller.selected_tab() == next && sidebar_in_front(&controller),
        format!("{:?} after {ids:?}", controller.selected_tab()),
    );
    dragged_out(&controller, &root);
    controller.select_tab_by_id(tab_of(&controller, &root, "a.rs"));
    main_after(PAUSE, move || beside(root));
}

/// A tab moved to a window of its own keeps the sidebar's picture while the other window has the sidebar, gets the
/// sidebar back in front, and joins again with "Merge All Windows".
fn dragged_out(controller: &WindowController, root: &Path) {
    let c = tab_of(controller, root, "c.rs");
    controller.select_tab_by_id(c);
    let Some(window) = controller.window() else { return };
    window.moveTabToNewWindow(None);
    let alone = window.tabbedWindows().is_none_or(|tabbed| tabbed.count() == 1);
    controller.select_tab_by_id(tab_of(controller, root, "a.rs"));
    let pictures = controller.sidebar_pictures();
    let apart_ok = alone && sidebar_in_front(controller) && pictures.iter().filter(|shown| !**shown).count() == 1;
    controller.select_tab_by_id(c);
    check(
        "a tab moved to a window of its own keeps a picture of the sidebar and gets the sidebar in front",
        apart_ok && sidebar_in_front(controller) && controller.tab_windows().len() == 4,
        format!("alone {alone}, pictures {pictures:?}, {}", shown(controller)),
    );
    window.mergeAllWindows(None);
    let grouped = window.tabbedWindows().map_or(0, |tabbed| tabbed.count());
    check(
        "\"Merge All Windows\" brings it back into the tab bar",
        grouped == 4,
        format!("{grouped} in the group"),
    );
}

/// A file opens beside the one in front, and the focus moves between the two.
fn beside(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "panels-tabs");
    }
    let a = tab_of(&controller, &root, "a.rs");
    let windows = controller.tab_windows().len();
    controller.open_file_beside(&root.join("d.rs"));
    let d = tab_of(&controller, &root, "d.rs");
    check(
        "a file opens beside the one in front, in its tab",
        controller.window_tabs().iter().any(|docs| docs == &["a.rs", "d.rs"])
            && controller.tab_windows().len() == windows,
        shown(&controller),
    );
    controller.select_tab_by_id(a);
    controller.focus_next_panel();
    check(
        "the focus moves on to the other document of the tab",
        controller.selected_tab() == Some(d),
        format!("{:?}", controller.selected_tab()),
    );
    controller.set_share_of(a, SHARE);
    let alive = crate::terminal::inspect::view(TERMINAL.load(Ordering::SeqCst)).is_some_and(|view| !view.has_exited());
    check(
        "the terminal keeps its shell while other tabs come to the front",
        alive && shell_now() == Some(SHELL.load(Ordering::SeqCst)),
        format!("{:?}", shell_now()),
    );
    main_after(PAUSE, move || closing_side(root));
}

/// Closing one of two documents leaves the other alone in its tab; closing a tab's only one closes the tab.
fn closing_side(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "panels-beside");
    }
    let windows = controller.tab_windows().len();
    let b = tab_of(&controller, &root, "b.rs");
    controller.close_tab(b);
    check(
        "closing a tab's only document closes the tab",
        controller.tab_windows().len() == windows - 1 && tab_of(&controller, &root, "b.rs") == -1,
        shown(&controller),
    );
    let windows = controller.tab_windows().len();
    let d = tab_of(&controller, &root, "d.rs");
    controller.close_tab(d);
    check(
        "closing one of two documents leaves the other alone in the tab",
        controller.window_tabs().iter().any(|docs| docs == &["a.rs"]) && controller.tab_windows().len() == windows,
        shown(&controller),
    );
    let a = tab_of(&controller, &root, "a.rs");
    controller.select_tab_by_id(a);
    controller.open_file_beside(&root.join("d.rs"));
    controller.set_share_of(a, SHARE);
    let replayed = replay_agent(&controller, SESSION) && has_session(&controller, SESSION);
    check("an AI thread opens in a tab of its own", replayed, shown(&controller));
    main_after(PAUSE, move || closing(root));
}

fn closing(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let expected = controller.window_tabs();
    let share = controller.tab_shares().into_iter().flatten().next();
    controller.close_all_tabs();
    main_after(PAUSE * 3, move || reopen(root, expected, share));
}

fn reopen(root: PathBuf, expected: Vec<Vec<String>>, share: Option<f64>) {
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
        "closing the window saves its tabs",
        saved.as_ref().is_some_and(|layout| {
            layout.tabs.len() == expected.len()
                && layout.has(&ide_core::layout::Tab::Agent {
                    session: SESSION.into(),
                })
                && layout.tabs.iter().any(|tab| tab.documents.len() == 2)
        }),
        format!("{saved:?}"),
    );
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(mtm, root.clone());
    main_after(PAUSE, move || restored(root, expected, share));
}

fn restored(root: PathBuf, expected: Vec<Vec<String>>, share: Option<f64>) {
    let Some(controller) = window_of(&root) else { return };
    let tabs = controller.window_tabs();
    check(
        "the tabs come back in their order when the folder opens again, a.rs with d.rs beside it",
        tabs == expected && tabs.iter().any(|docs| docs == &["a.rs", "d.rs"]),
        format!("{} after {expected:?}", shown(&controller)),
    );
    check(
        "the AI thread comes back as its session, played from a recording here",
        has_session(&controller, SESSION),
        shown(&controller),
    );
    let now = controller.tab_shares().into_iter().flatten().next();
    check(
        "the two documents get back their widths",
        share.zip(now).is_some_and(|(before, now)| (before - now).abs() < 0.05),
        format!("{share:.2?} then {now:.2?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "panels-restored");
    }
    controller.close_all_tabs();
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
