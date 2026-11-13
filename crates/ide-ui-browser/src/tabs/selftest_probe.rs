//! What the self-test reads from and does with the parts of `tabs.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// What the self-test reads from a browser tab.
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

/// Presses "Trotzdem öffnen" on the first tab of `window` that offers it; whether one did.
pub fn proceed(window: u64) -> bool {
    let Some(view) = tabs_of(window).into_iter().find(|view| view.seen().proceed) else {
        return false;
    };
    view.proceed();
    true
}

/// Presses the key button of the browser tab in front of `window`, as the self-test's user.
pub fn fill_login(window: u64) {
    if let Some(view) = front(window) {
        view.fill_login();
    }
}

/// The browser tab in front of `window`, which takes the actions of its buttons.
pub fn front_view(window: u64) -> Option<Retained<objc2_app_kit::NSView>> {
    front(window).map(Retained::into_super)
}

/// The address field of the browser tab in front of `window`.
pub fn address_field(window: u64) -> Option<Retained<objc2_app_kit::NSTextField>> {
    front(window).map(|view| view.address_field())
}

pub fn inspect(window: u64) -> Vec<Seen> {
    tabs_of(window).iter().map(|view| view.seen()).collect()
}
