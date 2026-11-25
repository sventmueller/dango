//! The rows of the panel's list: a station's heading with its count, and an order with the shape of its condition,
//! its title, agent and runtime, and the red dot while it waits (E71).

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSLineBreakMode, NSStackViewGravity, NSTextField, NSUserInterfaceItemIdentification,
    NSView,
};
use objc2_foundation::NSString;
use splat_i18n::tf;

use crate::model::list::Line;
use crate::model::words;
use crate::ui;
use ide_ui_common::style;
use ide_ui_common::util::{label, ns};

/// The identifier of an order row's detail, which takes the runtime in place.
const DETAIL: &str = "ideFactoryDetail";

/// The view of `line`.
pub(crate) fn view(mtm: MainThreadMarker, line: &Line) -> Retained<NSView> {
    match line {
        Line::Station(station, count) => {
            let title = tf("{} ({})", &[words::station(*station), &count.to_string()]);
            ide_ui_common::panel::section_cell(mtm, &title).into_super()
        }
        Line::Order(order) => {
            let tone = order.tone();
            let (row, text) = ide_ui_common::panel::icon_row(
                mtm,
                (tone.symbol(), tone.word(), &ui::tone_color(tone)),
                order.title(),
                0,
                &order.spoken(),
            );
            text.setContentCompressionResistancePriority_forOrientation(
                objc2_app_kit::NSLayoutPriorityDefaultLow,
                objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
            );
            let detail = label(mtm, &order.detail());
            detail.setIdentifier(Some(&ns(DETAIL)));
            detail.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
            detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
            ide_ui_common::ui_size::set_system_font(&detail, style::SMALL);
            detail.setContentCompressionResistancePriority_forOrientation(
                objc2_app_kit::NSLayoutPriorityDefaultLow - 1.0,
                objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
            );
            row.addView_inGravity(&detail, NSStackViewGravity::Trailing);
            if order.waits() {
                row.addView_inGravity(&ui::dot(row.mtm()), NSStackViewGravity::Trailing);
            }
            row.into_super()
        }
    }
}

/// Puts the runtime of `line` into `cell`, built by [`view`] for a line of the same look.
pub(crate) fn retime(cell: &NSView, line: &Line) {
    let Line::Order(order) = line else { return };
    let Some(detail) = labelled(cell, &ns(DETAIL)) else {
        return;
    };
    let text = order.detail();
    if detail.stringValue().to_string() != text {
        detail.setStringValue(&ns(&text));
        cell.setAccessibilityLabel(Some(&ns(&order.spoken())));
    }
}

/// The label below `view` with the identifier `id`.
fn labelled(view: &NSView, id: &NSString) -> Option<Retained<NSTextField>> {
    for sub in view.subviews() {
        if sub.identifier().is_some_and(|own| own.isEqualToString(id)) {
            return sub.downcast::<NSTextField>().ok();
        }
        if let Some(found) = labelled(&sub, id) {
            return Some(found);
        }
    }
    None
}
