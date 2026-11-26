//! What the toolbar shows, for the self-test.

use objc2_app_kit::NSWindow;

use super::FIELD;
use crate::util::ns;

/// What the input field's switch in `window`'s toolbar shows once AppKit asked it again: its tool tip, whether it is
/// enabled and whether it shows the accent colour of a field that is on.
pub(crate) fn field_switch(window: &NSWindow) -> String {
    let Some(toolbar) = window.toolbar() else {
        return "no toolbar".into();
    };
    toolbar.validateVisibleItems();
    let id = ns(FIELD);
    let Some(item) = toolbar.items().iter().find(|item| *item.itemIdentifier() == *id) else {
        return "no switch".into();
    };
    let on = item.image().is_some_and(|image| !image.isTemplate());
    format!(
        "{} ({}{})",
        item.toolTip().map(|tip| tip.to_string()).unwrap_or_default(),
        if item.isEnabled() { "enabled" } else { "disabled" },
        if on { ", accent" } else { "" }
    )
}
