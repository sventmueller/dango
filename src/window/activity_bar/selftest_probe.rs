//! What the self-test reads from and does with the parts of `activity_bar.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl ActivityBar {
    /// The tooltip of `panel`'s symbol.
    pub(crate) fn tip(&self, panel: Panel) -> Option<String> {
        let (_, button) = self.buttons.iter().find(|(p, _)| *p == panel)?;
        button.toolTip().map(|tip| tip.to_string())
    }

    /// How many marks show on the symbols.
    pub(crate) fn marks(&self) -> usize {
        self.buttons
            .iter()
            .flat_map(|(_, button)| button.subviews().to_vec())
            .filter(|view| view.downcast_ref::<objc2_app_kit::NSBox>().is_some() && !view.isHidden())
            .count()
    }

    /// The context menu of `panel`'s symbol.
    pub(crate) fn menu_of(&self, panel: Panel) -> Option<Retained<NSMenu>> {
        let (_, button) = self.buttons.iter().find(|(p, _)| *p == panel)?;
        button.menu()
    }

    /// The panels whose symbol shows.
    pub(crate) fn shown(&self) -> Vec<Panel> {
        self.buttons
            .iter()
            .filter(|(_, button)| !button.isHidden())
            .map(|(panel, _)| *panel)
            .collect()
    }

    pub(crate) fn button_count(&self) -> usize {
        self.buttons.len() + 1
    }
}
