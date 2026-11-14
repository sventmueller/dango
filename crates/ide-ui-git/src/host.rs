//! The app as the Git panel reaches it: its editors, threads and agent settings (E99).

use ide_ui_common::window::WindowId;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use ide_agent::{ApiKey, Endpoint, ProviderError};
use ide_git::ThreadTree;
use ide_ui_common::registry::{Registry, Serial};
use objc2::rc::{Retained, Weak};

/// The Git panels, diff tabs and branch toolbar items of every window.
#[derive(Default)]
pub struct State {
    pub(crate) panels: Registry<Retained<crate::panel::GitPanel>>,
    pub(crate) diff_tabs: Registry<Weak<crate::diff::DiffTab>>,
    pub(crate) toolbar_items: RefCell<Vec<crate::toolbar::Shown>>,
    pub(crate) message_runs: Serial,
}

/// What the Git panel needs of the app's editors, threads and agent settings; the rest of a window it reaches
/// through `ide_ui_common::window`.
pub trait Host {
    /// The Git panel's state in the running app.
    fn state(&self) -> Rc<State>;
    /// The files below `root` with edits not yet saved.
    fn unsaved_below(&self, root: &Path) -> Vec<PathBuf>;
    /// Saves the open documents below `root`; whether all of them were saved.
    fn save_below(&self, root: &Path) -> bool;
    /// The open editors below `root` take what changed on disk (E24).
    fn check_disk_below(&self, root: &Path);
    /// Brings the open thread that works in `folder` to the front; whether there was one.
    fn select_thread_in(&self, folder: &Path) -> bool;
    /// Whether a thread works in `folder`.
    fn thread_works_in(&self, folder: &Path) -> bool;
    /// Opens a thread of the worktree's agent in `tree` in `window` (I1).
    fn open_thread(&self, window: WindowId, tree: &ThreadTree);
    /// The `claude` program on the login shell's PATH.
    fn claude_program(&self, env: &[(String, String)]) -> Option<PathBuf>;
    /// The own agent's endpoint `id` (E6).
    fn endpoint(&self, id: &str) -> Option<Endpoint>;
    /// The key the user gave `endpoint`.
    fn key(&self, endpoint: &Endpoint) -> Option<ApiKey>;
    /// What a provider's error says, in the user's words.
    fn provider_error(&self, error: &ProviderError) -> String;
}

static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();

/// Makes `host` the app the Git panels belong to; once, before the first window opens.
pub fn install(host: &'static (dyn Host + Sync)) {
    let _ = HOST.set(host);
}

pub(crate) fn host() -> &'static (dyn Host + Sync) {
    *HOST
        .get()
        .expect("the app installs its host before the first Git panel")
}

pub(crate) fn state() -> Rc<State> {
    host().state()
}
