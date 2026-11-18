//! The self-test's hooks of `dictation.rs`.

use super::*;

/// The keys the tab shows, for the self-test: one line per provider.
pub(crate) fn key_lines(settings: &DictationSettings) -> Vec<String> {
    settings
        .ivars()
        .views
        .get()
        .map(|views| {
            views
                .keys
                .iter()
                .map(|row| row.state.stringValue().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Whether the tab offers Lowspeak's choices and shows a missing model's download, for the self-test.
pub(crate) fn rows_shown(settings: &DictationSettings) -> (bool, bool) {
    settings
        .ivars()
        .views
        .get()
        .map(|views| {
            let shown = |row: usize| !views.grid.rowAtIndex(tag(row)).isHidden();
            (shown(OFFER_ROWS[0]), shown(MODEL_ROW))
        })
        .unwrap_or_default()
}
