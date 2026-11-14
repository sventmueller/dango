//! The browser tabs of every window, by the id of the window each lives in: opening, finding and closing them.

use std::cell::{Cell, RefCell};

use ide_core::settings::{Settings, keys};
use objc2::rc::{Retained, Weak};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_web_kit::{WKWebView, WKWebViewConfiguration};
use splat_i18n::t;

use super::address::{self, Address};
use super::host::state;
use super::view::BrowserView;
use ide_ui_common::util::main_soon;
use ide_ui_common::window::{TabId, TabKind, Tabs, Window, WindowId};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// The browser tabs of every window.
#[derive(Default)]
pub struct BrowserTabs {
    /// The open browser tabs of all windows.
    tabs: RefCell<Vec<Weak<BrowserView>>>,
    /// Numbers the keys of tabs that show no particular address.
    serial: Cell<u64>,
}

fn with_tabs<R>(f: impl FnOnce(&RefCell<Vec<Weak<BrowserView>>>) -> R) -> R {
    f(&state().tabs.tabs)
}

fn with_serial<R>(f: impl FnOnce(&Cell<u64>) -> R) -> R {
    f(&state().tabs.serial)
}

/// Opens a browser tab at `url`, or empty, in `window`. A URL whose tab is open selects that tab instead; a tab of a
/// local server reloads, since a dev server that called it has new content.
pub fn open_tab(mtm: MainThreadMarker, window: WindowId, url: Option<&str>) {
    let engine = Settings::load_user().get(&keys::SEARCH_ENGINE);
    let Some(address) = url.and_then(|url| address::resolve(url, engine)) else {
        let view = BrowserView::new(mtm, window, None, unique_key());
        if attach(&view) {
            view.start_empty();
        }
        return;
    };
    let key = address.key();
    let open = tabs_of(window)
        .into_iter()
        .find(|view| view.key() == key || view.current_url().is_some_and(|url| address::key_of(&url) == key));
    if let Some(view) = open {
        Window::new(window).select_tab(view.key());
        view.revisit(address);
        return;
    }
    let view = BrowserView::new(mtm, window, None, key);
    if attach(&view) {
        view.go(address);
    }
}

/// Reloads the open tabs of `window` that show a page of `origin`, such as `http://localhost:5173`, for a dev server
/// that does not reload by itself after a file was saved (I7).
pub fn reload_matching(window: WindowId, origin: &str) {
    let Some(origin) = address::origin(origin) else { return };
    for view in tabs_of(window) {
        if view.current_url().and_then(|url| address::origin(&url)).as_deref() == Some(origin.as_str()) {
            view.reload_page();
        }
    }
}

/// The addresses the browser tabs of `window` show.
pub fn urls(window: WindowId) -> Vec<String> {
    tabs_of(window)
        .into_iter()
        .filter_map(|view| view.current_url())
        .collect()
}

/// The address the browser tab `tab` of `window` shows, for opening it again.
pub fn url_of_tab(window: WindowId, tab: TabId) -> Option<String> {
    tabs_of(window)
        .into_iter()
        .find(|view| view.tab() == Some(tab))
        .and_then(|view| view.current_url())
}

/// Opens `url` from a link of `opener`'s page in a new tab beside it.
pub(super) fn open_tab_beside(opener: &BrowserView, url: &str) {
    let view = BrowserView::new(opener.mtm(), opener.window_id(), None, unique_key());
    if attach(&view) {
        view.go(Address::Url(url.to_string()));
    }
}

/// A tab for a page that `opener`'s page opens with `window.open` or `target=_blank`; WebKit loads it into the web
/// view this returns, which keeps the page's link to its opener.
pub(super) fn open_popup(opener: &BrowserView, configuration: &WKWebViewConfiguration) -> Option<Retained<WKWebView>> {
    let view = BrowserView::new(opener.mtm(), opener.window_id(), Some(configuration), unique_key());
    attach(&view).then(|| view.web_view())
}

/// Closes the tab of `view` once WebKit is done with the call that asked for it.
pub(super) fn close_later(view: &BrowserView) {
    let address = std::ptr::from_ref(view) as usize;
    main_soon(move || {
        let view = with_tabs(|tabs| {
            tabs.borrow()
                .iter()
                .filter_map(Weak::load)
                .find(|view| std::ptr::from_ref::<BrowserView>(view) as usize == address)
        });
        if let Some((view, tab)) = view.and_then(|view| view.tab().map(|tab| (view, tab))) {
            Window::new(view.window_id()).close_tab(tab);
        }
    });
}

/// Adds `view` as a tab of its window; whether the window was still open.
fn attach(view: &Retained<BrowserView>) -> bool {
    let added = Window::new(view.window_id()).add_tab(
        TabKind::Browser,
        Some(view.key().to_string()),
        t("Browser"),
        Retained::into_super(view.clone()),
    );
    let Some(id) = added else { return false };
    view.set_tab(id);
    with_tabs(|tabs| tabs.borrow_mut().push(Weak::from_retained(view)));
    true
}

/// The tab whose page is `web`.
pub(super) fn tab_of(web: &WKWebView) -> Option<Retained<BrowserView>> {
    with_tabs(|tabs| {
        tabs.borrow()
            .iter()
            .filter_map(Weak::load)
            .find(|view| std::ptr::eq(&*view.web_view(), web))
    })
}

/// The browser tab in front in `window`.
fn front(window: WindowId) -> Option<Retained<BrowserView>> {
    let selected = Window::new(window).selected_tab()?;
    tabs_of(window).into_iter().find(|view| view.tab() == Some(selected))
}

/// Runs a browser command of the menu bar on the browser tab in front of `window`, wherever the focus is.
pub fn perform(window: WindowId, action: objc2::runtime::Sel) {
    if let Some(view) = front(window) {
        view.perform(action);
    }
}

/// Whether the browser command `action` of the menu bar applies to the tab in front of `window`.
pub fn validate(window: WindowId, action: objc2::runtime::Sel) -> bool {
    front(window).is_some_and(|view| view.validate(Some(action)))
}

/// The open browser tabs of `window`; forgets closed ones.
fn tabs_of(window: WindowId) -> Vec<Retained<BrowserView>> {
    with_tabs(|tabs| {
        let mut tabs = tabs.borrow_mut();
        tabs.retain(|tab| tab.load().is_some());
        tabs.iter()
            .filter_map(Weak::load)
            .filter(|view| view.window_id() == window)
            .collect()
    })
}

/// A key for a tab that shows no particular address, which no address ever has.
fn unique_key() -> String {
    let serial = with_serial(|serial| {
        serial.set(serial.get() + 1);
        serial.get()
    });
    format!("browser:{serial}")
}
