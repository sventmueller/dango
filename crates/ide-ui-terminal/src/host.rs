//! What the terminal views need of the app (E99): the registry of open views, kept in the app's state, and the window
//! a view belongs to, which it names by the window's id and asks to show what it opened.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use ide_terminal::Event;
use objc2::rc::{Retained, Weak};

use super::view::TerminalView;
use ide_ui_common::util::main_soon;

/// Something the user opened from a terminal, which the app shows where it belongs (Review C13 "Link geöffnet").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opened {
    /// A web address, for a browser tab.
    Url(String),
    /// A file at a line and column, both counted from 1.
    File { path: PathBuf, line: usize, column: usize },
    /// A new terminal in this folder.
    TerminalHere(PathBuf),
}

/// The app the terminal views live in, as far as they need it.
pub trait Host {
    /// The terminal views of the running app.
    fn state(&self) -> Rc<State>;
    /// The terminal views unless the app has none (yet), for code such as `Drop` that must not fail.
    fn try_state(&self) -> Option<Rc<State>>;
    /// Shows what the user opened from a terminal in window `window`.
    fn opened(&self, window: u64, what: Opened);
    /// Names tab `tab` of window `window` `title`.
    fn set_tab_title(&self, window: u64, tab: isize, title: &str);
}

static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();

/// Makes `host` the app the terminal views belong to; once, before the first terminal opens.
pub fn install(host: &'static (dyn Host + Sync)) {
    let _ = HOST.set(host);
}

pub(super) fn host() -> &'static (dyn Host + Sync) {
    *HOST
        .get()
        .expect("the app installs its host before the first terminal opens")
}

/// The terminal views of every window by id.
#[derive(Default)]
pub struct State {
    views: RefCell<HashMap<u64, Weak<TerminalView>>>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// The setting `OPTION_AS_META`, read when a terminal opens and set by the settings window.
static OPTION_AS_META: AtomicBool = AtomicBool::new(false);

/// Makes ⌥ Meta for every key in all terminals, or only where it types no character.
pub fn set_option_as_meta(on: bool) {
    OPTION_AS_META.store(on, Ordering::Relaxed);
}

pub(super) fn option_as_meta() -> bool {
    OPTION_AS_META.load(Ordering::Relaxed)
}

pub(super) fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub(super) fn register(view: &TerminalView) {
    host().state().views.borrow_mut().insert(view.id(), Weak::from(view));
}

pub(super) fn forget(id: u64) {
    if let Some(state) = HOST.get().and_then(|host| host.try_state()) {
        state.views.borrow_mut().remove(&id);
    }
}

/// The terminal view `id`, while it is open.
pub fn find(id: u64) -> Option<Retained<TerminalView>> {
    host().state().views.borrow().get(&id).and_then(Weak::load)
}

/// The terminals open now, in every window.
pub fn all_views() -> Vec<Retained<TerminalView>> {
    host().state().views.borrow().values().filter_map(Weak::load).collect()
}

/// Hands the session's events, which arrive on its reader thread, to the view on the main thread.
pub(super) fn listener(id: u64) -> impl Fn(Event) + Send + Sync + 'static {
    move |event| {
        main_soon(move || {
            if let Some(view) = find(id) {
                view.handle(event);
            }
        });
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
