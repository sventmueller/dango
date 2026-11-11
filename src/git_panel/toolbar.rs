//! The branch beside the window's title: a pull-down with what the Git panel's branch popup offers, hidden where git
//! does not run.

use std::cell::RefCell;

use objc2::rc::{Retained, Weak};
use objc2::runtime::NSObjectProtocol;
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{NSBezelStyle, NSMenuItem, NSPopUpButton, NSToolbarItem};
use objc2_foundation::{NSCopying, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::panel::{GitPanel, Phase};
use crate::util::{ns, symbol};
use crate::window::WindowController;

pub const TOOLBAR_ID: &str = "ide.branch";

struct Shown {
    item: Weak<NSToolbarItem>,
    popup: Weak<NSPopUpButton>,
    controller: Weak<WindowController>,
}

thread_local! {
    static SHOWN: RefCell<Vec<Shown>> = const { RefCell::new(Vec::new()) };
}

/// The toolbar item of `controller`'s window, filled from its Git panel once that read the repository.
pub fn toolbar_item(controller: &WindowController) -> Retained<NSToolbarItem> {
    let mtm = controller.mtm();
    let item = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(TOOLBAR_ID));
    let popup = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(120.0, 24.0)),
        true,
    );
    popup.setBezelStyle(NSBezelStyle::Toolbar);
    crate::util::set_label(&popup, Some(t("Branch")));
    popup.setToolTip(Some(&ns(t("Branch wechseln oder anlegen"))));
    item.setView(Some(&popup));
    item.setLabel(&ns(t("Branch")));
    item.setToolTip(Some(&ns(t("Branch wechseln oder anlegen"))));
    SHOWN.with(|shown| {
        shown.borrow_mut().push(Shown {
            item: Weak::from(&*item),
            popup: Weak::from(&*popup),
            controller: Weak::from(controller),
        });
    });
    match super::panel::of(controller) {
        Some(panel) => panel.refresh_toolbar(),
        None => set_hidden(&item, true),
    }
    item
}

/// Hides `item` where macOS can hide a toolbar item, else disables it.
fn set_hidden(item: &NSToolbarItem, hidden: bool) {
    if item.respondsToSelector(sel!(setHidden:)) {
        item.setHidden(hidden);
    } else {
        item.setEnabled(!hidden);
    }
}

impl GitPanel {
    /// Puts the branch and the branch popup's choices into the toolbar items of the panel's window.
    pub fn refresh_toolbar(&self) {
        let Some(controller) = self.controller() else { return };
        let shown: Vec<(Retained<NSToolbarItem>, Retained<NSPopUpButton>)> = SHOWN.with(|shown| {
            let mut shown = shown.borrow_mut();
            shown.retain(|s| s.item.load().is_some() && s.controller.load().is_some());
            shown
                .iter()
                .filter(|s| s.controller.load().is_some_and(|c| std::ptr::eq(&*c, &*controller)))
                .filter_map(|s| Some((s.item.load()?, s.popup.load()?)))
                .collect()
        });
        if shown.is_empty() {
            return;
        }
        let (ready, title) = {
            let state = self.read();
            let title = match &state.branch.head {
                Some(head) => head.clone(),
                None => {
                    let short: String = state.branch.commit.as_deref().unwrap_or("").chars().take(8).collect();
                    tf("Losgelöst bei {}", &[&short])
                }
            };
            (state.phase == Phase::Ready, title)
        };
        let choices: Vec<Retained<NSMenuItem>> = if ready {
            self.ui().branch.itemArray().to_vec()
        } else {
            Vec::new()
        };
        for (item, popup) in shown {
            set_hidden(&item, !ready);
            if !ready {
                continue;
            }
            popup.removeAllItems();
            let Some(menu) = popup.menu() else { continue };
            let head = NSMenuItem::new(self.mtm());
            head.setTitle(&ns(&title));
            head.setImage(symbol("arrow.triangle.branch", t("Branch")).as_deref());
            menu.addItem(&head);
            for choice in &choices {
                let copy: Retained<NSMenuItem> = choice.copy();
                menu.addItem(&copy);
            }
            crate::util::set_label(&popup, Some(&tf("{}: {}", &[t("Branch"), &title])));
        }
    }
}

/// What the toolbar's branch item of `controller` shows: its title and its choices, `None` while it is hidden.
#[cfg(feature = "selftest")]
pub fn seen(controller: &WindowController) -> Vec<Option<(String, Vec<String>)>> {
    SHOWN.with(|shown| {
        shown
            .borrow()
            .iter()
            .filter(|s| s.controller.load().is_some_and(|c| std::ptr::eq(&*c, controller)))
            .filter_map(|s| Some((s.item.load()?, s.popup.load()?)))
            .map(|(item, popup)| {
                let hidden = if item.respondsToSelector(sel!(isHidden)) {
                    item.isHidden()
                } else {
                    !item.isEnabled()
                };
                (!hidden).then(|| {
                    let titles: Vec<String> = popup.itemArray().iter().map(|i| i.title().to_string()).collect();
                    (
                        titles.first().cloned().unwrap_or_default(),
                        titles.into_iter().skip(1).collect(),
                    )
                })
            })
            .collect()
    })
}

/// Chooses the entry `title` of the toolbar's branch item of `controller`, as a click on it does; whether it is there.
#[cfg(feature = "selftest")]
pub fn choose(controller: &WindowController, title: &str) -> bool {
    let popup = SHOWN.with(|shown| {
        shown
            .borrow()
            .iter()
            .filter(|s| s.controller.load().is_some_and(|c| std::ptr::eq(&*c, controller)))
            .find_map(|s| s.popup.load())
    });
    let Some(menu) = popup.and_then(|popup| popup.menu()) else {
        return false;
    };
    let found = menu
        .itemArray()
        .iter()
        .skip(1)
        .position(|item| item.title().to_string() == title)
        .map(|index| index + 1);
    if let Some(index) = found.and_then(|i| isize::try_from(i).ok()) {
        menu.performActionForItemAtIndex(index);
    }
    found.is_some()
}
