//! What the program of a terminal asks of a user who may look elsewhere, shown as Tapa shows it (Tapa's F55, F56,
//! F63) with the app's own means: the red dot on its tab for the bell and for a notification while the terminal is
//! not in front (E71), the notification of OSC 9 or OSC 777 posted as an AI tab posts its own while another app is in
//! front (I9), and the progress of OSC 9;4 as a ring on its tab and a bar on the Dock icon.

use ide_ui_common::notify;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::NSColor;
use term_session::event::{Notification, Progress};
use term_ui::{TerminalView, find};

use super::progress::Ring;
use crate::context::AppContext;
use crate::window::{TabId, WindowController};

/// What starts the identifier of a terminal's notification, followed by its view's id; a new one replaces the last.
const PREFIX: &str = "terminal-";

/// The program of `view` rang the bell, which the view rang itself: its tab shows the red dot while the user does
/// not see it.
pub(super) fn rang(cx: &AppContext, view: u64) {
    if !find(view).is_some_and(|found| in_front(&found)) {
        show_dot(cx, view, true);
    }
}

/// The program of `view` asked for `notification`: while the terminal is not in front its tab shows the red dot, and
/// while another app is in front the notification goes to macOS.
pub(super) fn asked(cx: &AppContext, view: u64, notification: &Notification) {
    let Some(found) = find(view) else { return };
    if in_front(&found) {
        return;
    }
    show_dot(cx, view, true);
    let (title, body) = parts(notification, &heading(&found));
    let posted = notify::app_in_background();
    if posted {
        follow_clicks(cx);
        notify::post(&format!("{PREFIX}{view}"), &title, &body);
    }
    #[cfg(feature = "selftest")]
    cx.terminals().note_heard(super::state::Heard {
        view,
        title,
        body,
        posted,
    });
}

/// The user looks at the terminal `view` now, which took the keyboard focus: its red dot goes.
pub(super) fn seen(cx: &AppContext, view: u64) {
    show_dot(cx, view, false);
}

/// The window tab in front of `controller`'s window, or the window itself, came to the front: its terminals learn
/// whether they show now, which AppKit does not tell them for a native tab, and those the user sees lose their red dot.
pub(crate) fn came_to_front(controller: &WindowController) {
    let window = controller.id().get();
    for view in term_ui::all_views() {
        if view.window_id().get() == window {
            view.shown();
        }
    }
    let cx = controller.context();
    for view in cx.terminals().waiting_in(controller.id().get()) {
        if find(view).is_some_and(|found| in_front(&found)) {
            show_dot(cx, view, false);
        }
    }
}

/// The program of `view` reported `progress`, or with `None` that its work ended: its tab and the Dock icon show it.
pub(super) fn reported(cx: &AppContext, view: u64, progress: Option<Progress>) {
    let terminals = cx.terminals();
    if !terminals.set_progress(view, progress) {
        return;
    }
    if let Some((controller, tab)) = tab_of(cx, view) {
        controller.set_tab_progress(tab, progress.map(Ring::of));
    }
    if let Some(mtm) = MainThreadMarker::new() {
        terminals.show_progress_on_dock(mtm);
    }
}

/// The document `id` of `controller`'s window closes: what its terminal reported goes, from the Dock icon too.
pub(super) fn forget(controller: &WindowController, id: TabId) {
    let terminals = controller.context().terminals();
    if terminals.forget_tab(controller.id().get(), id) {
        terminals.show_progress_on_dock(controller.mtm());
    }
}

/// Whether the user sees `view`: the app in front, its window the key window, and the view not behind another.
fn in_front(view: &TerminalView) -> bool {
    !notify::app_in_background()
        && !view.isHiddenOrHasHiddenAncestor()
        && view.window().is_some_and(|window| window.isKeyWindow())
}

/// Shows the red dot on the tab of `view`, or takes it away.
fn show_dot(cx: &AppContext, view: u64, waits: bool) {
    if !cx.terminals().set_waits(view, waits) {
        return;
    }
    if let Some((controller, tab)) = tab_of(cx, view) {
        let red = waits.then(NSColor::systemRedColor);
        controller.set_tab_status(tab, red.as_deref());
    }
}

/// The window and the tab that show `view`.
fn tab_of(cx: &AppContext, view: u64) -> Option<(Retained<WindowController>, TabId)> {
    let tab = cx.terminals().tab_of(view)?;
    let controller = cx.windows().get(find(view)?.window_id().get())?;
    Some((controller, tab))
}

/// What names the terminal `view` in its notifications: the title its program set, else its folder from `~`.
fn heading(view: &TerminalView) -> String {
    view.title()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| ide_ui_common::panel::display_path(&view.cwd()))
}

/// The notification's title and text: its own title, else `heading`; with no text, the title alone as the text.
fn parts(notification: &Notification, heading: &str) -> (String, String) {
    let title = notification.title.clone().unwrap_or_else(|| heading.to_string());
    if notification.body.is_empty() {
        return (heading.to_string(), title);
    }
    (title, notification.body.clone())
}

/// Has a click on a terminal's notification bring its tab to the front, once the first is posted.
fn follow_clicks(cx: &AppContext) {
    let back = cx.leitstand().back();
    cx.terminals().keep_clicks(move || {
        notify::follow_clicks(move |identifier| {
            let Some(view) = view_of(&identifier) else { return };
            let back = back.clone();
            crate::util::main_soon(move || {
                back.with(|cx| bring_forward(cx, view));
            });
        })
    });
}

/// Brings the tab of `view` to the front with the keyboard in the terminal.
fn bring_forward(cx: &AppContext, view: u64) {
    let (Some((controller, tab)), Some(found)) = (tab_of(cx, view), find(view)) else {
        return;
    };
    controller.select_tab_by_id(tab);
    if let Some(window) = found.window() {
        window.makeFirstResponder(Some(&found));
    }
}

/// The terminal view a notification's identifier names.
fn view_of(identifier: &str) -> Option<u64> {
    identifier.strip_prefix(PREFIX)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(title: Option<&str>, body: &str) -> Notification {
        Notification {
            title: title.map(str::to_string),
            body: body.into(),
        }
    }

    #[test]
    fn a_notification_without_title_takes_the_terminals_heading() {
        let parts = |title, body| parts(&notification(title, body), "~/Repos/dango");
        assert_eq!(parts(None, "Build done"), ("~/Repos/dango".into(), "Build done".into()));
        assert_eq!(parts(Some("cargo"), "3 failed"), ("cargo".into(), "3 failed".into()));
        assert_eq!(parts(Some("Done"), ""), ("~/Repos/dango".into(), "Done".into()));
    }

    #[test]
    fn the_identifier_names_the_terminal() {
        assert_eq!(view_of(&format!("{PREFIX}42")), Some(42));
        assert_eq!(view_of("agent-42"), None);
        assert_eq!(view_of("order-7"), None);
    }
}
