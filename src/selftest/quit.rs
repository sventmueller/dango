//! The `quit` scenario (E23): ⌘Q while an AI tab's turn runs asks first, with "Abbrechen" as the default button and
//! "Beenden" marked destructive, and "Abbrechen" keeps the app; with nothing running, quitting asks nothing.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message, sel};
use objc2_app_kit::{NSAlertFirstButtonReturn, NSApplication, NSButton, NSView};
use serde_json::json;
use splat_i18n::t;

use super::helpers::{self, picture_alert, when_alert_shows};
use super::{check, finish, give_up};
use crate::util::main_on_run_loop;

const NAME: &str = "quit";

pub(crate) fn run() {
    helpers::open_project(NAME, "quit-project", &[], |controller| {
        let root = helpers::root("quit-project");
        check(
            "with nothing running, quitting would stop nothing",
            crate::quit::running().is_empty(),
            crate::quit::running().join(", "),
        );
        let Some(tab) = ide_ui_agent::open_replay(&controller) else {
            return give_up(NAME, "an AI tab opens");
        };
        tab.feed(&turn_starts());
        tab.feed(&state("running"));
        let title = tab.title();
        check(
            "a running turn is work that quitting stops",
            crate::quit::running().contains(&title),
            crate::quit::running().join(", "),
        );
        main_on_run_loop(move || quit_while_running(root, title));
    });
}

/// What the alert in front showed, and whether the scenario clicked "Abbrechen" in it.
#[derive(Default, Debug)]
struct Seen {
    texts: Vec<String>,
    /// The button Return presses.
    default: Option<String>,
    /// The buttons marked as destructive.
    destructive: Vec<String>,
    cancelled: bool,
}

/// Chooses Quit in the app menu, as ⌘Q does, while the AI tab's turn runs; the alert's "Abbrechen" is clicked.
fn quit_while_running(root: PathBuf, title: String) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let seen = Rc::new(RefCell::new(Seen::default()));
    picture_alert("quit-alert");
    inspect_and_cancel(&seen);
    let chosen = choose_quit(mtm);
    let seen = seen.take();
    check("the app menu has Quit with ⌘Q", chosen, "");
    check(
        "⌘Q while a turn runs asks first and names the tab",
        seen.texts
            .iter()
            .any(|text| text == t("Beenden, während Agenten noch arbeiten?"))
            && seen.texts.iter().any(|text| text.contains(&title)),
        format!("{:?}", seen.texts),
    );
    check(
        "\"Abbrechen\" is the default button and \"Beenden\" the destructive one",
        seen.default.as_deref() == Some(t("Abbrechen")) && seen.destructive == [t("Beenden")],
        format!("default {:?}, destructive {:?}", seen.default, seen.destructive),
    );
    check(
        "\"Abbrechen\" keeps the app and its window",
        seen.cancelled && helpers::window_of(&root).is_some(),
        format!("cancelled {}", seen.cancelled),
    );
    helpers::wait_until(
        || MainThreadMarker::new().is_some_and(|mtm| NSApplication::sharedApplication(mtm).modalWindow().is_none()),
        move |_| after_the_turn(root),
    );
}

/// Ends the turn; then nothing runs, and quitting may go on without a question.
fn after_the_turn(root: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let tab = helpers::window_of(&root).and_then(|controller| ide_ui_agent::of_window(&controller).into_iter().next());
    if let Some(tab) = &tab {
        tab.feed(&state("idle"));
    }
    check(
        "a turn that ended is no work that quitting stops",
        tab.is_some() && crate::quit::running().is_empty(),
        crate::quit::running().join(", "),
    );
    let seen = Rc::new(RefCell::new(Seen::default()));
    let asking = inspect_and_cancel(&seen);
    let may_quit = crate::quit::agents_may_stop(mtm);
    asking.stop();
    check(
        "without a running agent, quitting asks nothing",
        may_quit && seen.borrow().texts.is_empty(),
        format!("{:?}", seen.borrow()),
    );
    finish(NAME);
}

/// Records the next alert as soon as it shows and clicks its "Abbrechen"; one without it ends with its first button,
/// so the run fails on its checks instead of hanging.
fn inspect_and_cancel(seen: &Rc<RefCell<Seen>>) -> helpers::AlertWait {
    let seen = Rc::clone(seen);
    when_alert_shows(None, move |alert| {
        let Some(content) = alert.contentView() else {
            return helpers::answer_alert(NSAlertFirstButtonReturn);
        };
        let mut buttons = Vec::new();
        buttons_in(&content, &mut buttons);
        let mut seen = seen.borrow_mut();
        seen.texts = helpers::alert_texts();
        seen.default = alert.defaultButtonCell().map(|cell| cell.title().to_string());
        seen.destructive = buttons
            .iter()
            .filter(|b| b.hasDestructiveAction())
            .map(|b| b.title().to_string())
            .collect();
        if let Some(cancel) = buttons.iter().find(|b| b.title().to_string() == t("Abbrechen")) {
            seen.cancelled = true;
            // SAFETY: A click without a sender, on the alert's own button, which the alert keeps alive.
            unsafe { cancel.performClick(None) };
            // The alert's stop takes effect with the next event.
            helpers::post_event();
        } else {
            helpers::answer_alert(NSAlertFirstButtonReturn);
        }
    })
}

fn buttons_in(view: &NSView, out: &mut Vec<Retained<NSButton>>) {
    if let Some(button) = view.downcast_ref::<NSButton>()
        && !button.title().to_string().is_empty()
    {
        out.push(button.retain());
    }
    for child in view.subviews().iter() {
        buttons_in(&child, out);
    }
}

/// Chooses the app menu's item that sends `terminate:` with ⌘Q, as the shortcut does; false when there is none.
fn choose_quit(mtm: MainThreadMarker) -> bool {
    let Some(menu) = NSApplication::sharedApplication(mtm)
        .mainMenu()
        .and_then(|main| main.itemAtIndex(0))
        .and_then(|app| app.submenu())
    else {
        return false;
    };
    let Some(index) = (0..menu.numberOfItems()).find(|&n| {
        menu.itemAtIndex(n)
            .is_some_and(|item| item.action() == Some(sel!(terminate:)) && item.keyEquivalent().to_string() == "q")
    }) else {
        return false;
    };
    menu.performActionForItemAtIndex(index);
    true
}

/// The first frame of an answer in the main conversation, which starts a turn; a state of "running" alone may be
/// background agents after the turn ended.
fn turn_starts() -> String {
    json!({
        "type": "stream_event",
        "event": {"type": "message_start", "message": {"id": "m", "type": "message", "role": "assistant", "content": []}},
        "parent_tool_use_id": null,
        "session_id": "s",
        "uuid": "u",
    })
    .to_string()
}

/// A frame that sets the session's state, as the CLI reports it.
fn state(state: &str) -> String {
    json!({"type": "system", "subtype": "session_state_changed", "state": state}).to_string()
}
