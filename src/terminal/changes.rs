//! What a terminal reports to the app and what the user opened from it, shown where it belongs: the title on the tab,
//! a dev server in the banner, the bell, notifications and progress on the tab, in macOS and on the Dock icon, a link
//! in the browser tab, a file in the editor, the input field's state on the toolbar's switch.

use std::rc::Rc;

use splat_i18n::t;
use term_ui::kit::WindowId;
use term_ui::{Change, Opened};

use super::signals;
use crate::context::AppContext;

/// Takes in what the session of view `view` reported.
pub(crate) fn changed(cx: &Rc<AppContext>, view: u64, change: Change) {
    let terminals = cx.terminals();
    match change {
        Change::Title(title) => show_title(cx, view, title),
        Change::Directory(folder) => terminals.moved(view, folder),
        Change::Prompt => {
            terminals.prompted(view);
            signals::reported(cx, view, None);
        }
        Change::CommandStarted(line) => terminals.started(view, line),
        Change::CommandFinished(code) => {
            terminals.finished(view, code);
            signals::reported(cx, view, None);
        }
        Change::DevServer(url) => {
            if let Some(banner) = terminals.banner_of(view) {
                banner.show(&url);
            }
        }
        Change::Bell => signals::rang(cx, view),
        Change::Notification(notification) => signals::asked(cx, view, &notification),
        Change::Progress(progress) => signals::reported(cx, view, progress),
        Change::Exited(_) => signals::reported(cx, view, None),
        Change::Focused => signals::seen(cx, view),
        Change::Field => show_field(cx, view),
        Change::Elsewhere(_) | Change::Input => {}
    }
}

/// Shows on the toolbar's switch of view `view`'s window what its input field does now.
fn show_field(cx: &AppContext, view: u64) {
    let window = term_ui::find(view).map(|v| v.window_id());
    if let Some(controller) = window.and_then(|window| cx.windows().get(window.get())) {
        crate::window::toolbar::show_field(&controller);
    }
}

/// Names the tab of view `view` after `title`, or the terminal when the program set none.
fn show_title(cx: &AppContext, view: u64, title: Option<String>) {
    let (Some(tab), Some(window)) = (cx.terminals().tab_of(view), term_ui::find(view).map(|v| v.window_id())) else {
        return;
    };
    if let Some(controller) = cx.windows().get(window.get()) {
        controller.set_tab_title(tab, title.as_deref().unwrap_or_else(|| t("Terminal")));
    }
}

/// Shows what the user opened from a terminal of window `window`.
pub(crate) fn opened(cx: &Rc<AppContext>, window: WindowId, what: Opened) {
    let Some(controller) = cx.windows().get(window.get()) else {
        return;
    };
    match what {
        Opened::Url(url) => crate::browser::open_tab(&controller, Some(&url)),
        Opened::File { path, line, column } => crate::editor::reveal_position(&controller, &path, line, column),
        Opened::TerminalHere(folder) => super::open_tab(&controller, Some(&folder)),
        Opened::ClosePane(view) => {
            // The view asks from its own context menu, which must not see it go before the menu is done.
            let back = cx.leitstand().back();
            crate::util::main_soon(move || {
                back.with(|cx| {
                    if let (Some(tab), Some(controller)) = (cx.terminals().tab_of(view), cx.windows().get(window.get()))
                    {
                        controller.close_tab(tab);
                    }
                });
            });
        }
    }
}
