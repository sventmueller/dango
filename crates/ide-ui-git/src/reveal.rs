//! A commit shown from outside the panel, as blame's popover asks (E86): its row in the history (E74), read further
//! back page by page when it is older than what the panel shows, and what it changed in one file.

use std::path::Path;
use std::time::{Duration, Instant};

use ide_git::CommitId;
use objc2_foundation::NSIndexSet;

use super::model::{RowKey, Section};
use super::panel::{self, GitPanel, with_panel};
use ide_ui_common::util::main_after;
use ide_ui_common::window::{AsWindow, Panel, Panels};

/// How many further pages a reveal reads before it gives up on a commit.
const MAX_PAGES: usize = 20;
/// How long one page may take to read.
const PAGE_WAIT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(150);

/// Opens the Git panel of `controller` with the commit `hash` selected in its history.
pub fn reveal_commit(controller: &(impl AsWindow + ?Sized), hash: &CommitId) {
    let controller = &controller.as_window();
    controller.activate_panel(Panel::Git);
    if let Some(panel) = panel::of(controller) {
        reveal(&panel, hash.clone(), 0);
    }
}

/// Shows what the commit `hash` changed in `path`, relative to the repository's root `repo_root`, in a diff tab.
pub fn show_commit_diff(controller: &(impl AsWindow + ?Sized), repo_root: &Path, hash: &CommitId, path: &Path) {
    let controller = &controller.as_window();
    super::diff::show_commit(controller, repo_root, hash, path, false);
}

fn reveal(panel: &GitPanel, hash: CommitId, pages: usize) {
    if panel.read().model.commit(&hash).is_some() {
        select(panel, &hash);
        return;
    }
    let shown = panel.read().model.commits.len();
    if pages >= MAX_PAGES || shown < panel.history_limit() && pages > 0 {
        objc2_app_kit::NSBeep();
        return;
    }
    panel.read_more();
    wait(panel.id(), hash, pages + 1, shown, Instant::now() + PAGE_WAIT);
}

/// Waits for the next page of history to arrive, then looks for the commit again.
fn wait(id: u64, hash: CommitId, pages: usize, before: usize, until: Instant) {
    main_after(POLL, move || {
        with_panel(id, |panel| {
            if panel.read().model.commits.len() > before || Instant::now() > until {
                reveal(panel, hash, pages);
            } else {
                wait(id, hash, pages, before, until);
            }
        });
    });
}

fn select(panel: &GitPanel, hash: &CommitId) {
    panel.state().collapsed.remove(&Section::Commits);
    let outline = &panel.ui().outline;
    let section = panel.item_for(&RowKey::Section(Section::Commits));
    // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
    unsafe { outline.expandItem(Some(&section)) };
    // SAFETY: `item_for` returns the panel's cached item for the key, the same object the data source gave the view.
    let row = unsafe { outline.rowForItem(Some(&panel.item_for(&RowKey::Commit(hash.clone())))) };
    let Ok(index) = usize::try_from(row) else {
        return;
    };
    outline.selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(index), false);
    outline.scrollRowToVisible(row);
    if let Some(window) = outline.window() {
        window.makeFirstResponder(Some(outline));
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
