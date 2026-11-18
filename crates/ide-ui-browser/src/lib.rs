//! The browser tab (E17): a `WKWebView` below a navigation row, which also previews a project's dev server (I7).
//!
//! What pages may do, decided here: every tab of the app shares one persistent website data store, so logins to a
//! dev server survive a restart. Pages get no way into the app (no script message handlers), open other apps'
//! schemes never and local files only inside the window's folder, read-only. Pop-ups and `target=_blank` open as tabs,
//! downloads ask where to save, uploads take only what the user picks, camera and microphone stay off. A certificate
//! macOS does not trust can be accepted only for a development server (see `trust`), HTTP logins are asked for and
//! kept in memory until the app quits. A login from macOS AutoFill goes only into the main frame of the host it was
//! asked for (see `autofill`).
//!
//! The crate knows windows only by their ids; the app adds and closes their tabs and holds the browser's `State`
//! through `Host` (E99).

mod address;
mod autofill;
mod controls;
mod dialogs;
mod downloads;
mod failure;
mod host;
mod notice;
mod tabs;
mod trust;
mod view;
mod web;

pub use host::{Host, State, install};
pub use tabs::{open_tab, perform, reload_matching, url_of_tab, urls, validate};
pub use trust::may_accept;
pub use web::set_local_cookie;
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
