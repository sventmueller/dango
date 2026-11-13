//! The app as the browser sees it: the windows its tabs live in, by id, and the state of every tab, download and
//! accepted certificate, which the app holds in its context (E99).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2_app_kit::NSView;

/// What the browser asks of the app; a window is named by its id, which stays valid after the window closed.
pub trait Host {
    /// The browser's state in the running app.
    fn state(&self) -> Rc<State>;
    /// Adds `view` as a browser tab named `key` to `window`; its tab id, or `None` once the window closed.
    fn add_tab(&self, window: u64, key: String, title: &str, view: Retained<NSView>) -> Option<isize>;
    /// Brings the tab named `key` of `window` to the front.
    fn select_tab(&self, window: u64, key: &str);
    fn close_tab(&self, window: u64, tab: isize);
    /// The tab in front in `window`.
    fn selected_tab(&self, window: u64) -> Option<isize>;
    /// Sets the title of `tab` in `window`; whether the window is still open.
    fn set_tab_title(&self, window: u64, tab: isize, title: &str) -> bool;
    /// The folder `window` shows, whose files its tabs may load.
    fn project_root(&self, window: u64) -> Option<PathBuf>;
}

static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();

/// Makes `host` the app the browser tabs belong to; once, before the first tab opens.
pub fn install(host: &'static (dyn Host + Sync)) {
    let _ = HOST.set(host);
}

pub(super) fn host() -> &'static (dyn Host + Sync) {
    *HOST
        .get()
        .expect("the app installs its host before the first browser tab")
}

pub(super) fn state() -> Rc<State> {
    host().state()
}

/// The browser tabs of every window, the downloads that run and the certificates of development servers.
#[derive(Default)]
pub struct State {
    pub(super) tabs: super::tabs::BrowserTabs,
    pub(super) downloads: super::downloads::Downloading,
    pub(super) certificates: super::trust::Certificates,
    #[cfg(feature = "selftest")]
    pub(super) dialogs: super::dialogs::Dialogs,
}
