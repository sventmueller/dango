//! The app as the `ide-ui-*` crates see it: each crate's `Host`, answered from the app context (E99).

use std::rc::Rc;

/// The running app, as the crates' hooks reach it.
pub struct AppHost;

impl ide_ui_export::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_export::State> {
        crate::context::app().exports.clone()
    }
}

impl ide_ui_terminal::Host for AppHost {
    fn state(&self) -> Rc<ide_ui_terminal::State> {
        crate::context::app().terminal_views.clone()
    }

    fn try_state(&self) -> Option<Rc<ide_ui_terminal::State>> {
        crate::context::try_app().map(|cx| cx.terminal_views.clone())
    }

    fn opened(&self, window: u64, what: ide_ui_terminal::Opened) {
        use ide_ui_terminal::Opened;
        let Some(controller) = crate::context::app().windows.get(window) else {
            return;
        };
        match what {
            Opened::Url(url) => crate::browser::open_tab(&controller, Some(url.as_str())),
            Opened::File { path, line, column } => crate::editor::reveal_position(&controller, &path, line, column),
            Opened::TerminalHere(cwd) => crate::terminal::open_tab(&controller, Some(cwd.as_path())),
        }
    }

    fn set_tab_title(&self, window: u64, tab: isize, title: &str) {
        if let Some(controller) = crate::context::app().windows.get(window) {
            controller.set_tab_title(tab, title);
        }
    }
}

#[cfg(feature = "dictation")]
impl crate::dictation::Host for AppHost {
    fn state(&self) -> Rc<crate::dictation::State> {
        crate::context::app().dictation.clone()
    }
}
