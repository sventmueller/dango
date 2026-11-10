//! The row above the folder tree, as VS Code's explorer has it: the folder's name and small buttons for a new file and
//! a new folder, created in the selected folder or the root, and for folding every folder.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSFont, NSLayoutConstraintOrientation, NSLineBreakMode, NSStackView,
    NSStackViewGravity, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::t;

use crate::util::{label, symbol_button};

const HEIGHT: f64 = 22.0;
const SYMBOL_POINTS: f64 = 12.0;

/// The header row with `name` above `tree`, its buttons sending their actions to `target`, the tree's data source.
pub fn page(mtm: MainThreadMarker, name: &str, target: &AnyObject, tree: &NSView) -> Retained<NSView> {
    let header = header(mtm, name, target);
    let page = NSView::new(mtm);
    page.addSubview(&header);
    page.addSubview(tree);
    header.setTranslatesAutoresizingMaskIntoConstraints(false);
    tree.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        header.topAnchor().constraintEqualToAnchor(&page.topAnchor()),
        header.leadingAnchor().constraintEqualToAnchor(&page.leadingAnchor()),
        header.trailingAnchor().constraintEqualToAnchor(&page.trailingAnchor()),
        header.heightAnchor().constraintEqualToConstant(HEIGHT),
        tree.topAnchor().constraintEqualToAnchor(&header.bottomAnchor()),
        tree.leadingAnchor().constraintEqualToAnchor(&page.leadingAnchor()),
        tree.trailingAnchor().constraintEqualToAnchor(&page.trailingAnchor()),
        tree.bottomAnchor().constraintEqualToAnchor(&page.bottomAnchor()),
    ] {
        constraint.setActive(true);
    }
    page
}

fn header(mtm: MainThreadMarker, name: &str, target: &AnyObject) -> Retained<NSStackView> {
    let title = label(mtm, name);
    // SAFETY: AppKit sets its font weight constants when it loads.
    title.setFont(Some(&NSFont::systemFontOfSize_weight(11.0, unsafe {
        objc2_app_kit::NSFontWeightSemibold
    })));
    title.setTextColor(Some(&NSColor::secondaryLabelColor()));
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    title.setContentCompressionResistancePriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(2.0);
    row.setEdgeInsets(NSEdgeInsets {
        top: 0.0,
        left: 8.0,
        bottom: 0.0,
        right: 6.0,
    });
    row.addView_inGravity(&title, NSStackViewGravity::Leading);
    for (symbol, name, action) in [
        ("doc.badge.plus", t("Neue Datei …"), sel!(ideTreeNewFile:)),
        ("folder.badge.plus", t("Neuer Ordner …"), sel!(ideTreeNewFolder:)),
        (
            "rectangle.compress.vertical",
            t("Alle einklappen"),
            sel!(ideTreeCollapseAll:),
        ),
    ] {
        let button: Retained<NSButton> = symbol_button(mtm, symbol, name, SYMBOL_POINTS, Some(target), action);
        button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        crate::util::set_label(&button, Some(name));
        row.addView_inGravity(&button, NSStackViewGravity::Trailing);
    }
    row.setAccessibilityLabel(Some(&crate::util::ns(name)));
    row
}
