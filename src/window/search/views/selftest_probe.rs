//! What the self-test reads from and does with the parts of `views.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl Views {
    pub(in super::super::super) fn set_toggled(&self, toggle: Toggle, on: bool) {
        if let Some((_, button)) = self.toggles.iter().find(|(t, _)| *t == toggle) {
            button.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        self.paint_toggles();
    }

    pub(in super::super::super) fn results_shown(&self) -> bool {
        !self.results.isHidden()
    }
}
