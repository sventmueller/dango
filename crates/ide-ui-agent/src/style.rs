//! The shared sizes, fonts and colours of `ide_ui_common::style`, and those only the AI tab uses.

pub use ide_ui_common::style::*;

use objc2::rc::Retained;
use objc2_app_kit::NSFont;

/// `points`, a size drawn for text of the default size `BODY`, at the text size the user chose for the AI tab.
pub(crate) fn chat(points: f64) -> f64 {
    points * crate::text_size::current() / BODY
}

/// Running text of the AI tab.
pub(crate) fn chat_body() -> Retained<NSFont> {
    NSFont::systemFontOfSize(chat(BODY))
}

/// Secondary lines of the AI tab: times, status, hints.
pub(crate) fn chat_small() -> Retained<NSFont> {
    NSFont::systemFontOfSize(chat(SMALL))
}
