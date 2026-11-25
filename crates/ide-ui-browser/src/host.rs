//! The app as the browser sees it beside the windows its tabs live in, which it reaches through
//! `ide_ui_common::window`: the state of every tab, download and accepted certificate, which the app holds in its
//! context (E99).

use std::rc::Rc;
use std::sync::OnceLock;

/// What the browser asks of the app beside its windows.
pub trait Host {
    /// The browser's state in the running app.
    fn state(&self) -> Rc<State>;
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
