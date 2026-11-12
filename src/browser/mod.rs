//! The browser tab (E17): a `WKWebView` below a navigation row, which also previews a project's dev server (I7).
//!
//! What pages may do, decided here: every tab of the app shares one persistent website data store, so logins to a
//! dev server survive a restart. Pages get no way into the app (no script message handlers), open other apps'
//! schemes never and local files only inside the window's folder, read-only. Pop-ups and `target=_blank` open as tabs,
//! downloads ask where to save, uploads take only what the user picks, camera and microphone stay off. A certificate
//! macOS does not trust can be accepted only for a development server (see `trust`), HTTP logins are asked for and
//! kept in memory until the app quits. A login from macOS AutoFill goes only into the main frame of the host it was
//! asked for (see `autofill`).

mod address;
mod autofill;
mod controls;
mod dialogs;
mod downloads;
mod failure;
mod notice;
mod trust;
mod view;
mod web;

use std::cell::{Cell, RefCell};

use ide_core::settings::{Settings, keys};
use objc2::MainThreadOnly;
use objc2::rc::{Retained, Weak};
use objc2_web_kit::{WKWebView, WKWebViewConfiguration};
use splat_i18n::t;

use crate::util::main_soon;
use crate::window::{TabKind, WindowController};
use address::Address;
use view::BrowserView;

thread_local! {
    /// The open browser tabs of all windows.
    static TABS: RefCell<Vec<Weak<BrowserView>>> = const { RefCell::new(Vec::new()) };
    /// Numbers the keys of tabs that show no particular address.
    static SERIAL: Cell<u64> = const { Cell::new(0) };
}

/// Opens a browser tab at `url`, or empty. A URL whose tab is open selects that tab instead; a tab of a local server
/// reloads, since a dev server that called it has new content.
pub fn open_tab(controller: &WindowController, url: Option<&str>) {
    let engine = Settings::load_user().get(&keys::SEARCH_ENGINE);
    let Some(address) = url.and_then(|url| address::resolve(url, engine)) else {
        let view = BrowserView::new(controller.mtm(), controller, None, unique_key());
        attach(controller, &view);
        view.start_empty();
        return;
    };
    let key = address.key();
    let open = tabs_of(controller)
        .into_iter()
        .find(|view| view.key() == key || view.current_url().is_some_and(|url| address::key_of(&url) == key));
    if let Some(view) = open {
        controller.select_tab(view.key());
        view.revisit(address);
        return;
    }
    let view = BrowserView::new(controller.mtm(), controller, None, key);
    attach(controller, &view);
    view.go(address);
}

/// Reloads the open tabs of `controller`'s window that show a page of `origin`, such as `http://localhost:5173`,
/// for a dev server that does not reload by itself after a file was saved (I7).
#[expect(
    dead_code,
    reason = "the editor calls it after saving, once it knows the project's dev servers"
)]
pub fn reload_matching(controller: &WindowController, origin: &str) {
    let Some(origin) = address::origin(origin) else { return };
    for view in tabs_of(controller) {
        if view.current_url().and_then(|url| address::origin(&url)).as_deref() == Some(origin.as_str()) {
            view.reload_page();
        }
    }
}

/// The address the browser tab `tab` of `controller`'s window shows, for opening it again.
pub fn url_of_tab(controller: &WindowController, tab: isize) -> Option<String> {
    tabs_of(controller)
        .into_iter()
        .find(|view| view.tab() == tab)
        .and_then(|view| view.current_url())
}

/// Opens `url` from a link of `opener`'s page in a new tab beside it.
fn open_tab_beside(opener: &BrowserView, url: &str) {
    let Some(controller) = opener.controller() else { return };
    let view = BrowserView::new(controller.mtm(), &controller, None, unique_key());
    attach(&controller, &view);
    view.go(Address::Url(url.to_string()));
}

/// A tab for a page that `opener`'s page opens with `window.open` or `target=_blank`; WebKit loads it into the web
/// view this returns, which keeps the page's link to its opener.
fn open_popup(opener: &BrowserView, configuration: &WKWebViewConfiguration) -> Option<Retained<WKWebView>> {
    let controller = opener.controller()?;
    let view = BrowserView::new(controller.mtm(), &controller, Some(configuration), unique_key());
    attach(&controller, &view);
    Some(view.web_view())
}

/// Closes the tab of `view` once WebKit is done with the call that asked for it.
fn close_later(view: &BrowserView) {
    let address = std::ptr::from_ref(view) as usize;
    main_soon(move || {
        let view = TABS.with(|tabs| {
            tabs.borrow()
                .iter()
                .filter_map(Weak::load)
                .find(|view| std::ptr::from_ref::<BrowserView>(view) as usize == address)
        });
        if let Some(view) = view
            && let Some(controller) = view.controller()
        {
            controller.close_tab(view.tab());
        }
    });
}

fn attach(controller: &WindowController, view: &Retained<BrowserView>) {
    let id = controller.add_tab(
        TabKind::Browser,
        Some(view.key().to_string()),
        t("Browser"),
        Retained::into_super(view.clone()),
    );
    view.set_tab(id);
    TABS.with(|tabs| tabs.borrow_mut().push(Weak::from_retained(view)));
}

/// The tab whose page is `web`.
fn tab_of(web: &WKWebView) -> Option<Retained<BrowserView>> {
    TABS.with(|tabs| {
        tabs.borrow()
            .iter()
            .filter_map(Weak::load)
            .find(|view| std::ptr::eq(&*view.web_view(), web))
    })
}

/// The browser tab in front in `controller`'s window.
fn front(controller: &WindowController) -> Option<Retained<BrowserView>> {
    let selected = controller.selected_tab()?;
    tabs_of(controller).into_iter().find(|view| view.tab() == selected)
}

/// Runs a browser command of the menu bar on the browser tab in front, wherever the focus is.
pub fn perform(controller: &WindowController, action: objc2::runtime::Sel) {
    if let Some(view) = front(controller) {
        view.perform(action);
    }
}

/// Whether the browser command `action` of the menu bar applies to the tab in front.
pub fn validate(controller: &WindowController, action: objc2::runtime::Sel) -> bool {
    front(controller).is_some_and(|view| view.validate(Some(action)))
}

/// The open browser tabs of `controller`'s window; forgets closed ones.
fn tabs_of(controller: &WindowController) -> Vec<Retained<BrowserView>> {
    TABS.with(|tabs| {
        let mut tabs = tabs.borrow_mut();
        tabs.retain(|tab| tab.load().is_some());
        tabs.iter()
            .filter_map(Weak::load)
            .filter(|view| view.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
            .collect()
    })
}

/// A key for a tab that shows no particular address, which no address ever has.
fn unique_key() -> String {
    let serial = SERIAL.with(|serial| {
        serial.set(serial.get() + 1);
        serial.get()
    });
    format!("browser:{serial}")
}

/// What the self-test reads from a browser tab.
#[cfg(feature = "selftest")]
pub struct Seen {
    pub url: Option<String>,
    /// The text of the address field.
    pub address: String,
    /// Whether the address field is being edited.
    pub editing: bool,
    /// Whether the Web Inspector is on.
    pub inspector: bool,
    /// The heading of the native page standing in for the web page, while it shows.
    pub failure: Option<String>,
    /// Whether that page offers to accept the server's certificate.
    pub proceed: bool,
    /// The text of the notice bar, while it shows.
    pub notice: Option<String>,
}

/// Presses "Trotzdem öffnen" on the first tab that offers it; whether one did.
#[cfg(feature = "selftest")]
pub fn proceed(controller: &WindowController) -> bool {
    let Some(view) = tabs_of(controller).into_iter().find(|view| view.seen().proceed) else {
        return false;
    };
    view.proceed();
    true
}

#[cfg(feature = "selftest")]
pub use dialogs::answer_asking;

/// Presses the key button of the browser tab in front, as the self-test's user.
#[cfg(feature = "selftest")]
pub fn fill_login(controller: &WindowController) {
    if let Some(view) = front(controller) {
        view.fill_login();
    }
}

#[cfg(feature = "selftest")]
pub fn inspect(controller: &WindowController) -> Vec<Seen> {
    tabs_of(controller).iter().map(|view| view.seen()).collect()
}
