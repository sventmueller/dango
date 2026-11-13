//! A short note at the foot of the tab area that goes away by itself, such as how many documents came back after a
//! crash; VoiceOver reads it out when it shows.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSBox, NSBoxType, NSColor, NSFont, NSImageView, NSStackView, NSTextField, NSTitlePosition,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSObjectNSDelayedPerforming, NSSize};

use crate::util::{ns, symbol};

/// How far the note keeps from the foot, above an editor's status line.
const BOTTOM: f64 = 40.0;
/// How long the note stays, in seconds.
const SHOWN: f64 = 8.0;

/// Shows `text` at the foot of `over`, in front of what it holds, until it goes by itself; the top is the place of an
/// editor's bar about its file.
pub fn show(mtm: MainThreadMarker, over: &NSView, text: &str) -> Retained<NSBox> {
    let note = NSBox::new(mtm);
    note.setBoxType(NSBoxType::Custom);
    note.setTitlePosition(NSTitlePosition::NoTitle);
    note.setCornerRadius(8.0);
    note.setBorderWidth(1.0);
    note.setBorderColor(&NSColor::separatorColor());
    note.setFillColor(&NSColor::windowBackgroundColor());
    note.setContentViewMargins(NSSize::new(12.0, 6.0));
    let label = NSTextField::labelWithString(&ns(text), mtm);
    label.setFont(Some(&NSFont::systemFontOfSize(ide_ui_common::style::LABEL)));
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setSpacing(ide_ui_common::style::space::S);
    if let Some(image) = symbol("arrow.counterclockwise", "") {
        let icon = NSImageView::imageViewWithImage(&image, mtm);
        icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        stack.addArrangedSubview(&icon);
    }
    stack.addArrangedSubview(&label);
    if let Some(content) = note.contentView() {
        content.addSubview(&stack);
        crate::util::pin_edges(&stack, &content);
    }
    over.addSubview(&note);
    note.setTranslatesAutoresizingMaskIntoConstraints(false);
    note.bottomAnchor()
        .constraintEqualToAnchor_constant(&over.bottomAnchor(), -BOTTOM)
        .setActive(true);
    note.centerXAnchor()
        .constraintEqualToAnchor(&over.centerXAnchor())
        .setActive(true);
    announce(mtm, text);
    // SAFETY: `removeFromSuperview` takes no argument, so the nil object is never read; the run loop keeps the note
    // alive until then.
    unsafe { note.performSelector_withObject_afterDelay(sel!(removeFromSuperview), None, SHOWN) };
    note
}

pub use ide_ui_common::panel::announce;
