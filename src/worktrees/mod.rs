//! Worktree threads (I1) as the app reaches them: `ide_ui_git::worktrees`, and starting a thread in a new worktree
//! from the menu of new tabs.

mod start;

pub use ide_ui_git::worktrees::*;
pub use start::{agent_usable, agents, ask_and_open, can_start, open_thread};
