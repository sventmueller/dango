//! The `panes` scenario (E145): splitting takes the window's tabs into panes, right and below, nested, each with a bar of
//! its own in place of the window's; a file opens in a native tab of its own while the window is not split and in the
//! active pane once it is; tabs move between panes and to a pane's edge; a pane fills the window and closes; the
//! commands of a pane's bar and of the menu and the tree's Option click and pane menu act on panes; a terminal
//! and an AI thread keep running behind another document; the panes come back when the folder opens again; and the
//! last pane but one closing gives the documents native tabs again.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use splat_i18n::{t, tf};

use super::{check, finish, give_up_saying, helpers, snapshot};
use crate::window::panes::Side;
use crate::window::{TabId, WindowController};

const SCENARIO: &str = "panes";
const PROJECT: &str = "panes-project";
const FILES: [&str; 5] = ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"];
/// A pause for the window to draw before a picture or a step that needs its layout, which has no state to wait for.
const PAUSE: Duration = Duration::from_millis(600);
/// How long a terminal and an AI thread behind another document are left alone before they are asked whether they
/// still run.
const BEHIND: Duration = Duration::from_millis(1500);
const SESSION: &str = "51e1f7e5-0000-4000-8000-0000000000cc";

fn window_of(root: &Path) -> Option<Retained<WindowController>> {
    let controller = helpers::window_of(root);
    if controller.is_none() {
        give_up_saying(SCENARIO, "the folder gets its window", root.display().to_string());
    }
    controller
}

fn tab_of(controller: &WindowController, root: &Path, name: &str) -> Option<TabId> {
    controller.tab_with_key(&root.join(name).to_string_lossy())
}

fn select(controller: &WindowController, id: Option<TabId>) {
    if let Some(id) = id {
        controller.select_tab_by_id(id);
    }
}

fn lay_out(controller: &WindowController) {
    if let Some(content) = controller.window().and_then(|window| window.contentView()) {
        content.layoutSubtreeIfNeeded();
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

/// Splitting right takes the window's tabs into the first pane, hides the window's tab bar and gives every pane a
/// bar of its own; the empty new pane is the active one, which the next file fills.
fn opened(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let welcome = controller.selected_tab();
    controller.open_file(&root.join("a.rs"));
    if let Some(welcome) = welcome {
        controller.close_tab(welcome);
    }
    controller.open_file(&root.join("b.rs"));
    check(
        "a file opens in a native tab of its own while the window is not split",
        controller.window_tabs() == [vec!["a.rs".to_string()], vec!["b.rs".to_string()]]
            && controller.tab_bar_visible(),
        format!("{:?}", controller.window_tabs()),
    );
    select(&controller, tab_of(&controller, &root, "a.rs"));
    controller.split_pane(Side::Right);
    lay_out(&controller);
    let empty = controller.pane_shape();
    check(
        "splitting takes the window's tabs into the first pane, hides the window's tab bar and shows every pane's bar",
        empty == "H([a.rs*, b.rs] []!)"
            && controller.tab_windows().len() == 1
            && !controller.tab_bar_visible()
            && controller.pane_bars().iter().all(|(bar, _)| *bar),
        format!(
            "{empty} {:?}, bar {}",
            controller.pane_bars(),
            controller.tab_bar_visible()
        ),
    );
    controller.open_file(&root.join("c.rs"));
    controller.open_file(&root.join("d.rs"));
    lay_out(&controller);
    check(
        "a file opened in a split window lands in the active pane, also the empty one",
        controller.pane_shape() == "H([a.rs*, b.rs] [c.rs, d.rs*]!)" && controller.tab_windows().len() == 1,
        controller.pane_shape(),
    );
    helpers::after(PAUSE, move || below(root));
}

/// Splitting a pane of two below moves its document in front into the new pane; a file open elsewhere comes to the
/// front where it is; the Option-Shift menu names every pane.
fn below(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "panes-bars");
    }
    controller.split_pane(Side::Below);
    lay_out(&controller);
    check(
        "splitting below moves the document in front into a new pane under it, nested in the split",
        controller.pane_shape() == "H([a.rs*, b.rs] V([c.rs] [d.rs]!))",
        controller.pane_shape(),
    );
    let b = tab_of(&controller, &root, "b.rs");
    controller.open_file(&root.join("b.rs"));
    check(
        "a file already open comes to the front of its pane",
        b.is_some()
            && controller.selected_tab() == b
            && controller.pane_shape() == "H([a.rs, b.rs*]! V([c.rs] [d.rs]))",
        format!("{:?} in {}", controller.selected_tab(), controller.pane_shape()),
    );
    select(&controller, tab_of(&controller, &root, "d.rs"));
    let menu = controller.file_pane_menu();
    let first = tf("Im Bereich {}: {}", &["1", "b.rs"]);
    check(
        "Option-Shift on a file offers each pane by its document, beside and below",
        menu.len() == 5 && menu.first() == Some(&first),
        format!("{menu:?}"),
    );
    helpers::after(PAUSE, move || dragging(root));
}

/// A tab dropped into another pane moves there and the pane it leaves goes; dropped at a pane's right edge it gets a
/// pane of its own there.
fn dragging(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "panes-split");
    }
    let (a, c, d) = (
        tab_of(&controller, &root, "a.rs"),
        tab_of(&controller, &root, "c.rs"),
        tab_of(&controller, &root, "d.rs"),
    );
    let (Some(a), Some(c), Some(d)) = (a, c, d) else {
        return check("the documents are open", false, controller.pane_shape());
    };
    if let Some(into) = controller.pane_of(a) {
        controller.drop_tab_on_pane(c, into, 0.5, 0.5);
    }
    lay_out(&controller);
    let moved = controller.pane_shape();
    if let Some(edge) = controller.pane_of(d) {
        controller.drop_tab_on_pane(c, edge, 0.92, 0.5);
    }
    lay_out(&controller);
    check(
        "a tab dropped into another pane moves there, the pane it leaves goes; at a pane's right edge it gets its own",
        moved == "H([a.rs, b.rs, c.rs*]! [d.rs])" && controller.pane_shape() == "H([a.rs, b.rs*] [d.rs] [c.rs]!)",
        format!("{moved} then {}", controller.pane_shape()),
    );
    controller.zoom_pane();
    lay_out(&controller);
    let zoomed = controller.pane_bars();
    controller.zoom_pane();
    lay_out(&controller);
    check(
        "a pane fills the tab alone and gives the others their place back",
        zoomed.iter().map(|(_, shown)| *shown).collect::<Vec<_>>() == [false, false, true]
            && controller.pane_bars().iter().all(|(_, shown)| *shown),
        format!("{zoomed:?} then {:?}", controller.pane_bars()),
    );
    controller.close_pane();
    let closed = controller.pane_shape();
    if let Some(c) = tab_of(&controller, &root, "c.rs") {
        controller.close_tab(c);
    }
    check(
        "closing a pane gives its room and its documents to the one before, which keeps its document in front",
        closed == "H([a.rs, b.rs*] [d.rs*, c.rs]!)",
        closed,
    );
    commands(root, a);
}

/// The commands of a pane's bar and of the menu: what the menu offers, a double click on a tab, the next document
/// of a pane, a document split off into a pane of its own, and an empty pane closed by its number.
fn commands(root: PathBuf, a: TabId) {
    let Some(controller) = window_of(&root) else { return };
    let (Some(b), Some(d)) = (tab_of(&controller, &root, "b.rs"), tab_of(&controller, &root, "d.rs")) else {
        return check("the documents are open", false, controller.pane_shape());
    };
    let state = controller.pane_menu_state();
    let holds = (controller.pane_has_more(a), controller.pane_has_more(d));
    check(
        "the menu sees two panes, the active one with one document and not filling the tab; only the first holds more",
        state == Some((2, 1, false)) && holds == (true, false),
        format!("{state:?}, {holds:?}"),
    );
    controller.click_pane_tab(a, 2);
    let clicked = controller.pane_shape();
    controller.step_pane_tab(1);
    check(
        "a click on a pane's tab brings its document and pane to the front, and the menu steps to the next document",
        clicked == "H([a.rs*, b.rs]! [d.rs])"
            && controller.pane_shape() == "H([a.rs, b.rs*]! [d.rs])"
            && controller.selected_tab() == Some(b),
        format!("{clicked} then {}", controller.pane_shape()),
    );
    controller.split_off_pane(b, Side::Below);
    lay_out(&controller);
    let split = (
        controller.pane_menu_state(),
        controller.pane_of(a),
        controller.pane_of(b),
    );
    if let Some(into) = controller.pane_of(a) {
        controller.drop_tab_on_pane(b, into, 0.5, 0.5);
    }
    lay_out(&controller);
    check(
        "a document split off gets a pane of its own below and goes back by a drop",
        split.0.map(|(panes, _, _)| panes) == Some(3)
            && split.1.zip(split.2).is_some_and(|(a, b)| a != b)
            && controller.pane_shape() == "H([a.rs, b.rs*]! [d.rs])",
        format!("{split:?}, then {}", controller.pane_shape()),
    );
    controller.select_tab_by_id(d);
    controller.split_pane(Side::Right);
    let empty = controller.pane_choices().into_iter().find(|(_, front)| front.is_none());
    let with_empty = controller.pane_shape();
    if let Some((number, _)) = empty {
        controller.close_pane_numbered(number);
    }
    lay_out(&controller);
    check(
        "an empty pane closes by its number and gives its room back",
        with_empty == "H([a.rs, b.rs*] [d.rs] []!)"
            && controller.pane_shape().replace('!', "") == "H([a.rs, b.rs*] [d.rs])",
        format!("{with_empty} then {}", controller.pane_shape()),
    );
    from_tree(root, a, d);
}

/// The tree's Option click opens a file beside the document in front, its pane menu opens one in the pane picked, and
/// closing the other documents of a pane leaves the one asked for.
fn from_tree(root: PathBuf, a: TabId, d: TabId) {
    let Some(controller) = window_of(&root) else { return };
    let beside = controller.option_click_file(&root.join("c.rs"));
    let missing = controller.option_click_file(&root.join("missing.rs"));
    lay_out(&controller);
    let c = tab_of(&controller, &root, "c.rs");
    let own = c.and_then(|c| controller.pane_of(c));
    check(
        "an Option click on a file in the tree opens it in a pane of its own beside, on no row it does nothing",
        beside
            && !missing
            && own.is_some()
            && own != controller.pane_of(a)
            && own != controller.pane_of(d)
            && controller.pane_menu_state().map(|(panes, _, _)| panes) == Some(3),
        format!("{beside} {missing} {}", controller.pane_shape()),
    );
    if let Some(number) = controller.pane_of(d) {
        controller.choose_pane_for_file(&root.join("e.rs"), number);
    }
    let e = tab_of(&controller, &root, "e.rs");
    let picked = e.and_then(|e| controller.pane_of(e));
    controller.close_others_in_pane(d);
    check(
        "a file picked from the tree's pane menu opens in that pane, and closing the others of the pane closes it again",
        picked.is_some() && picked == controller.pane_of(d) && tab_of(&controller, &root, "e.rs").is_none(),
        format!("{picked:?} {}", controller.pane_shape()),
    );
    select(&controller, c);
    controller.close_pane();
    lay_out(&controller);
    let heir = c.and_then(|c| controller.pane_of(c));
    if let Some(c) = c {
        controller.close_tab(c);
    }
    lay_out(&controller);
    check(
        "closing the opened pane hands its document to a pane beside, and a split without Option goes right",
        heir.is_some()
            && (heir == controller.pane_of(a) || heir == controller.pane_of(d))
            && controller.pane_shape().replace('!', "") == "H([a.rs, b.rs*] [d.rs])"
            && controller.split_side() == Side::Right,
        format!("{heir:?} {}", controller.pane_shape()),
    );
    behind(root, a);
}

/// A terminal and an AI thread opened into a pane keep running while another document of the pane is in front.
fn behind(root: PathBuf, a: TabId) {
    let Some(controller) = window_of(&root) else { return };
    controller.select_tab_by_id(a);
    crate::terminal::open_tab(&controller, None);
    let terminal = controller.selected_tab();
    let shell = crate::terminal::inspect::latest().map(|(id, _)| id);
    let replayed = super::panels::replay_agent(&controller, SESSION);
    let agent = controller.selected_tab();
    controller.select_tab_by_id(a);
    let shape = controller.pane_shape();
    helpers::after(BEHIND, move || {
        let Some(controller) = window_of(&root) else { return };
        let ids = controller.tab_ids();
        let alive = shell
            .and_then(crate::terminal::inspect::view)
            .is_some_and(|view| !view.has_exited());
        let session = ide_ui_agent::of_window(&controller)
            .iter()
            .any(|tab| tab.session_id().as_deref() == Some(SESSION));
        check(
            "a terminal and an AI thread behind another document of their pane keep running",
            replayed
                && shape.starts_with("H([a.rs*, ")
                && terminal
                    .zip(agent)
                    .is_some_and(|(t, g)| ids.contains(&t) && ids.contains(&g))
                && alive
                && session,
            format!("{shape}, alive {alive}, session {session}"),
        );
        for id in [terminal, agent].into_iter().flatten() {
            controller.close_tab(id);
        }
        saving(root);
    });
}

/// The panes, nested, with their shares, come back when the folder opens again.
fn saving(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    select(&controller, tab_of(&controller, &root, "d.rs"));
    controller.open_file_beside(&root.join("e.rs"), Side::Below);
    if let Some(a) = tab_of(&controller, &root, "a.rs") {
        controller.set_share_of(a, 0.4);
    }
    lay_out(&controller);
    let shape = controller.pane_shape().replace('!', "");
    let share = controller.tab_shares().into_iter().flatten().next();
    controller.close_all_tabs();
    let probe = root.clone();
    helpers::wait_until(
        move || {
            helpers::window_of(&probe).is_none()
                && ide_core::layout::load(&probe).is_some_and(|layout| layout.document_count() == 4)
        },
        move |_| {
            let Some(mtm) = MainThreadMarker::new() else { return };
            crate::app::open_project(&crate::context::app(), mtm, root.clone());
            let probe = root.clone();
            helpers::wait_until(
                move || {
                    helpers::window_of(&probe).is_some_and(|c| c.tab_windows().len() == 1 && c.tab_ids().len() == 4)
                },
                move |_| helpers::after(PAUSE, move || restored(root, shape, share)),
            );
        },
    );
}

fn restored(root: PathBuf, shape: String, share: Option<f64>) {
    let Some(controller) = window_of(&root) else { return };
    select(&controller, tab_of(&controller, &root, "a.rs"));
    lay_out(&controller);
    let now = controller.pane_shape().replace('!', "");
    let share_now = controller.tab_shares().into_iter().flatten().next();
    check(
        "the panes come back nested as they were, with their shares, when the folder opens again",
        shape.starts_with("H([a.rs*, b.rs] V([d.rs] [e.rs]")
            && now == shape
            && share
                .zip(share_now)
                .is_some_and(|(before, now)| (before - now).abs() < 0.05),
        format!("{shape} {share:.2?} then {now} {share_now:.2?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "panes-restored");
    }
    for name in ["e.rs", "d.rs"] {
        if let Some(id) = tab_of(&controller, &root, name) {
            controller.close_tab(id);
        }
    }
    check(
        "closing the last pane but one gives every document a native tab of its own again, with the window's bar",
        controller.window_tabs() == [vec!["a.rs".to_string()], vec!["b.rs".to_string()]]
            && controller.tab_bar_visible(),
        format!("{:?}, bar {}", controller.window_tabs(), controller.tab_bar_visible()),
    );
    let menu = controller.file_pane_menu();
    check(
        "without panes, Option-Shift on a file offers beside, below and a window tab of its own",
        menu == [t("Daneben öffnen"), t("Darunter öffnen"), t("In neuem Tab öffnen")],
        format!("{menu:?}"),
    );
    finish(SCENARIO);
}
