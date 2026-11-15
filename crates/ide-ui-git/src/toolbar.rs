//! The branch beside the window's title: a pull-down with what the Git panel's branch popup offers, hidden where git
//! does not run.

use objc2::rc::{Retained, Weak};
use objc2::runtime::NSObjectProtocol;
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{NSBezelStyle, NSMenuItem, NSPopUpButton, NSToolbarItem};
use objc2_foundation::{NSCopying, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::model::Phase;
use super::panel::GitPanel;
use ide_ui_common::util::{ns, symbol};
use ide_ui_common::window::{AsWindow, Window};

pub const TOOLBAR_ID: &str = "ide.branch";

pub(super) struct Shown {
    item: Weak<NSToolbarItem>,
    popup: Weak<NSPopUpButton>,
    controller: Window,
}

/// The toolbar item of `controller`'s window, filled from its Git panel once that read the repository.
pub fn toolbar_item(controller: &(impl AsWindow + ?Sized)) -> Retained<NSToolbarItem> {
    let controller = &controller.as_window();
    let mtm = controller.mtm();
    let item = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(TOOLBAR_ID));
    let popup = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(120.0, 24.0)),
        true,
    );
    popup.setBezelStyle(NSBezelStyle::Toolbar);
    ide_ui_common::util::set_label(&popup, Some(t("Branch")));
    popup.setToolTip(Some(&ns(t("Branch wechseln oder anlegen"))));
    item.setView(Some(&popup));
    item.setLabel(&ns(t("Branch")));
    item.setToolTip(Some(&ns(t("Branch wechseln oder anlegen"))));
    crate::host::state().toolbar_items.borrow_mut().push(Shown {
        item: Weak::from(&*item),
        popup: Weak::from(&*popup),
        controller: *controller,
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
    pub(crate) fn refresh_toolbar(&self) {
        let Some(controller) = self.controller() else { return };
        let shown: Vec<(Retained<NSToolbarItem>, Retained<NSPopUpButton>)> = {
            let state = crate::host::state();
            let mut shown = state.toolbar_items.borrow_mut();
            shown.retain(|s| s.item.load().is_some() && s.controller.is_open());
            shown
                .iter()
                .filter(|s| s.controller == controller)
                .filter_map(|s| Some((s.item.load()?, s.popup.load()?)))
                .collect()
        };
        if shown.is_empty() {
            return;
        }
        let (ready, title) = {
            let state = self.read();
            let title = match &state.branch.head {
                Some(head) => head.clone(),
                None => {
                    let short = state.branch.commit.as_ref().map_or("", ide_git::CommitId::short);
                    tf("Losgelöst bei {}", &[short])
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
            ide_ui_common::util::set_label(&popup, Some(&tf("{}: {}", &[t("Branch"), &title])));
        }
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
