//! The lines of the sessions of orders in the activity view (C58): an order's session runs without an AI tab, so its
//! line names the order and its role and can stop the turn, which leaves the order waiting for the user.

use objc2_app_kit::NSColor;
use splat_i18n::t;

use super::{Action, Line};
use crate::window::WindowController;

/// The role's name in the activity view.
fn role(role: ide_factory::Role) -> &'static str {
    match role {
        ide_factory::Role::Planner => t("Planer"),
        ide_factory::Role::Builder => t("Builder"),
        ide_factory::Role::Reviewer => t("Prüfer"),
        ide_factory::Role::Scout => t("Späher"),
    }
}

/// The lines of the working sessions of the orders of `controller`'s folder.
pub(super) fn lines(controller: &WindowController) -> Vec<Line> {
    let Some(root) = controller.project_root() else {
        return Vec::new();
    };
    controller
        .context()
        .leitstand()
        .activity(&root)
        .into_iter()
        .map(|(name, kind, key)| Line {
            symbol: "gearshape.2",
            tint: NSColor::controlAccentColor(),
            title: name,
            detail: role(kind).to_string(),
            indent: false,
            buttons: vec![(t("Stopp"), Action::StopOrder(key))],
        })
        .collect()
}
