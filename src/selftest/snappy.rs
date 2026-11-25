//! The `snappy-*` scenarios: the inputs that must feel instant while the app works, each measured with the stall
//! detector, which no main thread step of the measured part may reach.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_ui_common::util::stall;

use super::{check, finish, give_up, helpers};
use crate::window::{self, Panel};

/// The longest a main thread step of a measured part may take in debug and coverage runs.
const BUDGET: Duration = Duration::from_millis(50);
/// The same in a timed run: a frame, E197's bound for any main thread step.
const TIMED_BUDGET: Duration = Duration::from_millis(16);
/// How long a big folder may take to read on a busy Mac.
const LISTING: Duration = Duration::from_secs(20);

/// Whether this is a release self-test without coverage instrumentation, the only run whose timings count.
pub(crate) fn timed_run() -> bool {
    // The coverage run sets the variable for the profiles of its instrumented build.
    !cfg!(debug_assertions) && std::env::var_os("LLVM_PROFILE_FILE").is_none()
}

/// The longest a main thread step of a measured part may take in this run.
pub(super) fn budget() -> Duration {
    if timed_run() { TIMED_BUDGET } else { BUDGET }
}

/// A jump that lays out and draws a whole new screen in one step, such as one to the end of a long text: E197's bound
/// for content that is complete, not a frame.
pub(super) const JUMP_BUDGET: Duration = Duration::from_millis(50);

/// The same for a part whose steps AppKit itself holds above a frame, measured in docs/snappy.md: switching native
/// window tabs (`NSWindowTabGroup`), dragging split views, and TextKit 2 shifting every cached paragraph after an edit
/// near the top of a long text; the perf history keeps their measured values from rising.
pub(super) fn appkit_budget() -> Duration {
    BUDGET
}

/// The main thread step from which the stall detector reports in this run.
pub(crate) fn stall_threshold() -> Duration {
    if timed_run() { stall::FRAME } else { stall::STALL }
}

/// Records the median of `samples` as `name` for `cargo xtask perf-compare`, in a timed run only.
pub(super) fn record(name: &str, samples: &[Duration]) {
    if timed_run() {
        ide_core::perf::durations(name, samples);
    }
}

/// Starts a measured part.
pub(super) fn measure() {
    let _ = stall::take_worst();
}

/// Checks that no main thread step since `measure` held up input.
pub(super) fn measured(name: &'static str, what: &str) {
    measured_within(name, what, budget());
}

/// Checks that no main thread step since `measure` took `bound` or longer.
pub(super) fn measured_within(name: &'static str, what: &str, bound: Duration) {
    let worst = stall::take_worst();
    record(name, &[worst]);
    check(
        name,
        worst < bound,
        format!("{what}: the longest step took {} ms", worst.as_millis()),
    );
}

fn tree_rows(root: &Path) -> isize {
    window::for_project(root).map_or(0, |c| c.tree_rows())
}

const TREE: &str = "snappy-tree";
const TREE_PROJECT: &str = "snappy-tree-project";
/// The entries of the wide folder.
const WIDE: isize = 5_000;

/// `snappy-tree`: a folder of 5 000 entries opens, scrolls to its end and closes.
pub(crate) fn tree() {
    let wide = helpers::folder(TREE_PROJECT).join("wide");
    let _ = std::fs::create_dir_all(&wide);
    for n in 0..WIDE {
        let _ = std::fs::write(wide.join(format!("file{n}.txt")), "");
    }
    helpers::open_project(TREE, TREE_PROJECT, &[("README.md", "# Wide\n")], |controller| {
        controller.activate_panel(Panel::Files);
        let root = helpers::root(TREE_PROJECT);
        let probe = root.clone();
        helpers::wait_within(LISTING, move || tree_rows(&probe) >= 2, move |_| tree_opens(root));
    });
}

fn tree_opens(root: PathBuf) {
    let Some(controller) = window::for_project(&root) else {
        return give_up(TREE, "the folder's window is open");
    };
    measure();
    controller.click_in_tree(&root.join("wide"), false);
    let probe = root.clone();
    helpers::wait_within(
        LISTING,
        move || tree_rows(&probe) > WIDE,
        move |listed| {
            let rows = tree_rows(&root);
            check("a folder of 5 000 entries opens", listed, format!("{rows} rows"));
            if let Some(controller) = window::for_project(&root) {
                if let Some(outline) = controller.file_tree_outline() {
                    outline.scrollRowToVisible(rows - 1);
                    outline.scrollRowToVisible(0);
                }
                if let Some(window) = controller.window() {
                    window.displayIfNeeded();
                }
                controller.click_in_tree(&root.join("wide"), false);
            }
            check(
                "the folder closes again",
                tree_rows(&root) == 2,
                format!("{} rows", tree_rows(&root)),
            );
            measured(
                "opening, scrolling and closing a folder of 5 000 entries never holds up input",
                "the wide folder",
            );
            finish(TREE);
        },
    );
}
