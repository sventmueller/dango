//! The app as the debugger reaches it: its editors and terminals by window id (E99).

use ide_ui_common::window::WindowId;
use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use ide_ui_common::registry::Registry;
use objc2::rc::Retained;

use crate::commands::DebugCommands;
use crate::panel::DebugPanel;

/// The debuggers of every window and the Debug menu's target.
#[derive(Default)]
pub struct State {
    pub(crate) panels: Registry<Retained<DebugPanel>>,
    pub(crate) commands: OnceCell<Retained<DebugCommands>>,
}

/// What the debugger needs of the app's editors and terminals; the rest of a window it reaches through
/// `ide_ui_common::window`.
pub trait Host {
    /// The debugger's state in the running app.
    fn state(&self) -> Rc<State>;
    /// The editors of `window` show the debugger's breakpoints, paused line and values anew.
    fn refresh_editors(&self, window: WindowId);
    /// The editors of `window` close the value popovers of a frame that is gone.
    fn forget_values(&self, window: WindowId);
    /// The file of the editor in front in `window` and its language, the file `None` while it is unsaved.
    fn front_editor(&self, window: WindowId) -> Option<(Option<PathBuf>, Option<String>)>;
    /// The file and caret line, from 0, of the editor in front in `window`.
    fn caret_line(&self, window: WindowId) -> Option<(PathBuf, usize)>;
    /// Shows `line`, from 0, of `path` in an editor of `window`, leaving the keyboard focus where it is.
    fn reveal_line(&self, window: WindowId, path: &Path, line: usize);
    /// Runs the shell command `line` in a new terminal tab of `window` in `cwd`; `then` gets the shell's process id,
    /// or `None` when the terminal did not start.
    fn open_running(&self, window: WindowId, cwd: &Path, line: String, then: Box<dyn FnOnce(Option<u32>) + Send>);
}

static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();

/// Makes `host` the app the debugger belongs to; once, before the first window opens.
pub fn install(host: &'static (dyn Host + Sync)) {
    let _ = HOST.set(host);
}

pub(crate) fn host() -> &'static (dyn Host + Sync) {
    *HOST.get().expect("the app installs its host before the first debugger")
}

pub(crate) fn state() -> Rc<State> {
    host().state()
}
