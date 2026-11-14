//! The bar below the navigation row that tells a download finished, with "Im Finder zeigen" and a close button,
//! or why a login was not filled in.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, Message, sel};
use objc2_app_kit::{
    NSAccessibility, NSBox, NSBoxType, NSColor, NSImageView, NSLayoutAttribute, NSLayoutConstraint, NSLineBreakMode,
    NSStackView, NSTextField, NSUserInterfaceLayoutOrientation, NSView, NSWorkspace,
};
use objc2_foundation::{NSArray, NSEdgeInsets, NSURL};
use splat_i18n::{t, tf};

use ide_ui_common::constant;
use ide_ui_common::util::{borderless_button, label, ns, symbol, symbol_button};

pub struct Notice {
    view: Retained<NSView>,
    icon: Retained<NSImageView>,
    text: Retained<NSTextField>,
    show: Retained<objc2_app_kit::NSButton>,
    /// Keeps the bar at no height while it is hidden.
    collapsed: Retained<NSLayoutConstraint>,
    file: RefCell<Option<Retained<NSURL>>>,
}

impl Notice {
    /// The bar, hidden; its buttons send `ideBrowserShowDownload:` and `ideBrowserCloseNotice:` to `target`.
    pub fn new(mtm: MainThreadMarker, target: &AnyObject) -> Notice {
        let icon = NSImageView::new(mtm);
        icon.setImage(symbol("arrow.down.circle", t("Download")).as_deref());
        icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let text = label(mtm, "");
        text.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
        text.setContentCompressionResistancePriority_forOrientation(
            250.0,
            objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
        );
        let show = borderless_button(mtm, t("Im Finder zeigen"), Some(target), sel!(ideBrowserShowDownload:));
        show.setContentTintColor(Some(&NSColor::controlAccentColor()));
        let close = symbol_button(
            mtm,
            "xmark",
            t("Hinweis schließen"),
            10.0,
            Some(target),
            sel!(ideBrowserCloseNotice:),
        );
        close.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setAlignment(NSLayoutAttribute::CenterY);
        row.setSpacing(ide_ui_common::style::space::M);
        row.setEdgeInsets(NSEdgeInsets {
            top: 5.0,
            left: 12.0,
            bottom: 5.0,
            right: 10.0,
        });
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, objc2_app_kit::NSLayoutConstraintOrientation::Horizontal);
        for view in [&*icon as &NSView, &text, &show, &spacer, &close] {
            row.addArrangedSubview(view);
        }
        let line = NSBox::new(mtm);
        line.setBoxType(NSBoxType::Separator);
        let view = NSView::new(mtm);
        view.setAccessibilityElement(true);
        view.setAccessibilityRole(Some(constant!(objc2_app_kit::NSAccessibilityGroupRole)));
        view.setAccessibilityLabel(Some(&ns(t("Download"))));
        for subview in [&*row as &NSView, &line] {
            view.addSubview(subview);
            subview.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        for constraint in [
            row.topAnchor().constraintEqualToAnchor(&view.topAnchor()),
            row.leadingAnchor().constraintEqualToAnchor(&view.leadingAnchor()),
            row.trailingAnchor().constraintEqualToAnchor(&view.trailingAnchor()),
            line.topAnchor().constraintEqualToAnchor(&row.bottomAnchor()),
            line.leadingAnchor().constraintEqualToAnchor(&view.leadingAnchor()),
            line.trailingAnchor().constraintEqualToAnchor(&view.trailingAnchor()),
            line.bottomAnchor().constraintEqualToAnchor(&view.bottomAnchor()),
        ] {
            constraint.setPriority(750.0);
            constraint.setActive(true);
        }
        let collapsed = view.heightAnchor().constraintEqualToConstant(0.0);
        collapsed.setActive(true);
        view.setHidden(true);
        Notice {
            view,
            icon,
            text,
            show,
            collapsed,
            file: RefCell::new(None),
        }
    }

    pub fn view(&self) -> &NSView {
        &self.view
    }

    /// Tells that `file` finished downloading.
    pub fn show_download(&self, file: &NSURL) {
        let name = file
            .lastPathComponent()
            .map_or_else(String::new, |name| name.to_string());
        self.icon
            .setImage(symbol("arrow.down.circle", t("Download")).as_deref());
        self.text.setStringValue(&ns(&tf("„{}“ geladen", &[&name])));
        self.show.setHidden(false);
        *self.file.borrow_mut() = Some(file.retain());
        self.reveal_bar(t("Download"));
    }

    /// Tells why a login was not filled into the page.
    pub fn show_login(&self, text: &str) {
        self.icon.setImage(symbol("key", t("Anmeldung")).as_deref());
        self.text.setStringValue(&ns(text));
        self.show.setHidden(true);
        *self.file.borrow_mut() = None;
        self.reveal_bar(t("Anmeldung"));
    }

    fn reveal_bar(&self, title: &str) {
        self.view.setAccessibilityLabel(Some(&ns(title)));
        self.collapsed.setActive(false);
        self.view.setHidden(false);
    }

    pub fn close(&self) {
        self.view.setHidden(true);
        self.collapsed.setActive(true);
        *self.file.borrow_mut() = None;
    }

    /// Shows the downloaded file in the Finder.
    pub fn reveal(&self) {
        if let Some(file) = self.file.borrow().as_ref() {
            NSWorkspace::sharedWorkspace()
                .activateFileViewerSelectingURLs(&NSArray::from_retained_slice(std::slice::from_ref(file)));
        }
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
