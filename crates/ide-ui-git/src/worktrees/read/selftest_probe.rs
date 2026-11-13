//! What the self-test reads from and does with the parts of `read.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// How often the worktree list was read, and how often a worktree's files.
pub(super) static COUNTS: Mutex<(usize, usize)> = Mutex::new((0, 0));

/// How often the worktree list was read, and how often a worktree's files, for the self-test.
pub fn counts() -> (usize, usize) {
    *COUNTS.lock_ignoring_poison()
}

pub(super) fn count(list: usize, files: usize) {
    let mut counts = COUNTS.lock_ignoring_poison();
    counts.0 += list;
    counts.1 += files;
}
