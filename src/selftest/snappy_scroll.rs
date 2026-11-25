//! `snappy-scroll`: an editor of 10 000 lines with folds, an AI timeline of 10 000 cards and a folder of 5 000 entries
//! in the file tree are scrolled, each step in a frame of its own: smoothly, a twelfth of a screen a frame as a trackpad
//! does, and a screen at a time as Page Down does; each part reports its main thread time per frame and its missed
//! ticks against the budgets of docs/snappy.md, which a timed run holds them to.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::rc::Retained;
use objc2_app_kit::NSScrollView;
use objc2_foundation::{NSPoint, NSRange};

use super::snappy_input::steps;
use super::{check, finish, give_up, helpers};
use crate::editor::{self, selftest as edit};
use crate::window::{self, Panel, TickReading, WindowController};
use ide_ui_common::window::WindowId;

const NAME: &str = "snappy-scroll";
const PROJECT: &str = "snappy-scroll-project";
const FILE: &str = "long.txt";
/// A frame's main thread time while scrolling smoothly.
const FRAME_BUDGET: Duration = Duration::from_millis(6);
/// A page step builds a whole screen anew, so it has the budget of a finished content, not of a frame.
const PAGE_BUDGET: Duration = Duration::from_millis(50);
/// Blocks of six lines, 10 002 lines in all; every third folds.
const BLOCKS: usize = 1_667;
const LINES: usize = 6;
const CARDS: usize = 10_000;
const ENTRIES: isize = 5_000;
/// How a part scrolls: by how many screens a step, how many steps away from where it starts and back, and the budget.
#[derive(Clone, Copy)]
struct Pace {
    screens: f64,
    away: usize,
    back: usize,
    budget: Duration,
    what: &'static str,
}

/// What a part scrolls, and its checks for scrolling it smoothly and a page at a time.
type Part = (&'static str, [&'static str; 2]);

const EDITOR: Part = (
    "an editor of 10 000 lines with folds",
    [
        "scrolling an editor of 10 000 lines with folds smoothly stays within a frame",
        "scrolling an editor of 10 000 lines with folds a page at a time stays within its budget",
    ],
);
const TIMELINE: Part = (
    "an AI timeline of 10 000 cards",
    [
        "scrolling an AI timeline of 10 000 cards smoothly stays within a frame",
        "scrolling an AI timeline of 10 000 cards a page at a time stays within its budget",
    ],
);
const TREE: Part = (
    "a file tree of 5 000 entries",
    [
        "scrolling a file tree of 5 000 entries smoothly stays within a frame",
        "scrolling a file tree of 5 000 entries a page at a time stays within its budget",
    ],
);

const SMOOTH: Pace = Pace {
    screens: 1.0 / 12.0,
    away: 240,
    back: 80,
    budget: FRAME_BUDGET,
    what: "smoothly",
};
const PAGES: Pace = Pace {
    screens: 1.0,
    away: 40,
    back: 20,
    budget: PAGE_BUDGET,
    what: "a page at a time",
};
/// How long the app may settle after its setup before a part is measured.
const SETTLE: Duration = Duration::from_millis(1500);
/// How long a big folder may take to read on a busy Mac.
const LISTING: Duration = Duration::from_secs(20);

fn text() -> String {
    (0..BLOCKS)
        .map(|b| format!("block {b:04} {{\n    alpha {b:04} = 1;\n    beta {b:04} = 2;\n    gamma {b:04} = 3;\n}}\n\n"))
        .collect()
}

pub(crate) fn run() {
    let wide = helpers::folder(PROJECT).join("wide");
    let _ = std::fs::create_dir_all(&wide);
    for n in 0..ENTRIES {
        let _ = std::fs::write(wide.join(format!("file{n}.txt")), "");
    }
    let text = text();
    helpers::open_project(NAME, PROJECT, &[(FILE, text.as_str())], |controller| {
        let root = helpers::root(PROJECT);
        controller.open_file(&root.join(FILE));
        let probe = root.clone();
        helpers::wait_until(
            move || view(&probe).is_some_and(|v| v.line_start(BLOCKS * LINES - 1).is_some()),
            move |opened| {
                check("a file of 10 000 lines opens", opened, "");
                fold(&root);
                helpers::after(SETTLE, move || scroll_editor(root));
            },
        );
    });
}

fn view(root: &Path) -> Option<Retained<editor::EditorTextView>> {
    let controller = window::for_project(root)?;
    editor::editor_for(&controller, &root.join(FILE))
}

fn fold(root: &Path) {
    let Some(view) = view(root) else { return };
    for block in (0..BLOCKS).step_by(3) {
        if let Some(at) = view.line_start(block * LINES) {
            view.setSelectedRange(NSRange::new(at, 0));
            edit::fold_at_cursor(&view);
        }
    }
    view.setSelectedRange(NSRange::new(0, 0));
    check(
        "every third block of the long file folds",
        edit::folded(&view) == BLOCKS.div_ceil(3),
        format!("{} folded", edit::folded(&view)),
    );
}

fn scroll_editor(root: PathBuf) {
    let (Some(controller), Some(scroll)) = (
        window::for_project(&root),
        view(&root).and_then(|v| v.enclosingScrollView()),
    ) else {
        return give_up(NAME, "the long file's editor is open");
    };
    both(&controller, scroll, 1.0, EDITOR, move || timeline(root));
}

fn timeline(root: PathBuf) {
    let Some(controller) = window::for_project(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let Some(tab) = ide_ui_agent::open_replay(&*controller) else {
        return give_up(NAME, "an AI tab plays a recording");
    };
    tab.feed(&super::agent_perf::history(CARDS));
    tab.select();
    check(
        "the AI tab shows 10 000 cards",
        tab.timeline().count() >= CARDS,
        format!("{} rows", tab.timeline().count()),
    );
    helpers::after(SETTLE, move || {
        let Some(controller) = window::for_project(&root) else {
            return give_up(NAME, "the folder's window is open");
        };
        let scroll = objc2::Message::retain(tab.timeline().view());
        // The timeline starts at its end, so it scrolls up first.
        both(&controller, scroll, -1.0, TIMELINE, move || {
            drop(tab);
            tree(root);
        });
    });
}

fn tree_rows(root: &Path) -> isize {
    window::for_project(root).map_or(0, |c| c.tree_rows())
}

fn tree(root: PathBuf) {
    let Some(controller) = window::for_project(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    controller.activate_panel(Panel::Files);
    let probe = root.clone();
    helpers::wait_within(
        LISTING,
        move || tree_rows(&probe) >= 2,
        move |_| {
            if let Some(controller) = window::for_project(&root) {
                controller.click_in_tree(&root.join("wide"), false);
            }
            let probe = root.clone();
            helpers::wait_within(
                LISTING,
                move || tree_rows(&probe) > ENTRIES,
                move |listed| {
                    check(
                        "a folder of 5 000 entries opens in the tree",
                        listed,
                        format!("{} rows", tree_rows(&root)),
                    );
                    helpers::after(SETTLE, move || scroll_tree(root));
                },
            );
        },
    );
}

fn scroll_tree(root: PathBuf) {
    let Some(controller) = window::for_project(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let Some(scroll) = controller.file_tree_outline().and_then(|o| o.enclosingScrollView()) else {
        return give_up(NAME, "the file tree shows");
    };
    both(&controller, scroll, 1.0, TREE, || finish(NAME));
}

/// Scrolls `scroll` of `controller`'s window smoothly, then a page at a time, and reports both for `what`; then `then`.
fn both(
    controller: &WindowController,
    scroll: Retained<NSScrollView>,
    direction: f64,
    (what, checks): Part,
    then: impl FnOnce() + 'static,
) {
    let window = controller.id();
    let again = Retained::clone(&scroll);
    scroll_through(controller, scroll, direction, SMOOTH, move |times, ticks| {
        report(SMOOTH, what, checks[0], &times, ticks);
        let Some(controller) = crate::host::window(window) else {
            return give_up(NAME, "the folder's window is open");
        };
        scroll_through(&controller, again, direction, PAGES, move |times, ticks| {
            report(PAGES, what, checks[1], &times, ticks);
            then();
        });
    });
}

/// Scrolls `scroll` of `controller`'s window `pace.away` steps in `direction`, then `pace.back` steps back, a step a
/// frame with each step marked for missed ticks; then `then` with each frame's time and the ticks.
fn scroll_through(
    controller: &WindowController,
    scroll: Retained<NSScrollView>,
    direction: f64,
    pace: Pace,
    then: impl FnOnce(Vec<Duration>, TickReading) + 'static,
) {
    let window = controller.id();
    let _ = controller.take_ticks();
    steps(
        pace.away + pace.back,
        move |at| {
            if let Some(controller) = crate::host::window(window) {
                controller.mark_action();
            }
            let screens = pace.screens * if at < pace.away { direction } else { -direction };
            scroll_by(&scroll, screens);
        },
        move |times| then(times, ticks_of(window)),
    );
}

fn ticks_of(window: WindowId) -> TickReading {
    crate::host::window(window).map_or(TickReading::Unavailable, |c| c.take_ticks())
}

/// Moves what `scroll` shows by `screens` of its height, as far as its document goes.
fn scroll_by(scroll: &NSScrollView, screens: f64) {
    let clip = scroll.contentView();
    let bounds = clip.bounds();
    let height = scroll.documentView().map_or(0.0, |d| d.frame().size.height);
    let end = (height - bounds.size.height).max(0.0);
    let y = (bounds.origin.y + screens * bounds.size.height).clamp(0.0, end);
    clip.scrollToPoint(NSPoint::new(bounds.origin.x, y));
    scroll.reflectScrolledClipView(&clip);
}

/// The `percent` percentile of `sorted`.
fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    let index = (sorted.len().saturating_sub(1) * percent + 50) / 100;
    sorted.get(index).copied().unwrap_or_default()
}

/// Records the frames' times of scrolling `what` at `pace` and reports them under `check_name`, with the ticks.
fn report(pace: Pace, what: &str, check_name: &'static str, times: &[Duration], ticks: TickReading) {
    let name = format!("snappy-scroll: {what}, {}", pace.what);
    let mut sorted = times.to_vec();
    sorted.sort();
    let (p50, p95, max) = (
        percentile(&sorted, 50),
        percentile(&sorted, 95),
        percentile(&sorted, 100),
    );
    super::snappy::record(&format!("{name}, frame"), &sorted);
    super::snappy::record(&format!("{name}, frame p95"), &[p95]);
    super::snappy::record(&format!("{name}, frame max"), &[max]);
    record_ticks(&name, ticks);
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    check(
        check_name,
        sorted.len() == pace.away + pace.back && (p95 <= pace.budget || !super::snappy::timed_run()),
        format!(
            "{} frames: p50 {:.1} ms, p95 {:.1} ms, longest {:.1} ms ({} the budget of {} ms{}); {}",
            sorted.len(),
            ms(p50),
            ms(p95),
            ms(max),
            if p95 <= pace.budget { "within" } else { "over" },
            pace.budget.as_millis(),
            if super::snappy::timed_run() {
                ""
            } else {
                ", which only a timed run holds to"
            },
            said(ticks),
        ),
    );
}

/// Records the missed ticks of `ticks` as `name`, in a timed run and when the display link ticked.
pub(super) fn record_ticks(name: &str, ticks: TickReading) {
    if let TickReading::Counted { missed, .. } = ticks
        && super::snappy::timed_run()
    {
        ide_core::perf::value(&format!("{name}, missed ticks"), "ticks", true, missed as f64);
    }
}

/// `ticks` for a check's detail.
pub(super) fn said(ticks: TickReading) -> String {
    match ticks {
        TickReading::Counted { actions, missed } => format!("{missed} of {actions} steps missed their tick"),
        TickReading::Unavailable => "missed ticks unavailable: the display link did not tick for every step".into(),
    }
}
