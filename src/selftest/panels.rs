//! The `panels` scenario (E57): every document opens in a native tab of its own, the tabs of a folder share one tab
//! bar, the sidebar follows the tab in front without changing, a document opens beside another in its tab and closing
//! one side leaves the other, a terminal keeps its shell through all of it, and the tabs come back when the folder
//! opens again.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use ide_process::Supervisor;
use objc2::MainThreadMarker;
use objc2::rc::Retained;

use super::{check, finish, helpers, snapshot};
use crate::window::{self, TabId, WindowController};

const SCENARIO: &str = "panels";
const FILES: [&str; 4] = ["a.rs", "b.rs", "c.rs", "d.rs"];
const PROJECT: &str = "panels-project";
/// How long the terminal's login shell may take to start.
const SHELL_START: Duration = Duration::from_secs(30);
/// How long the window's processes may take to end once it closed.
const WIND_DOWN: Duration = Duration::from_secs(8);
/// A pause for the window to draw before a picture or a step that needs its layout, which has no state to wait for.
const PAUSE: Duration = Duration::from_millis(600);
/// The share of the left document that the tab of a.rs keeps.
const SHARE: f64 = 0.35;

/// The session of the AI tab, which comes back by its id.
const SESSION: &str = "51e1f7e5-0000-4000-8000-0000000000bb";

/// Opens an AI tab that plays the start of the session `session`.
pub(super) fn replay_agent(controller: &WindowController, session: &str) -> bool {
    let Some(tab) = ide_ui_agent::open_replay(controller) else {
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
    ide_ui_agent::of_window(controller)
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

fn tab_of(controller: &WindowController, root: &Path, name: &str) -> Option<TabId> {
    controller.tab_with_key(&root.join(name).to_string_lossy())
}

fn select(controller: &WindowController, tab: Option<TabId>) {
    if let Some(tab) = tab {
        controller.select_tab_by_id(tab);
    }
}

/// Whether the sidebar is in the window of the tab in front.
fn sidebar_in_front(controller: &WindowController) -> bool {
    match (controller.sidebar_window(), controller.window()) {
        (Some(sidebar), Some(front)) => std::ptr::eq(&*sidebar, &*front),
        _ => false,
    }
}

pub(super) fn run() {
    let texts: Vec<String> = FILES
        .iter()
        .map(|name| format!("// {name}\nfn main() {{}}\n"))
        .collect();
    let files: Vec<(&str, &str)> = FILES.iter().copied().zip(texts.iter().map(String::as_str)).collect();
    helpers::open_project(SCENARIO, PROJECT, &files, |_| opened(helpers::root(PROJECT)));
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
    TERMINAL_TAB.store(controller.selected_tab().map_or(-1, TabId::get), Ordering::SeqCst);
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
    helpers::wait_within(SHELL_START, || shell_now().is_some(), move |_| switching(root));
}

/// Bringing another tab to the front moves the sidebar along as it was.
fn switching(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let shell = shell_now();
    check("the terminal's shell starts", shell.is_some(), format!("{shell:?}"));
    SHELL.store(shell.unwrap_or(0), Ordering::SeqCst);
    let before = controller.inspect();
    let rows = controller.tree_rows();
    select(&controller, tab_of(&controller, &root, "a.rs"));
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
        .position(|docs| a.is_some_and(|a| docs.contains(&a)))
        .and_then(|index| ids.get((index + 1) % ids.len()))
        .and_then(|docs| docs.first().copied());
    controller.select_next_tab();
    check(
        "⇧⌘] brings the next tab to the front, with the sidebar",
        next.is_some() && controller.selected_tab() == next && sidebar_in_front(&controller),
        format!("{:?} after {ids:?}", controller.selected_tab()),
    );
    dragged_out(&controller, &root);
    select(&controller, tab_of(&controller, &root, "a.rs"));
    helpers::after(PAUSE, move || beside(root));
}

/// A tab moved to a window of its own keeps the sidebar's picture while the other window has the sidebar, gets the
/// sidebar back in front, and joins again with "Merge All Windows".
fn dragged_out(controller: &WindowController, root: &Path) {
    let c = tab_of(controller, root, "c.rs");
    select(controller, c);
    let Some(window) = controller.window() else { return };
    window.moveTabToNewWindow(None);
    let alone = window.tabbedWindows().is_none_or(|tabbed| tabbed.count() == 1);
    let a = tab_of(controller, root, "a.rs");
    select(controller, a);
    let pictures = controller.sidebar_pictures();
    // Only a window on screen beside the one in front needs the picture; the tabs behind the one in front show none.
    let order = controller.window_tab_ids();
    let picture_of = |id: Option<TabId>| {
        order
            .iter()
            .position(|docs| id.is_some_and(|id| docs.contains(&id)))
            .and_then(|at| pictures.get(at).copied())
    };
    let apart_ok = alone && sidebar_in_front(controller) && picture_of(c) == Some(true) && picture_of(a) == Some(false);
    select(controller, c);
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
    controller.open_file_beside(&root.join("d.rs"), crate::window::panes::Side::Right);
    let d = tab_of(&controller, &root, "d.rs");
    check(
        "a file opens beside the one in front, which takes the window's tabs into the panes and hides its tab bar (E145)",
        windows > 1
            && controller.tab_windows().len() == 1
            && controller.pane_bars().len() == 2
            && !controller.tab_bar_visible()
            && controller
                .window_tabs()
                .first()
                .is_some_and(|docs| docs.last().is_some_and(|last| last == "d.rs")),
        format!("{} {}", shown(&controller), controller.pane_shape()),
    );
    select(&controller, a);
    controller.focus_next_panel();
    check(
        "the focus moves on to the other document of the tab",
        d.is_some() && controller.selected_tab() == d,
        format!("{:?}", controller.selected_tab()),
    );
    if let Some(a) = a {
        controller.set_share_of(a, SHARE);
    }
    let alive = crate::terminal::inspect::view(TERMINAL.load(Ordering::SeqCst)).is_some_and(|view| !view.has_exited());
    check(
        "the terminal keeps its shell while other tabs come to the front",
        alive && shell_now() == Some(SHELL.load(Ordering::SeqCst)),
        format!("{:?}", shell_now()),
    );
    helpers::after(PAUSE, move || closing_side(root));
}

/// Closing one of two documents leaves the other alone in its tab; closing a tab's only one closes the tab.
fn closing_side(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "panels-beside");
    }
    if let Some(b) = tab_of(&controller, &root, "b.rs") {
        controller.close_tab(b);
    }
    check(
        "closing a document of a pane leaves the pane and the split",
        controller.tab_windows().len() == 1
            && tab_of(&controller, &root, "b.rs").is_none()
            && controller.pane_bars().len() == 2,
        format!("{} {}", shown(&controller), controller.pane_shape()),
    );
    if let Some(d) = tab_of(&controller, &root, "d.rs") {
        controller.close_tab(d);
    }
    check(
        "closing the other pane's only document gives every document a native tab of its own again",
        controller.window_tabs().iter().all(|docs| docs.len() == 1)
            && controller.tab_windows().len() == 3
            && controller.tab_bar_visible(),
        shown(&controller),
    );
    let a = tab_of(&controller, &root, "a.rs");
    select(&controller, a);
    controller.open_file_beside(&root.join("d.rs"), crate::window::panes::Side::Right);
    if let Some(a) = a {
        controller.set_share_of(a, SHARE);
    }
    let replayed = replay_agent(&controller, SESSION) && has_session(&controller, SESSION);
    check(
        "an AI thread opened in a split window joins the active pane, a.rs's (E145)",
        replayed
            && controller.window_tabs().len() == 1
            && tab_of(&controller, &root, "a.rs").and_then(|a| controller.pane_of(a))
                == controller.selected_tab().and_then(|agent| controller.pane_of(agent)),
        shown(&controller),
    );
    helpers::after(PAUSE, move || closing(root));
}

fn closing(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let expected = controller.window_tabs();
    let share = controller.tab_shares().into_iter().flatten().next();
    let clicked = controller.click_close_button();
    check("the window has a close button", clicked, "");
    let (probe, count) = (root.clone(), expected.len());
    let shell = SHELL.load(Ordering::SeqCst);
    helpers::wait_until(
        move || {
            Supervisor::global()
                .running()
                .iter()
                .all(|process| process.pid != shell)
                && ide_core::layout::load(&probe).is_some_and(|layout| layout.tabs.len() == count)
        },
        move |_| reopen(root, expected, share),
    );
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
        "closing the window with its close button saves all its tabs",
        saved.as_ref().is_some_and(|layout| {
            layout.tabs.len() == expected.len()
                && layout.has(&ide_core::layout::Tab::Agent {
                    session: SESSION.into(),
                })
                && layout.tabs.iter().any(
                    |tab| matches!(&tab.root, ide_core::layout::Node::Split { children, .. } if children.len() == 2),
                )
        }),
        format!("{saved:?}"),
    );
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(mtm, root.clone());
    let (probe, wanted) = (root.clone(), expected.clone());
    helpers::wait_until(
        move || helpers::window_of(&probe).is_some_and(|c| c.window_tabs() == wanted && has_session(&c, SESSION)),
        move |_| helpers::after(PAUSE, move || restored(root, expected, share)),
    );
}

fn restored(root: PathBuf, expected: Vec<Vec<String>>, share: Option<f64>) {
    let Some(controller) = window_of(&root) else { return };
    let tabs = controller.window_tabs();
    check(
        "the panes come back with their documents in their order when the folder opens again, d.rs beside the others",
        tabs == expected && tabs.len() == 1 && tabs[0].last().is_some_and(|last| last == "d.rs"),
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
    wind_down();
}

/// Finishes once the window's processes ended, its shell and any language server: a quit from inside a main-queue
/// block cannot wait for running processes, since the app's answer to the quit comes through that same queue.
fn wind_down() {
    helpers::wait_within(
        WIND_DOWN,
        || Supervisor::global().running().is_empty(),
        |ended| {
            let left: Vec<String> = Supervisor::global()
                .running()
                .iter()
                .map(|process| format!("{:?} {}", process.kind, process.label))
                .collect();
            check("closing the window ends its processes", ended, left.join(", "));
            finish(SCENARIO);
        },
    );
}
