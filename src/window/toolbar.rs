//! The toolbar of each window tab; its activity item is `crate::activity`'s. New tabs come from the tab bar's "+".

use ide_process::Kind;
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSToolbarFlexibleSpaceItemIdentifier, NSToolbarItem, NSToolbarItemIdentifier,
    NSToolbarSidebarTrackingSeparatorItemIdentifier, NSToolbarToggleSidebarItemIdentifier,
};
use objc2_foundation::NSArray;
use splat_i18n::{t, tf};

use super::WindowController;
use crate::util::{constant, ns};

const ACTIVITY: &str = "ide.activity";
const QUICK_OPEN: &str = "ide.quickOpen";

pub fn default_identifiers() -> Retained<NSArray<NSToolbarItemIdentifier>> {
    let ids = [
        constant!(NSToolbarToggleSidebarItemIdentifier).to_owned(),
        constant!(NSToolbarSidebarTrackingSeparatorItemIdentifier).to_owned(),
        ns(crate::navigation::TOOLBAR_ID),
        ns(crate::git_panel::TOOLBAR_ID),
        constant!(NSToolbarFlexibleSpaceItemIdentifier).to_owned(),
        ns(QUICK_OPEN),
        ns(ACTIVITY),
    ];
    NSArray::from_retained_slice(&ids)
}

pub fn make(controller: &WindowController, id: &str) -> Option<Retained<NSToolbarItem>> {
    match id {
        ACTIVITY => Some(crate::activity::toolbar_item(controller)),
        QUICK_OPEN => Some(quick_open(controller)),
        _ if id == crate::navigation::TOOLBAR_ID => Some(crate::navigation::toolbar_item(controller)),
        _ if id == crate::git_panel::TOOLBAR_ID => Some(crate::git_panel::toolbar_item(controller)),
        _ => None,
    }
}

/// The magnifying glass that opens Open Quickly, which otherwise only its shortcut reaches.
fn quick_open(controller: &WindowController) -> Retained<NSToolbarItem> {
    let mtm = controller.mtm();
    let item = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(QUICK_OPEN));
    item.setImage(crate::util::symbol("magnifyingglass", t("Schnell öffnen")).as_deref());
    item.setLabel(&ns(t("Schnell öffnen")));
    item.setToolTip(Some(&ns(t("Schnell öffnen"))));
    // SAFETY: Without a target the item sends its action along the responder chain, where the window answers it.
    unsafe { item.setAction(Some(objc2::sel!(ideQuickOpen:))) };
    item
}

/// What a process is there for, as the activity view names it.
pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Terminal => t("Terminal"),
        Kind::LanguageServer => t("Sprachserver"),
        Kind::Agent => t("Agent"),
        Kind::DevServer => t("Dev-Server"),
        Kind::Git => t("Git"),
        Kind::DebugAdapter => t("Debug-Adapter"),
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
