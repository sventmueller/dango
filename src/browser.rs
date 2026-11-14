//! The browser tab of a window (E17) as the app reaches it: `ide_ui_browser` by the window's id, and the app as that
//! crate's `Host`.

use std::path::PathBuf;
use std::rc::Rc;

use ide_ui_browser::{Host, State};
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::NSView;

use crate::window::{TabKind, WindowController};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// Opens a browser tab at `url`, or empty. A URL whose tab is open selects that tab instead; a tab of a local server
/// reloads, since a dev server that called it has new content.
pub fn open_tab(controller: &WindowController, url: Option<&str>) {
    ide_ui_browser::open_tab(controller.mtm(), controller.id(), url);
}

/// Reloads the open tabs of `controller`'s window that show a page of `origin`, such as `http://localhost:5173`,
/// for a dev server that does not reload by itself after a file was saved (I7).
#[expect(
    dead_code,
    reason = "the editor calls it after saving, once it knows the project's dev servers"
)]
pub fn reload_matching(controller: &WindowController, origin: &str) {
    ide_ui_browser::reload_matching(controller.id(), origin);
}

/// The address the browser tab `tab` of `controller`'s window shows, for opening it again.
pub fn url_of_tab(controller: &WindowController, tab: isize) -> Option<String> {
    ide_ui_browser::url_of_tab(controller.id(), tab)
}

/// Runs a browser command of the menu bar on the browser tab in front, wherever the focus is.
pub fn perform(controller: &WindowController, action: objc2::runtime::Sel) {
    ide_ui_browser::perform(controller.id(), action);
}

/// Whether the browser command `action` of the menu bar applies to the tab in front.
pub fn validate(controller: &WindowController, action: objc2::runtime::Sel) -> bool {
    ide_ui_browser::validate(controller.id(), action)
}

impl Host for crate::host::AppHost {
    fn state(&self) -> Rc<State> {
        crate::host::context().browser().clone()
    }

    fn add_tab(&self, window: u64, key: String, title: &str, view: Retained<NSView>) -> Option<isize> {
        let controller = crate::host::window(window)?;
        Some(controller.add_tab(TabKind::Browser, Some(key), title, view))
    }

    fn select_tab(&self, window: u64, key: &str) {
        if let Some(controller) = crate::host::window(window) {
            controller.select_tab(key);
        }
    }

    fn close_tab(&self, window: u64, tab: isize) {
        if let Some(controller) = crate::host::window(window) {
            controller.close_tab(tab);
        }
    }

    fn selected_tab(&self, window: u64) -> Option<isize> {
        crate::host::window(window)?.selected_tab()
    }

    fn set_tab_title(&self, window: u64, tab: isize, title: &str) -> bool {
        let Some(controller) = crate::host::window(window) else {
            return false;
        };
        controller.set_tab_title(tab, title);
        true
    }

    fn project_root(&self, window: u64) -> Option<PathBuf> {
        crate::host::window(window)?.project_root()
    }
}
