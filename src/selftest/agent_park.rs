//! An AI tab out of view lets go of its cards and builds those in view again when it shows.

use ide_ui_agent::{AgentKey, AgentTab};

use super::agent::tab_of;
use super::{check, helpers};

/// `tab`, hidden behind another tab, lets go of its cards as it does once it stayed out of view.
pub(super) fn lets_go(tab: &AgentTab) {
    let rows = tab.timeline().rows_in_table();
    tab.timeline().park_now();
    let left = tab.timeline().rows_in_table();
    check(
        "an AI tab out of view lets go of its cards",
        rows > 0 && left == 0,
        format!("{rows} rows before, {left} after"),
    );
}

/// The tab `key`, shown again, has its rows and builds the cards in view, then `then` runs.
pub(super) fn builds_again(key: AgentKey, then: impl FnOnce() + 'static) {
    let shown = move || {
        tab_of(key).is_some_and(|tab| {
            tab.timeline().rows_in_table() == tab.timeline().count() && !tab.inspect().visible.is_empty()
        })
    };
    helpers::wait_until(shown, move |ok| {
        let (rows, count, cards) = tab_of(key).map_or((0, 0, 0), |tab| {
            (
                tab.timeline().rows_in_table(),
                tab.timeline().count(),
                tab.inspect().visible.len(),
            )
        });
        check(
            "an AI tab shown again builds the cards in view",
            ok,
            format!("{rows} of {count} rows in the table, {cards} cards in view"),
        );
        then();
    });
}
