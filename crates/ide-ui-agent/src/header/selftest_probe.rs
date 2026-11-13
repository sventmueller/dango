//! What the self-test reads from and does with the parts of `header.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl Header {
    /// The Remote Control button's tooltip, while it shows.
    pub fn remote_tip(&self) -> Option<String> {
        let tip = self.remote.toolTip().map(|t| t.to_string());
        tip.filter(|_| !self.remote.isHidden())
    }

    /// The width the title takes, the width its whole text would need, and the head's width, for the self-test.
    pub fn title_widths(&self) -> (f64, f64, f64) {
        self.view.layoutSubtreeIfNeeded();
        (
            self.title.frame().size.width,
            self.title.intrinsicContentSize().width,
            self.view.frame().size.width,
        )
    }
}
