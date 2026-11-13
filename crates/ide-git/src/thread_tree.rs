//! A worktree an AI thread works in (I1): what the app keeps about it and how it stands against its base.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ChangedFile;

/// What the app keeps about a thread's worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadTree {
    /// Its folder.
    pub path: PathBuf,
    /// Its branch, such as `thread/fix-login`.
    pub branch: String,
    /// The thread's title, which the branch is named after.
    pub title: String,
    /// The commit the branch started at.
    pub base: String,
    /// The branch the user was on then, which ahead and behind count against; `None` for a detached HEAD.
    pub base_branch: Option<String>,
    /// The agent its thread runs, by the id threads name it with; empty means Claude.
    #[serde(default)]
    pub agent: String,
}

/// A thread's worktree as it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeState {
    /// What the app keeps about the worktree.
    pub tree: ThreadTree,
    /// Commits on the thread's branch the base lacks.
    pub ahead: usize,
    /// Commits on the base the thread's branch lacks.
    pub behind: usize,
    /// Where the branch and the base meet, which its changes are shown against.
    pub since: String,
    /// What the worktree changed since then, committed or not.
    pub changed: Vec<ChangedFile>,
}
