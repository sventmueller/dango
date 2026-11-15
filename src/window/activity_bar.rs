//! The column of symbols at the left edge that picks the panel beside it; its context menu shows and hides each
//! symbol, for every window and the next start.

use ide_core::settings::{Settings, keys};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSLayoutAttribute, NSMenu, NSStackView, NSUserInterfaceLayoutOrientation,
    NSView,
};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::{t, tf};

use super::{Panel, WindowController};
use crate::util::{activate, auto_layout, fixed_size, ns, set_target, symbol_button, tagged};
#[cfg(feature = "selftest")]
mod selftest_probe;

const BUTTON_SIZE: f64 = 32.0;

pub(crate) struct ActivityBar {
    view: Retained<NSView>,
    buttons: Vec<(Panel, Retained<NSButton>)>,
}

impl ActivityBar {
    pub(crate) fn new(mtm: MainThreadMarker, target: &WindowController) -> ActivityBar {
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(NSLayoutAttribute::CenterX);
        stack.setSpacing(ide_ui_common::style::space::XS);
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
                ide_ui_common::style::symbol::LARGE,
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
            ide_ui_common::style::symbol::LARGE,
            None,
            sel!(ideShowSettings:),
        );
        sized(&settings);
        settings.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let menu = symbols_menu(mtm, target);
        for button in buttons.iter().map(|(_, button)| button).chain([&settings]) {
            // SAFETY: The button keeps the menu, whose items hold the window controller weakly.
            unsafe { button.setMenu(Some(&menu)) };
        }

        let view = NSView::new(mtm);
        view.setAccessibilityLabel(Some(&crate::util::ns(t("Seitenleisten-Auswahl"))));
        // SAFETY: The view keeps the menu, whose items hold the window controller weakly.
        unsafe { view.setMenu(Some(&menu)) };
        view.addSubview(&stack);
        view.addSubview(&settings);
        auto_layout(&[&stack, &settings]);
        activate([
            stack.topAnchor().constraintEqualToAnchor(&view.topAnchor()),
            stack.centerXAnchor().constraintEqualToAnchor(&view.centerXAnchor()),
            settings.centerXAnchor().constraintEqualToAnchor(&view.centerXAnchor()),
            settings
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&view.bottomAnchor(), -10.0),
        ]);
        let bar = ActivityBar { view, buttons };
        bar.leave_out(&hidden());
        bar
    }

    /// Hides the symbols of `hidden` and shows the others.
    pub(crate) fn leave_out(&self, hidden: &[Panel]) {
        for (panel, button) in &self.buttons {
            button.setHidden(hidden.contains(panel));
        }
    }

    pub(crate) fn view(&self) -> &NSView {
        &self.view
    }

    /// Marks `panel` as the one showing; `None` when the panel is closed.
    pub(crate) fn select(&self, panel: Option<Panel>) {
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
    pub(crate) fn set_status(&self, panel: Panel, words: Option<&str>) {
        let description = match words {
            Some(words) => tf("{}: {}", &[panel.title(), words]),
            None => panel.title().to_string(),
        };
        self.describe(panel, &description);
    }

    /// Names `count` changes with the label and tooltip of `panel`'s symbol.
    pub(crate) fn set_count(&self, panel: Panel, count: usize) {
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
}

/// The panels whose symbol the user hid, in the order of the bar.
pub(crate) fn hidden() -> Vec<Panel> {
    let ids = Settings::load_user().get(&keys::HIDDEN_PANELS);
    Panel::ALL
        .into_iter()
        .filter(|panel| ids.iter().any(|id| id == panel.id()))
        .collect()
}

/// Remembers `hidden` as the panels whose symbol stays out.
pub(crate) fn store_hidden(hidden: &[Panel]) {
    let ids: Vec<String> = hidden.iter().map(|panel| panel.id().to_string()).collect();
    if let Err(error) = Settings::load_user().set(&keys::HIDDEN_PANELS, &ids) {
        crate::diag::report("saving the hidden sidebar symbols", error);
    }
}

/// One item per panel, ticked while its symbol shows; the window controller validates and answers them.
fn symbols_menu(mtm: MainThreadMarker, target: &WindowController) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    for panel in Panel::ALL {
        let item = tagged(mtm, panel.title(), sel!(ideToggleActivitySymbol:), "", panel.tag());
        set_target(&item, target);
        menu.addItem(&item);
    }
    menu
}

fn sized(button: &NSButton) {
    button.setTranslatesAutoresizingMaskIntoConstraints(false);
    fixed_size(button, BUTTON_SIZE, BUTTON_SIZE);
}
