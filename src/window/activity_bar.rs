//! The column of symbols at the left edge that picks the panel beside it.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSLayoutAttribute, NSStackView, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::{t, tf};

use super::{Panel, WindowController};
use crate::util::{ns, symbol_button};

const BUTTON_SIZE: f64 = 32.0;
const SYMBOL_POINTS: f64 = 16.0;

pub struct ActivityBar {
    view: Retained<NSView>,
    buttons: Vec<(Panel, Retained<NSButton>)>,
}

impl ActivityBar {
    pub fn new(mtm: MainThreadMarker, target: &WindowController) -> ActivityBar {
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(NSLayoutAttribute::CenterX);
        stack.setSpacing(4.0);
        stack.setEdgeInsets(NSEdgeInsets {
            top: 6.0,
            left: 0.0,
            bottom: 0.0,
            right: 0.0,
        });
        let mut buttons = vec![];
        for panel in Panel::ALL {
            let button = symbol_button(
                mtm,
                panel.symbol(),
                panel.title(),
                SYMBOL_POINTS,
                Some(target),
                sel!(ideShowPanel:),
            );
            button.setTag(panel.tag());
            sized(&button);
            stack.addArrangedSubview(&button);
            buttons.push((panel, button));
        }
        let settings = symbol_button(
            mtm,
            "gearshape",
            t("Einstellungen"),
            SYMBOL_POINTS,
            None,
            sel!(ideShowSettings:),
        );
        sized(&settings);
        settings.setContentTintColor(Some(&NSColor::secondaryLabelColor()));

        let view = NSView::new(mtm);
        view.setAccessibilityLabel(Some(&crate::util::ns(t("Seitenleisten-Auswahl"))));
        view.addSubview(&stack);
        view.addSubview(&settings);
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        settings.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            stack.topAnchor().constraintEqualToAnchor(&view.topAnchor()),
            stack.centerXAnchor().constraintEqualToAnchor(&view.centerXAnchor()),
            settings.centerXAnchor().constraintEqualToAnchor(&view.centerXAnchor()),
            settings
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&view.bottomAnchor(), -10.0),
        ] {
            constraint.setActive(true);
        }
        ActivityBar { view, buttons }
    }

    pub fn view(&self) -> &NSView {
        &self.view
    }

    /// Marks `panel` as the one showing; `None` when the panel is closed.
    pub fn select(&self, panel: Option<Panel>) {
        for (candidate, button) in &self.buttons {
            let selected = Some(*candidate) == panel;
            let color = if selected {
                NSColor::controlAccentColor()
            } else {
                NSColor::secondaryLabelColor()
            };
            button.setContentTintColor(Some(&color));
            button.setAccessibilitySelected(selected);
        }
    }

    /// Names `words`, such as a failed CI run, with the label and tooltip of `panel`'s symbol; `None` names only the
    /// panel.
    pub fn set_status(&self, panel: Panel, words: Option<&str>) {
        let description = match words {
            Some(words) => tf("{}: {}", &[panel.title(), words]),
            None => panel.title().to_string(),
        };
        self.describe(panel, &description);
    }

    /// Names `count` changes with the label and tooltip of `panel`'s symbol.
    pub fn set_count(&self, panel: Panel, count: usize) {
        let description = match count {
            0 => panel.title().to_string(),
            1 => tf("{}, 1 Änderung", &[panel.title()]),
            n => tf("{}, {} Änderungen", &[panel.title(), &n.to_string()]),
        };
        self.describe(panel, &description);
    }

    fn describe(&self, panel: Panel, description: &str) {
        if let Some((_, button)) = self.buttons.iter().find(|(p, _)| *p == panel) {
            crate::util::set_label(button, Some(description));
            button.setToolTip(Some(&ns(description)));
        }
    }

    /// The tooltip of `panel`'s symbol.
    #[cfg(feature = "selftest")]
    pub fn tip(&self, panel: Panel) -> Option<String> {
        let (_, button) = self.buttons.iter().find(|(p, _)| *p == panel)?;
        button.toolTip().map(|tip| tip.to_string())
    }

    /// How many marks show on the symbols.
    #[cfg(feature = "selftest")]
    pub fn marks(&self) -> usize {
        self.buttons
            .iter()
            .flat_map(|(_, button)| button.subviews().to_vec())
            .filter(|view| view.downcast_ref::<objc2_app_kit::NSBox>().is_some() && !view.isHidden())
            .count()
    }

    #[cfg(feature = "selftest")]
    pub fn button_count(&self) -> usize {
        self.buttons.len() + 1
    }
}

fn sized(button: &NSButton) {
    button.setTranslatesAutoresizingMaskIntoConstraints(false);
    button
        .widthAnchor()
        .constraintEqualToConstant(BUTTON_SIZE)
        .setActive(true);
    button
        .heightAnchor()
        .constraintEqualToConstant(BUTTON_SIZE)
        .setActive(true);
}
