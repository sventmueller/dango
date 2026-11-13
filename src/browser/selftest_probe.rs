//! What the self-test reads from and does with the parts of `browser.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

pub use ide_ui_browser::{Seen, answer_asking};

/// Presses "Trotzdem öffnen" on the first tab that offers it; whether one did.
pub fn proceed(controller: &WindowController) -> bool {
    ide_ui_browser::proceed(controller.id())
}

/// Presses the key button of the browser tab in front, as the self-test's user.
pub fn fill_login(controller: &WindowController) {
    ide_ui_browser::fill_login(controller.id());
}

pub fn inspect(controller: &WindowController) -> Vec<Seen> {
    ide_ui_browser::inspect(controller.id())
}
