//! The buttons in the files panel's title row, as VS Code's explorer has them: a new file and a new folder, created
//! in the selected folder or the root, and folding every folder.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::NSStackView;
use splat_i18n::t;

use super::panels::{header_button, header_row};

/// The buttons, sending their actions to `target`, the tree's data source.
pub(crate) fn buttons(mtm: MainThreadMarker, target: &AnyObject) -> Retained<NSStackView> {
    let row = header_row(mtm);
    for (symbol, name, action) in [
        ("doc.badge.plus", t("Neue Datei …"), sel!(ideTreeNewFile:)),
        ("folder.badge.plus", t("Neuer Ordner …"), sel!(ideTreeNewFolder:)),
        (
            "rectangle.compress.vertical",
            t("Alle einklappen"),
            sel!(ideTreeCollapseAll:),
        ),
    ] {
        row.addArrangedSubview(&header_button(mtm, symbol, name, Some(target), action));
    }
    row
}
