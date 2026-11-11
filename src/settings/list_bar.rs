//! The buttons under a list of the settings, as macOS draws them: AppKit's own add and remove pictures in small square
//! buttons that join each other and the list's border.

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSImage, NSImageNameAddTemplate, NSImageNameRemoveTemplate, NSScrollView, NSStackView,
    NSTableView, NSUserInterfaceLayoutOrientation, NSView,
};

use crate::util::{constant, ns, set_action};

const WIDTH: f64 = 24.0;
const HEIGHT: f64 = 22.0;
/// How far the buttons reach into the list and into each other, so neighbouring borders draw as one line.
const OVERLAP: f64 = -1.0;
/// The room between the joined buttons and the ones after them.
const APART: f64 = 8.0;

/// The button that adds to a list, or removes its selected row.
pub fn button(
    mtm: MainThreadMarker,
    add: bool,
    description: &str,
    target: &AnyObject,
    action: Sel,
) -> Retained<NSButton> {
    let name = if add {
        constant!(NSImageNameAddTemplate)
    } else {
        constant!(NSImageNameRemoveTemplate)
    };
    let image = NSImage::imageNamed(name).unwrap_or_default();
    image.setAccessibilityDescription(Some(&ns(description)));
    // SAFETY: A nil target sends nothing until `set_action` gives the button its target and action.
    let button = unsafe { NSButton::buttonWithImage_target_action(&image, None, None, mtm) };
    set_action(&button, target, action);
    button.setBezelStyle(NSBezelStyle::SmallSquare);
    button.setToolTip(Some(&ns(description)));
    crate::util::set_label(&button, Some(description));
    button.setTranslatesAutoresizingMaskIntoConstraints(false);
    button.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
    button.heightAnchor().constraintEqualToConstant(HEIGHT).setActive(true);
    button
}

/// The row under a list: `joined` side by side as one control, then `rest` a little apart.
pub fn bar(mtm: MainThreadMarker, joined: &[&NSButton], rest: &[&NSView]) -> Retained<NSView> {
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(OVERLAP);
    for button in joined {
        row.addArrangedSubview(button);
    }
    if let Some(last) = joined.last() {
        row.setCustomSpacing_afterView(APART, last);
    }
    for view in rest {
        row.addArrangedSubview(view);
        row.setCustomSpacing_afterView(APART, view);
    }
    row.into_super()
}

/// Lets every list in `stack` touch the row that follows it, its bar.
pub fn attach(stack: &NSStackView) {
    let views = stack.arrangedSubviews();
    for (list, next) in views.iter().zip(views.iter().skip(1)) {
        let is_list = list.downcast_ref::<NSScrollView>().is_some_and(|scroll| {
            scroll
                .documentView()
                .is_some_and(|document| document.downcast_ref::<NSTableView>().is_some())
        });
        if is_list && next.downcast_ref::<NSStackView>().is_some() {
            stack.setCustomSpacing_afterView(OVERLAP, &list);
        }
    }
}
