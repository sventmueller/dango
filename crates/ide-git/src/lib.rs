//! git for the app, through the `git` program: repositories, status, diffs, staging down to single hunks, commits,
//! remotes, branches, the history as a graph with what each commit changed, the stash, worktrees and bringing a branch's
//! work over by merge or cherry-pick, checkpoints of a folder before an agent's turn, a secrets check before commits
//! and of any text before it leaves the Mac ([`Git::scan_text`]), and the checked text a model gets to write a commit
//! message.
//!
//! git runs code a repository controls — hooks, `core.fsmonitor`, filters from its configuration — so callers use this
//! crate only in folders the user trusts (E22). The crate does not check that itself; the project's trust decision
//! belongs to the app.
//!
//! Every call blocks until git ends and so belongs on a worker thread, never the main thread. Every process runs
//! through the supervisor as `Kind::Git` with the login shell's environment. Operations that throw work away for good
//! are reached through [`Repository::destructive`], so the app can ask for the user's approval before each (E27).

#![forbid(unsafe_code)]
#![warn(missing_docs, unreachable_pub)]

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
mod integrate;
mod message;
mod operation;
mod path;
mod remote;
mod repo;
mod run;
mod secrets;
mod staging;
mod stash;
mod status;
mod worktree;

pub use blame::{Blame, BlameCommit};
pub use branch::{Branch, Commit};
pub use checkpoint::{Checkpoint, CheckpointKind, Checkpoints, Created, Prune, Restore, RestoreOptions, Restored};
pub use commit::CommitOptions;
pub use destructive::Destructive;
pub use diff::{Change, DiffSource, FileDiff, Hunk, Line, LineKind};
pub use error::{Error, Result};
pub use history::CommitChanges;
pub use integrate::ChangedFile;
pub use message::{CheckedRequest, LINES_PER_FILE, MessageRequest, Place, RequestFinding, RequestScan, SUBJECTS};
pub use operation::Operation;
pub use remote::{
    FetchOptions, Progress, PullMode, PullOptions, PushFlag, PushOptions, PushOutcome, PushedRef, Remote,
};
pub use repo::Repository;
pub use run::{Cancel, Git};
pub use secrets::{Finding, SecretScan};
pub use stash::{Applied, Stash, StashApply, StashOptions};
pub use status::{
    BranchStatus, Conflict, EntryKind, FileState, Status, StatusEntry, StatusOptions, Submodule, Untracked,
};
pub use worktree::Worktree;
