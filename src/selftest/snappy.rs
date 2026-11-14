//! The `snappy-*` scenarios: the inputs that must feel instant while the app works, each measured with the stall
//! detector, which no main thread step of the measured part may reach.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_ui_common::util::stall;

use super::{check, finish, helpers};
use crate::window::{self, Panel};

/// The longest a main thread step of a measured part may take.
const BUDGET: Duration = Duration::from_millis(50);
/// How long a big folder may take to read on a busy Mac.
const LISTING: Duration = Duration::from_secs(20);

/// Starts a measured part.
fn measure() {
    let _ = stall::take_worst();
}

/// Checks that no main thread step since `measure` held up input.
fn measured(name: &'static str, what: &str) {
    let worst = stall::take_worst();
    check(
        name,
        worst < BUDGET,
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
pub fn tree() {
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
        return finish(TREE);
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
