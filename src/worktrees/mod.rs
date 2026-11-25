//! Worktree threads (I1) as the app reaches them: `ide_ui_git::worktrees`, and starting a thread in a new worktree
//! from the menu of new tabs.

mod start;

pub(crate) use ide_ui_git::worktrees::*;
pub(crate) use start::{agents, ask_and_open, can_start, open_thread};
