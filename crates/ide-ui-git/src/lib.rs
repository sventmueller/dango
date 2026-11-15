//! The Git panel of the sidebar and the diff tab. git runs only in folders the user trusts (E22), always off the main
//! thread; destructive operations, such as discarding or deleting a stash entry, ask first (E27).

mod commands;
mod commit_field;
mod diff;
mod diff_view;
mod graph;
mod history;
mod host;
pub mod message;
mod model;
mod ops;
mod panel;
#[cfg(feature = "selftest")]
pub mod probe;
mod progress;
mod refresh;
mod reveal;
mod rows;
mod sheets;
pub use sheets::confirm_delete;
mod stashes;
use ide_git::panel as text;
mod toolbar;
mod trees;
mod ui;
mod work;
pub mod worktrees;

/// The settings this crate declares, for the app's registry of every setting.
pub const SETTINGS: &[ide_core::settings::Declared] =
    &[message::source::MODEL.declared(), message::source::CONSENT.declared()];

use std::path::Path;

use ide_ui_common::window::AsWindow;

pub use commands::menu;
pub use host::{Host, State, install};
#[cfg(feature = "selftest")]
pub use reveal::selected_commit;
pub use reveal::{reveal_commit, show_commit_diff};
pub use text::ago;
pub use toolbar::{TOOLBAR_ID, toolbar_item};
#[cfg(feature = "selftest")]
pub use toolbar::{choose as toolbar_choose, seen as toolbar_seen};
pub use ui::empty_state;
pub use work::git;

/// Puts the Git panel of the window's folder in place.
pub fn attach(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    panel::GitPanel::attach(controller);
}

/// Something changed below `root` on disk, in the working tree or in `.git`.
pub fn changed(root: &Path) {
    for panel in panel::for_root(root) {
        panel.schedule_refresh();
    }
}

/// Whether the window's folder is a repository with a commit, as its Git panel read it last.
pub fn has_commit(controller: &(impl AsWindow + ?Sized)) -> bool {
    let controller = &controller.as_window();
    panel::of(controller).is_some_and(|panel| {
        let state = panel.read();
        state.phase == model::Phase::Ready && state.branch.commit.is_some()
    })
}

/// Makes a worktree for a thread of the agent `agent` titled `title` in the window's repository and opens the thread
/// there (I1).
pub fn create_tree(controller: &(impl AsWindow + ?Sized), title: String, agent: String) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.create_tree(title, agent);
    }
}

/// Reads git again for the window's Git panel, as after a change it made.
pub fn refresh(controller: &(impl AsWindow + ?Sized)) {
    let controller = &controller.as_window();
    if let Some(panel) = panel::of(controller) {
        panel.start_refresh();
    }
}

/// The user trusts `root` now, so git may start there.
pub fn trust_changed(root: &Path) {
    for panel in panel::for_root(root) {
        panel.start_refresh();
    }
}

/// What the self-test reads from and does with a window's Git panel, through the code its controls run.
#[cfg(feature = "selftest")]
pub mod selftest;
