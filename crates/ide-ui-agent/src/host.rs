//! The app as the AI tab and the Agents panel reach it: its terminals, editors, settings, folder trust, worktrees and
//! activity view, by window id (E99).

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use objc2::MainThreadMarker;
use objc2_app_kit::NSView;

use crate::work::Part;

/// What the AI tab needs of the app beside the window, which it reaches through `ide_ui_common::window`.
pub trait Host {
    /// The AI tabs' state in the running app.
    fn state(&self) -> Rc<crate::State>;
    /// The label and recent text of the terminal in view in `window`, else of the one opened last.
    fn recent_output(&self, window: u64) -> Option<(String, String)>;
    /// The selection of the editor in view in `window`, else of any of its editors that has one: its file, first and
    /// last line from 1, and text.
    fn editor_selection(&self, window: u64) -> Option<(PathBuf, (u32, u32), String)>;
    /// The diagnostics the language servers published below `root`, for the own agent (E6).
    fn agent_diagnostics(&self, root: &Path) -> Arc<dyn ide_agent::Diagnostics>;
    /// Asks the quick open index of `window` for the files that match `query`, at most `limit`.
    fn query_files(
        &self,
        window: u64,
        query: &str,
        limit: usize,
        answer: Box<dyn FnOnce(Vec<ide_project::Hit>) + Send>,
    );
    /// Opens a terminal tab of `window` in `cwd`.
    fn open_terminal(&self, window: u64, cwd: &Path);
    /// Shows the settings' tab of the own agent.
    fn show_agent_settings(&self, mtm: MainThreadMarker);
    /// Shows the settings' tab of the ACP agents.
    fn show_agents_settings(&self, mtm: MainThreadMarker);
    /// Keeps the permission rules Claude's settings in `cwd` gained, so the settings can show them.
    fn remember_claude_rules(&self, cwd: &Path);
    /// A thread finished a turn in `cwd`, which may be a worktree whose changes the Git panel reads then (I1).
    fn turn_ended(&self, window: u64, cwd: &Path);
    /// Shows an alert with `message` and `detail`.
    fn show_alert(&self, mtm: MainThreadMarker, message: &str, detail: &str);
    /// The user trusts `folder` (E22).
    fn trust(&self, folder: &Path);
    /// Opens the activity view's list of `part` of the AI tab `key` below `anchor`.
    fn show_work(&self, window: u64, key: u64, part: Part, anchor: &NSView);
}

static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();

/// Makes `host` the app the AI tabs belong to; once, before the first window opens.
pub fn install(host: &'static (dyn Host + Sync)) {
    let _ = HOST.set(host);
}

pub(crate) fn host() -> &'static (dyn Host + Sync) {
    *HOST.get().expect("the app installs its host before the first AI tab")
}

pub(crate) fn state() -> Rc<crate::State> {
    host().state()
}
