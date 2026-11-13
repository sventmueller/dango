//! The welcome tab: what to open, and the folders opened recently.

use std::path::PathBuf;

use ide_core::settings::{State, remembered};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSApplication, NSBezelStyle, NSButton, NSColor, NSFont, NSFontWeightSemibold, NSImageView, NSLayoutAttribute,
    NSStackView, NSUserInterfaceLayoutOrientation, NSView,
};
use splat_i18n::t;

use super::WindowController;
use crate::util::{borderless_button, label, ns};

/// How many recent folders the welcome tab lists.
const SHOWN_RECENT: usize = 8;

/// The edge length of the app icon above the title.
const ICON_SIZE: f64 = 72.0;

/// The recent folders that still exist, as the welcome tab lists them.
pub fn recent_folders() -> Vec<PathBuf> {
    State::load_user()
        .get(&remembered::RECENT_PROJECTS)
        .into_iter()
        .filter(|p| p.is_dir())
        .take(SHOWN_RECENT)
        .collect()
}

pub fn view(mtm: MainThreadMarker, target: &WindowController) -> Retained<NSView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::Leading);
    stack.setSpacing(ide_ui_common::style::space::L);

    if let Some(icon) = NSApplication::sharedApplication(mtm).applicationIconImage() {
        let image = NSImageView::imageViewWithImage(&icon, mtm);
        crate::util::hide_from_voiceover(&image);
        image.setTranslatesAutoresizingMaskIntoConstraints(false);
        // macOS app icons keep 100 of their 1024 points clear on each side, so the visible tile starts further in.
        let margin = ICON_SIZE * 100.0 / 1024.0;
        let frame = NSView::new(mtm);
        frame.setTranslatesAutoresizingMaskIntoConstraints(false);
        frame.addSubview(&image);
        for constraint in [
            image.widthAnchor().constraintEqualToConstant(ICON_SIZE),
            image.heightAnchor().constraintEqualToConstant(ICON_SIZE),
            image.topAnchor().constraintEqualToAnchor(&frame.topAnchor()),
            image.bottomAnchor().constraintEqualToAnchor(&frame.bottomAnchor()),
            image
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&frame.leadingAnchor(), -margin),
            frame.widthAnchor().constraintEqualToConstant(ICON_SIZE - margin),
        ] {
            constraint.setActive(true);
        }
        stack.addArrangedSubview(&frame);
    }
    let title = label(mtm, ide_product::NAME);
    // SAFETY: AppKit sets its font weight constants when it loads.
    title.setFont(Some(&NSFont::systemFontOfSize_weight(
        ide_ui_common::style::DISPLAY,
        unsafe { NSFontWeightSemibold },
    )));
    stack.addArrangedSubview(&title);
    let tagline = label(mtm, t("Code, Terminal und Agenten in einem Fenster."));
    tagline.setTextColor(Some(&NSColor::secondaryLabelColor()));
    stack.addArrangedSubview(&tagline);
    stack.setCustomSpacing_afterView(ide_ui_common::style::space::GROUP, &tagline);

    // SAFETY: A nil target sends the action along the responder chain, which ends at the app delegate that answers it.
    let open = unsafe {
        NSButton::buttonWithTitle_target_action(&ns(t("Ordner öffnen …")), None, Some(sel!(ideOpenFolder:)), mtm)
    };
    open.setBezelStyle(NSBezelStyle::Push);
    open.setKeyEquivalent(&ns("\r"));
    stack.addArrangedSubview(&open);

    let recent = recent_folders();
    if !recent.is_empty() {
        let heading = label(mtm, t("Zuletzt geöffnet"));
        heading.setFont(Some(&NSFont::boldSystemFontOfSize(ide_ui_common::style::HEADING)));
        stack.addArrangedSubview(&heading);
        stack.setCustomSpacing_afterView(ide_ui_common::style::space::GROUP, &open);
        for (index, folder) in recent.iter().enumerate() {
            let entry = borderless_button(
                mtm,
                &crate::app::display_path(folder),
                Some(target),
                sel!(ideOpenWelcomeRecent:),
            );
            entry.setTag(isize::try_from(index).unwrap_or(-1));
            entry.setContentTintColor(Some(&NSColor::linkColor()));
            stack.addArrangedSubview(&entry);
        }
    }

    let view = NSView::new(mtm);
    view.addSubview(&stack);
    stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        stack.centerXAnchor().constraintEqualToAnchor(&view.centerXAnchor()),
        stack
            .centerYAnchor()
            .constraintEqualToAnchor_constant(&view.centerYAnchor(), -30.0),
        stack
            .leadingAnchor()
            .constraintGreaterThanOrEqualToAnchor_constant(&view.leadingAnchor(), 24.0),
    ] {
        constraint.setActive(true);
    }
    view
}
