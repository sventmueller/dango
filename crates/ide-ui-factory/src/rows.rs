//! The rows of the panel's list: a station's heading with its count, and an order with the shape of its condition,
//! its title, agent and runtime, and the red dot while it waits (E71).

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSColor, NSLineBreakMode, NSStackViewGravity, NSView};
use splat_i18n::tf;

use crate::model::list::Line;
use crate::model::words;
use crate::ui;
use ide_ui_common::style;
use ide_ui_common::util::label;

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
                order.spoken(),
            );
            text.setContentCompressionResistancePriority_forOrientation(
                objc2_app_kit::NSLayoutPriorityDefaultLow,
                objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
            );
            let detail = label(mtm, order.detail());
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
