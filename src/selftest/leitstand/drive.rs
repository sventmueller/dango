//! What the scenarios that run orders do as the user does: the items of "Ablage ▸ Auftrag", validated and chosen
//! through their target, and the alerts an order's commands ask in, read and answered.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem, NSModalResponse, NSTextField, NSTextView};

use super::super::git::{answer_sheet, sheet_ready, sheet_views};
use super::super::helpers::wait_on_within;

/// How long an alert takes to show or to go.
const ALERT: Duration = Duration::from_secs(20);

/// The item of the main menu whose action is `action`, and whose tag is `tag` where one is given.
pub(super) fn menu_item(action: Sel, tag: Option<isize>) -> Option<Retained<NSMenuItem>> {
    let menu = NSApplication::sharedApplication(MainThreadMarker::new()?).mainMenu()?;
    find(&menu, action, tag)
}

fn find(menu: &NSMenu, action: Sel, tag: Option<isize>) -> Option<Retained<NSMenuItem>> {
    menu.itemArray().iter().find_map(|item| {
        if item.action() == Some(action) && tag.is_none_or(|tag| item.tag() == tag) {
            return Some(item.clone());
        }
        item.submenu().and_then(|sub| find(&sub, action, tag))
    })
}

/// Whether AppKit would show `item` enabled, as its target answers before the menu opens.
pub(super) fn enabled(item: &NSMenuItem) -> bool {
    let Some(target) = item.target() else { return false };
    // SAFETY: The Leitstand's menu target answers validateMenuItem: with a bool for the item, which lives for the call.
    unsafe { msg_send![&*target, validateMenuItem: item] }
}

/// Chooses `item` as a click on it does.
pub(super) fn choose(item: &NSMenuItem) {
    let Some(action) = item.action() else { return };
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: The item's own action and target, with the item as the sender, as AppKit sends them.
    unsafe { app.sendAction_to_from(action, item.target().as_deref(), Some(item)) };
}

/// Calls `then` with whether an alert has laid out its buttons on the window of `root`, as soon as it has or once
/// the wait is over.
pub(super) fn when_alert(root: PathBuf, then: impl FnOnce(PathBuf, bool) + 'static) {
    wait_on_within(root, ALERT, sheet_ready, then);
}

/// Answers the alert on the window of `root` with `response`, then calls `then` with whether it was there and is
/// gone.
pub(super) fn answer(root: PathBuf, response: NSModalResponse, then: impl FnOnce(PathBuf, bool) + 'static) {
    let answered = answer_sheet(&root, response, None);
    wait_on_within(
        root,
        ALERT,
        |controller| controller.window().is_some_and(|w| w.attachedSheet().is_none()),
        move |root, gone| then(root, answered && gone),
    );
}

/// The texts of the alert on the window of `root`: its message, its information and what its text view holds.
pub(super) fn alert_texts(root: &Path) -> Vec<String> {
    sheet_views(root)
        .iter()
        .filter_map(|view| {
            (view
                .downcast_ref::<NSTextField>()
                .map(|field| field.stringValue().to_string()))
            .or_else(|| view.downcast_ref::<NSTextView>().map(|text| text.string().to_string()))
        })
        .collect()
}
