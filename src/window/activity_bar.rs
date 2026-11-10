//! The column of symbols at the left edge that picks the panel beside it.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSBox, NSBoxType, NSButton, NSColor, NSFont, NSLayoutAttribute, NSStackView, NSTextAlignment,
    NSTextField, NSTitlePosition, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSSize};
use splat_i18n::{t, tf};

use super::{Panel, WindowController};
use crate::util::{label, ns, symbol_button};

const BUTTON_SIZE: f64 = 32.0;
const SYMBOL_POINTS: f64 = 16.0;
const BADGE_HEIGHT: f64 = 15.0;

pub struct ActivityBar {
    view: Retained<NSView>,
    buttons: Vec<(Panel, Retained<NSButton>)>,
    badges: Vec<(Panel, Badge)>,
}

/// The number on a panel's symbol.
struct Badge {
    shape: Retained<BadgeShape>,
    count: Retained<NSTextField>,
}

define_class!(
    // SAFETY: NSBox has no subclassing requirements and `BadgeShape` does not implement Drop.
    #[unsafe(super(NSBox))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEBadgeShape"]
    struct BadgeShape;

    unsafe impl NSObjectProtocol for BadgeShape {}

    impl BadgeShape {
        /// Lets clicks through to the button below.
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> Option<Retained<NSView>> {
            None
        }
    }
);

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
        let mut badges = vec![];
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
            badges.push((panel, Badge::new(mtm, &button)));
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
        ActivityBar { view, buttons, badges }
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

    /// Shows `count` on the symbol of `panel` and tells VoiceOver with the symbol's label; 0 hides it.
    /// Shows a dot in `color` on the symbol of `panel`, and `words` to VoiceOver with the symbol's label; `None`
    /// hides it. A count replaces it.
    pub fn set_status(&self, panel: Panel, status: Option<(&NSColor, &str)>) {
        let Some((_, badge)) = self.badges.iter().find(|(p, _)| *p == panel) else {
            return;
        };
        badge.count.setStringValue(&ns(""));
        badge.shape.setHidden(status.is_none());
        let description = match status {
            Some((color, words)) => {
                badge.shape.setFillColor(color);
                tf("{}: {}", &[panel.title(), words])
            }
            None => panel.title().to_string(),
        };
        if let Some((_, button)) = self.buttons.iter().find(|(p, _)| *p == panel) {
            crate::util::set_label(button, Some(&description));
            button.setToolTip(Some(&ns(&description)));
        }
    }

    pub fn set_badge(&self, panel: Panel, count: usize) {
        let Some((_, badge)) = self.badges.iter().find(|(p, _)| *p == panel) else {
            return;
        };
        badge.shape.setFillColor(&NSColor::controlAccentColor());
        let text = if count > 99 {
            "99+".to_string()
        } else {
            count.to_string()
        };
        badge.count.setStringValue(&ns(&text));
        badge.shape.setHidden(count == 0);
        let description = match count {
            0 => panel.title().to_string(),
            1 => tf("{}, 1 Änderung", &[panel.title()]),
            n => tf("{}, {} Änderungen", &[panel.title(), &n.to_string()]),
        };
        if let Some((_, button)) = self.buttons.iter().find(|(p, _)| *p == panel) {
            crate::util::set_label(button, Some(&description));
            button.setToolTip(Some(&ns(&description)));
        }
    }

    #[cfg(feature = "selftest")]
    pub fn badge(&self, panel: Panel) -> Option<String> {
        let (_, badge) = self.badges.iter().find(|(p, _)| *p == panel)?;
        (!badge.shape.isHidden()).then(|| badge.count.stringValue().to_string())
    }

    #[cfg(feature = "selftest")]
    pub fn button_count(&self) -> usize {
        self.buttons.len() + 1
    }
}

impl Badge {
    /// A hidden badge on the top trailing corner of `button`; the button's label speaks for it.
    fn new(mtm: MainThreadMarker, button: &NSButton) -> Badge {
        let count = label(mtm, "");
        count.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
            9.0,
            // SAFETY: AppKit sets its font weight constants when it loads.
            unsafe { objc2_app_kit::NSFontWeightSemibold },
        )));
        count.setTextColor(Some(&NSColor::whiteColor()));
        count.setAlignment(NSTextAlignment::Center);
        // SAFETY: NSBox's designated initializer, called once on the newly allocated object.
        let shape: Retained<BadgeShape> = unsafe { msg_send![BadgeShape::alloc(mtm), init] };
        shape.setBoxType(NSBoxType::Custom);
        shape.setTitlePosition(NSTitlePosition::NoTitle);
        shape.setBorderWidth(0.0);
        shape.setCornerRadius(BADGE_HEIGHT / 2.0);
        shape.setFillColor(&NSColor::controlAccentColor());
        shape.setContentViewMargins(NSSize::new(0.0, 0.0));
        shape.setAccessibilityElement(false);
        crate::util::hide_from_voiceover(&count);
        shape.setHidden(true);
        if let Some(content) = shape.contentView() {
            content.addSubview(&count);
            count.setTranslatesAutoresizingMaskIntoConstraints(false);
            for constraint in [
                count
                    .leadingAnchor()
                    .constraintEqualToAnchor_constant(&content.leadingAnchor(), 3.0),
                count
                    .trailingAnchor()
                    .constraintEqualToAnchor_constant(&content.trailingAnchor(), -3.0),
                count.centerYAnchor().constraintEqualToAnchor(&content.centerYAnchor()),
            ] {
                constraint.setActive(true);
            }
        }
        button.addSubview(&shape);
        shape.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            shape.heightAnchor().constraintEqualToConstant(BADGE_HEIGHT),
            shape.widthAnchor().constraintGreaterThanOrEqualToConstant(BADGE_HEIGHT),
            shape
                .centerXAnchor()
                .constraintEqualToAnchor_constant(&button.trailingAnchor(), -5.0),
            shape
                .centerYAnchor()
                .constraintEqualToAnchor_constant(&button.topAnchor(), 8.0),
        ] {
            constraint.setActive(true);
        }
        Badge { shape, count }
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
