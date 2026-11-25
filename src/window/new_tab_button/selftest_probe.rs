//! The self-test's hooks of `new_tab_button.rs`.

use super::*;

/// Makes the menu only recorded from now on, never shown, and forgets the one shown last.
pub(crate) fn record_only() {
    with_record_only(|record| record.set(true));
    with_below_plus(|below| below.set(None));
}

/// Whether the menu was asked for since `record_only`, and whether below the "+"; forgets it.
pub(crate) fn take_recorded() -> Option<bool> {
    with_below_plus(Cell::take)
}
