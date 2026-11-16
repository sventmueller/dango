//! git for the app, through the `git` program: repositories, status, diffs, staging down to single hunks, commits,
//! remotes, branches, the history as a graph with what each commit changed, the stash, worktrees and bringing a branch's
//! work over by merge or cherry-pick, checkpoints of a folder before an agent's turn, a secrets check before commits
//! and of any text before it leaves the Mac ([`Git::scan_text`]), and the checked text a model gets to write a commit
//! message.
//!
//! [`panel`] is what the app's Git panel decides without AppKit: its rows, the commit graph and its words for git's
//! errors; [`ThreadTree`] and [`TreeState`] describe the worktree of an AI thread or a work order, and
//! [`thread_trees`] makes, reads, takes over and removes them, with the variants a work order needs in
//! [`thread_trees::order`] (B45a).
//!
//! For the orchestrator of work orders (phase 13) there are also: [`Git::unattended`], git without the repository's
//! hooks, with a given environment and a deadline (E192); [`Repository::ref_snapshot`] for the ref watch (E173);
//! [`Git::scan_text_frozen`], the secrets check with rules frozen from the user's working copy (C52);
//! [`Repository::full_diff`], [`Repository::changes_between`] and [`Repository::branch_head`] to compare and verify.
//!
//! git runs code a repository controls — hooks, `core.fsmonitor`, filters from its configuration — so callers use this
//! crate only in folders the user trusts (E22). The crate does not check that itself; the project's trust decision
//! belongs to the app.
//!
//! Every call blocks until git ends and so belongs on a worker thread, never the main thread. Every process runs
//! through the supervisor as `Kind::Git` with the login shell's environment. Operations that throw work away for good
//! are reached through [`Repository::destructive`], so the app can ask for the user's approval before each (E27).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod blame;
mod branch;
mod checkpoint;
mod commit;
mod destructive;
mod diff;
mod error;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing;
mod history;
mod hooks;
mod id;
mod integrate;
mod message;
mod operation;
pub mod panel;
mod path;
mod refs;
mod remote;
mod repo;
mod run;
mod said;
mod secrets;
mod staging;
mod stash;
mod status;
mod thread_tree;
pub mod thread_trees;
mod worktree;

pub use blame::{Blame, BlameCommit};
pub use branch::{Branch, BranchHead, Commit};
pub use checkpoint::{
    Checkpoint, CheckpointKind, CheckpointThread, Checkpoints, Created, Prune, Restore, RestoreOptions, Restored,
};
pub use commit::CommitOptions;
pub use destructive::Destructive;
pub use diff::{Change, DiffSource, FileDiff, Hunk, Line, LineKind};
pub use error::{Error, Result};
pub use history::CommitChanges;
pub use id::CommitId;
pub use integrate::ChangedFile;
pub use message::{CheckedRequest, MessageRequest, Place, RequestFinding, RequestScan, SUBJECTS};
pub use operation::Operation;
pub use refs::{RefChange, RefChanges, RefKind, RefSnapshot};
pub use remote::{
    FetchOptions, Progress, PullMode, PullOptions, PushFlag, PushOptions, PushOutcome, PushedRef, Remote,
};
pub use repo::Repository;
pub use run::{Cancel, Git};
pub use secrets::{Finding, FrozenRules, SecretScan};
pub use stash::{Applied, Stash, StashApply, StashOptions, Unstash};
pub use status::{
    BranchStatus, Conflict, EntryKind, FileState, Status, StatusEntry, StatusOptions, Submodule, Untracked,
};
pub use thread_tree::{ThreadTree, TreeState};
pub use worktree::Worktree;
