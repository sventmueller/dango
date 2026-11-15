//! What the self-test reads from and does with the parts of `inspector.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl WindowController {
    /// Whether the inspector shows in the tab in front.
    pub(crate) fn inspector_shown(&self) -> bool {
        self.current_tab().is_some_and(|tab| tab.inspector_shown())
    }

    /// Whether the inspector shows in each tab, the one in front marked, for the self-test.
    pub(crate) fn inspector_states(&self) -> Vec<(bool, bool)> {
        let current = self.current_tab();
        self.open_tabs()
            .iter()
            .map(|tab| {
                (
                    tab.inspector_shown(),
                    current.as_ref().is_some_and(|c| std::rc::Rc::ptr_eq(c, tab)),
                )
            })
            .collect()
    }

    /// The inspector's width in the tab in front.
    pub(crate) fn inspector_width(&self) -> f64 {
        self.current_tab().map_or(0.0, |tab| tab.inspector_width())
    }

    /// Shows the inspector `width` wide in the tab in front, as a drag of its edge leaves it.
    pub(crate) fn set_inspector_width(&self, width: f64) {
        self.ivars().inspector.width.set(width.clamp(MIN_WIDTH, MAX_WIDTH));
        if let Some(tab) = self.current_tab() {
            tab.set_inspector(true, self.ivars().inspector.width.get());
        }
        self.later(|controller| {
            if let Some(tab) = controller.current_tab() {
                tab.release_inspector();
            }
        });
    }
}
