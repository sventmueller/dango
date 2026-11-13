//! What the self-test reads from and does with the parts of `sidebar.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl WindowController {
    /// The context menu of `panel`'s symbol in the activity bar.
    pub fn activity_menu(&self, panel: Panel) -> Option<Retained<objc2_app_kit::NSMenu>> {
        self.ivars().activity.borrow().as_ref()?.menu_of(panel)
    }

    /// The panels whose symbol the activity bar shows.
    pub fn shown_symbols(&self) -> Vec<Panel> {
        self.ivars()
            .activity
            .borrow()
            .as_ref()
            .map(ActivityBar::shown)
            .unwrap_or_default()
    }
}
