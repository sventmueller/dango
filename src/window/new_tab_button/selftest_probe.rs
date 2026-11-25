//! The self-test's hooks of `new_tab_button.rs`.

use super::*;

/// Makes the menu only recorded from now on, never shown, and forgets the one shown last.
pub(crate) fn record_only(menu: &NewTabMenu) {
    menu.record_only.set(true);
    menu.below_plus.set(None);
}

/// Whether the menu was asked for since `record_only`, and whether below the "+"; forgets it.
pub(crate) fn take_recorded(menu: &NewTabMenu) -> Option<bool> {
    menu.below_plus.take()
}
