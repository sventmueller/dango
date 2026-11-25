//! Worktree threads (I1) as the windows reach them: the logic is `ide_git::thread_trees` (F49, E99); here is only
//! what tells a window's Git panel that a worktree's files changed.

use std::path::Path;

#[cfg(feature = "selftest")]
pub use ide_git::thread_trees::read_counts;
pub use ide_git::thread_trees::{
    TakeOver, TreeError, create, folder, keep_branch, list, read, remove, stage_pending, take_over, tree_of,
};
pub use ide_git::{ThreadTree, TreeState};

/// A thread finished a turn in `cwd`: when that is a thread's worktree, which the folder's watcher does not see, the
/// window's Git panel reads its changes.
pub fn turn_ended(window: &(impl ide_ui_common::window::AsWindow + ?Sized), cwd: &Path) {
    if ide_git::thread_trees::holds(cwd) {
        ide_git::thread_trees::mark(cwd);
        crate::refresh(window);
    }
}

/// The editor saved `path`: when it lies in a thread's worktree, the window's Git panel reads that worktree's changes.
pub fn file_saved(window: &(impl ide_ui_common::window::AsWindow + ?Sized), path: &Path) {
    if let Some(tree) = tree_of(path) {
        ide_git::thread_trees::mark(&tree.path);
        crate::refresh(window);
    }
}
