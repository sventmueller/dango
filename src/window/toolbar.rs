//! The window's toolbar; its activity item is `crate::activity`'s.

use ide_process::Kind;
use objc2::rc::Retained;
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{
    NSToolbarFlexibleSpaceItemIdentifier, NSToolbarItem, NSToolbarItemIdentifier,
    NSToolbarSidebarTrackingSeparatorItemIdentifier, NSToolbarToggleSidebarItemIdentifier,
};
use objc2_foundation::NSArray;
use splat_i18n::{t, tf};

use super::WindowController;
use crate::util::{constant, ns, symbol};

const ACTIVITY: &str = "ide.activity";
const NEW_TAB: &str = "ide.newTab";

pub fn default_identifiers() -> Retained<NSArray<NSToolbarItemIdentifier>> {
    let ids = [
        constant!(NSToolbarToggleSidebarItemIdentifier).to_owned(),
        constant!(NSToolbarSidebarTrackingSeparatorItemIdentifier).to_owned(),
        constant!(NSToolbarFlexibleSpaceItemIdentifier).to_owned(),
        ns(ACTIVITY),
        ns(NEW_TAB),
    ];
    NSArray::from_retained_slice(&ids)
}

pub fn make(controller: &WindowController, id: &str) -> Option<Retained<NSToolbarItem>> {
    let mtm = controller.mtm();
    match id {
        ACTIVITY => Some(crate::activity::toolbar_item(controller)),
        NEW_TAB => {
            let entry = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(NEW_TAB));
            entry.setImage(symbol("plus", t("Neuer Tab")).as_deref());
            entry.setLabel(&ns(t("Neuer Tab")));
            entry.setToolTip(Some(&ns(t("Neuer Tab"))));
            // SAFETY: The window controller answers ideNewTab: with a method that takes the sender.
            unsafe {
                entry.setTarget(Some(controller));
                entry.setAction(Some(sel!(ideNewTab:)));
            }
            Some(entry)
        }
        _ => None,
    }
}

/// What a process is there for, as the activity view names it.
pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Terminal => t("Terminal"),
        Kind::LanguageServer => t("Sprachserver"),
        Kind::Agent => t("Agent"),
        Kind::DevServer => t("Dev-Server"),
        Kind::Git => t("Git"),
        Kind::Tool => t("Werkzeug"),
    }
}

/// A running time like `12 s`, `3 min` or `2 h`.
pub fn duration(seconds: u64) -> String {
    match seconds {
        0..60 => tf("{} s", &[&seconds.to_string()]),
        60..3600 => tf("{} min", &[&(seconds / 60).to_string()]),
        _ => tf("{} h", &[&(seconds / 3600).to_string()]),
    }
}

#[cfg(test)]
mod tests {
    use super::duration;

    #[test]
    fn running_times_read_in_the_largest_whole_unit() {
        assert_eq!(duration(5), "5 s");
        assert_eq!(duration(125), "2 min");
        assert_eq!(duration(7300), "2 h");
    }
}
