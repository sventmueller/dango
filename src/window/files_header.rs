//! The buttons in the files panel's title row, as VS Code's explorer has them: a new file and a new folder, created
//! in the selected folder or the root, and folding every folder.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSButton, NSColor, NSStackView, NSUserInterfaceLayoutOrientation};
use splat_i18n::t;

use crate::util::symbol_button;

const SYMBOL_POINTS: f64 = 12.0;

/// The buttons, sending their actions to `target`, the tree's data source.
pub fn buttons(mtm: MainThreadMarker, target: &AnyObject) -> Retained<NSStackView> {
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(2.0);
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
        row.addArrangedSubview(&button);
    }
    row
}
