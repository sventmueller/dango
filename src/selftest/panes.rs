//! The `panes` scenario (E145): a window tab splits into panes right and below, nested; a file opens in a native tab of
//! its own while the tab in front is not split and in the active pane once it is; the panes show tab bars only while
//! one holds more than one document; tabs move between panes and to a pane's edge; a pane fills the tab and closes;
//! a terminal and an AI thread keep running behind another document; and the panes come back when the folder opens
//! again.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use splat_i18n::tf;

use super::{check, finish, helpers, snapshot};
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
        check("the folder gets its window", false, root.display().to_string());
        finish(SCENARIO);
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

/// Splitting right gives an empty active pane, which the next file fills; a second file in a pane brings the bars.
fn opened(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let welcome = controller.selected_tab();
    controller.open_file(&root.join("a.rs"));
    if let Some(welcome) = welcome {
        controller.close_tab(welcome);
    }
    controller.open_file(&root.join("b.rs"));
    check(
        "a file opens in a native tab of its own while the tab in front is not split",
        controller.window_tabs() == [vec!["a.rs".to_string()], vec!["b.rs".to_string()]],
        format!("{:?}", controller.window_tabs()),
    );
    select(&controller, tab_of(&controller, &root, "a.rs"));
    controller.split_pane(Side::Right);
    let empty = controller.pane_shape();
    controller.open_file(&root.join("c.rs"));
    lay_out(&controller);
    check(
        "splitting right gives an empty active pane, where the next file opens instead of in a tab of its own",
        empty == "H([a.rs] []!)"
            && controller.pane_shape() == "H([a.rs] [c.rs]!)"
            && controller.tab_windows().len() == 2,
        format!("{empty} then {}", controller.pane_shape()),
    );
    let badges = tab_of(&controller, &root, "c.rs")
        .map(|c| controller.badges_of(c))
        .unwrap_or_default();
    let no_bars = controller.pane_bars().iter().all(|(bar, _)| !bar);
    check(
        "while every pane holds one document there are no tab bars, and the second pane has its badge",
        no_bars
            && badges.len() == 2
            && badges[0].is_none()
            && badges[1].as_ref().is_some_and(|(name, _)| name == "c.rs"),
        format!("{:?} {badges:?}", controller.pane_bars()),
    );
    controller.open_file(&root.join("d.rs"));
    lay_out(&controller);
    let badges = tab_of(&controller, &root, "d.rs")
        .map(|d| controller.badges_of(d))
        .unwrap_or_default();
    check(
        "a second document in a pane brings the tab bars of all panes, and the badges go",
        controller.pane_shape() == "H([a.rs] [c.rs, d.rs*]!)"
            && controller.pane_bars().iter().all(|(bar, _)| *bar)
            && badges.iter().all(Option::is_none),
        format!("{} {:?} {badges:?}", controller.pane_shape(), controller.pane_bars()),
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
        controller.pane_shape() == "H([a.rs] V([c.rs] [d.rs]!))" && controller.pane_bars().iter().all(|(bar, _)| !bar),
        format!("{} {:?}", controller.pane_shape(), controller.pane_bars()),
    );
    let b = tab_of(&controller, &root, "b.rs");
    controller.open_file(&root.join("b.rs"));
    check(
        "a file already open comes to the front where it is, also from a split tab",
        b.is_some() && controller.selected_tab() == b && controller.tab_windows().len() == 2,
        format!("{:?} of {:?}", controller.selected_tab(), controller.window_tabs()),
    );
    select(&controller, tab_of(&controller, &root, "d.rs"));
    let menu = controller.file_pane_menu();
    let first = tf("Im Bereich {}: {}", &["1", "a.rs"]);
    check(
        "Option-Shift on a file offers each pane by its document, beside, below and a tab of its own",
        menu.len() == 6 && menu.first() == Some(&first),
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
        moved == "H([a.rs, c.rs*]! [d.rs])" && controller.pane_shape() == "H([a.rs] [d.rs] [c.rs]!)",
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
    check(
        "closing a pane gives its room to the one before",
        controller.pane_shape() == "H([a.rs] [d.rs]!)" && tab_of(&controller, &root, "c.rs").is_none(),
        controller.pane_shape(),
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
            crate::app::open_project(mtm, root.clone());
            let probe = root.clone();
            helpers::wait_until(
                move || helpers::window_of(&probe).is_some_and(|c| c.tab_windows().len() == 2),
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
        shape == "H([a.rs] V([d.rs] [e.rs]))"
            && now == shape
            && share
                .zip(share_now)
                .is_some_and(|(before, now)| (before - now).abs() < 0.05),
        format!("{shape} {share:.2?} then {now} {share_now:.2?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "panes-restored");
    }
    finish(SCENARIO);
}
