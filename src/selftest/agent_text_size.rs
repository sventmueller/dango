//! The step of the `agent` scenario for the AI tab's text size from the settings.

use ide_ui_agent::AgentKey;

use super::agent::{NAME, tab_of};
use super::{check, give_up};

/// The text size the settings hand over reaches the open AI tab at once, and the default gives it back as it was.
pub(super) fn run(key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    let (font, row) = tab.text_size_seen();
    let (title, head) = tab.header_size_seen();
    ide_ui_agent::set_text_size(18.0);
    let (larger_font, larger_row) = tab.text_size_seen();
    let (larger_title, larger_head) = tab.header_size_seen();
    ide_ui_agent::set_text_size(ide_ui_agent::CHAT_FONT_SIZE.default_value());
    let (back_font, back_row) = tab.text_size_seen();
    let back_head = tab.header_size_seen();
    check(
        "a larger text size reaches the open AI tab's head at once, and the default gives it back",
        same_size(larger_title, 18.0)
            && larger_head > head + 4.0
            && same_size(back_head.0, title)
            && same_size(back_head.1, head),
        format!("{title} pt, {head} high → {larger_title} pt, {larger_head} → {back_head:?}"),
    );
    check(
        "a larger text size reaches the open AI tab's composer and cards at once",
        same_size(larger_font, 18.0) && larger_row > row + 4.0,
        format!("{font} pt, last row {row:.1} → {larger_font} pt, {larger_row:.1}"),
    );
    check(
        "the default text size gives the AI tab back as it was",
        same_size(back_font, font) && same_size(back_row, row),
        format!("{font} pt, last row {row:.1} → {back_font} pt, {back_row:.1}"),
    );
}

fn same_size(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0
}
