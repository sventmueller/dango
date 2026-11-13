//! Small building blocks of the AI tab's cards and bars.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityGroupRole, NSBezelStyle, NSBox, NSBoxType, NSButton, NSColor, NSControlSize,
    NSFont, NSImage, NSImageScaling, NSImageSymbolConfiguration, NSImageView, NSLayoutAttribute,
    NSLayoutConstraintOrientation, NSLineBreakMode, NSProgressIndicator, NSProgressIndicatorStyle, NSStackView,
    NSStackViewDistribution, NSTextField, NSTitlePosition, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSData, NSEdgeInsets, NSSize};

use ide_ui_common::constant;
use ide_ui_common::util::{ns, set_action, symbol};

/// A label on one line, cut at its end when it does not fit.
pub fn label(mtm: MainThreadMarker, text: &str, font: &NSFont, color: &NSColor) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&ns(text), mtm);
    field.setFont(Some(font));
    field.setTextColor(Some(color));
    field.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    field.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Horizontal);
    field
}

/// A label that wraps at `width`.
pub fn wrapping(
    mtm: MainThreadMarker,
    text: &str,
    font: &NSFont,
    color: &NSColor,
    width: f64,
) -> Retained<NSTextField> {
    let field = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    field.setFont(Some(font));
    field.setTextColor(Some(color));
    field.setPreferredMaxLayoutWidth(width.max(40.0));
    field.setSelectable(true);
    field
}

fn stack(mtm: MainThreadMarker, orientation: NSUserInterfaceLayoutOrientation, spacing: f64) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(orientation);
    stack.setSpacing(spacing);
    stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    stack
}

/// Views side by side, centred on one line.
pub fn row(mtm: MainThreadMarker, spacing: f64, views: &[&NSView]) -> Retained<NSStackView> {
    let stack = stack(mtm, NSUserInterfaceLayoutOrientation::Horizontal, spacing);
    stack.setAlignment(NSLayoutAttribute::CenterY);
    for view in views {
        stack.addArrangedSubview(view);
    }
    stack
}

/// Views one below the other, each as wide as the column.
pub fn column(mtm: MainThreadMarker, spacing: f64, views: &[&NSView]) -> Retained<NSStackView> {
    let stack = stack(mtm, NSUserInterfaceLayoutOrientation::Vertical, spacing);
    stack.setAlignment(NSLayoutAttribute::Leading);
    stack.setDistribution(NSStackViewDistribution::Fill);
    for view in views {
        add_full_width(&stack, view);
    }
    stack
}

/// Adds `view` to the vertical `stack`, as wide as the stack.
pub fn add_full_width(stack: &NSStackView, view: &NSView) {
    stack.addArrangedSubview(view);
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    view.widthAnchor()
        .constraintEqualToAnchor(&stack.widthAnchor())
        .setActive(true);
}

/// A view that takes the room left in a row.
pub fn spacer(mtm: MainThreadMarker) -> Retained<NSView> {
    let view = NSView::new(mtm);
    view.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
    view.setContentCompressionResistancePriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
    view
}

/// `content` in a rounded box with a 1-point border, `padding` points inside.
pub fn framed(
    mtm: MainThreadMarker,
    content: &NSView,
    border: &NSColor,
    fill: Option<&NSColor>,
    radius: f64,
    padding: NSEdgeInsets,
) -> Retained<NSBox> {
    let frame = NSBox::new(mtm);
    frame.setBoxType(NSBoxType::Custom);
    frame.setTitlePosition(NSTitlePosition::NoTitle);
    frame.setBorderWidth(1.0);
    frame.setBorderColor(border);
    frame.setCornerRadius(radius);
    let fill = fill.map_or_else(NSColor::clearColor, |c| c.retain());
    frame.setFillColor(&fill);
    frame.setContentViewMargins(NSSize::new(0.0, 0.0));
    frame.setTranslatesAutoresizingMaskIntoConstraints(false);
    let inner = NSView::new(mtm);
    frame.setContentView(Some(&inner));
    // The box would place its content view by autoresizing offsets taken from its first frames, which are stale
    // when the card is measured; pinned, the box is exactly as tall as its content.
    inner.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        inner.leadingAnchor().constraintEqualToAnchor(&frame.leadingAnchor()),
        inner.trailingAnchor().constraintEqualToAnchor(&frame.trailingAnchor()),
        inner.topAnchor().constraintEqualToAnchor(&frame.topAnchor()),
        inner.bottomAnchor().constraintEqualToAnchor(&frame.bottomAnchor()),
    ] {
        constraint.setActive(true);
    }
    inner.addSubview(content);
    content.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        content
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&inner.leadingAnchor(), padding.left),
        content
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&inner.trailingAnchor(), -padding.right),
        content
            .topAnchor()
            .constraintEqualToAnchor_constant(&inner.topAnchor(), padding.top),
        content
            .bottomAnchor()
            .constraintEqualToAnchor_constant(&inner.bottomAnchor(), -padding.bottom),
    ] {
        constraint.setActive(true);
    }
    frame
}

/// The side of a picture's tile, about as large as in Claude's app.
const TILE: f64 = 76.0;
const TILE_PADDING: f64 = 6.0;
const TILE_RADIUS: f64 = 10.0;

/// A picture as a tile showing it, named `name`, that opens it large on a click, with `remove` in its corner if it
/// can be dropped.
pub fn picture_tile(mtm: MainThreadMarker, data: &[u8], name: &str, remove: Option<&NSButton>) -> Retained<NSView> {
    let image = NSImage::initWithData(objc2::AnyThread::alloc(), &NSData::with_bytes(data));
    let view = NSImageView::new(mtm);
    view.setImage(image.as_deref());
    view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    view.setAccessibilityElement(false);
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    let tile = framed(
        mtm,
        &view,
        &NSColor::separatorColor(),
        Some(&NSColor::controlBackgroundColor()),
        TILE_RADIUS,
        insets(TILE_PADDING, TILE_PADDING, TILE_PADDING, TILE_PADDING),
    );
    tile.widthAnchor().constraintEqualToConstant(TILE).setActive(true);
    tile.heightAnchor().constraintEqualToConstant(TILE).setActive(true);
    let Some(inner) = tile.contentView() else {
        return Retained::into_super(tile);
    };
    let enlarge = super::enlarge::button(mtm, name);
    inner.addSubview(&enlarge);
    enlarge.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        enlarge.leadingAnchor().constraintEqualToAnchor(&inner.leadingAnchor()),
        enlarge
            .trailingAnchor()
            .constraintEqualToAnchor(&inner.trailingAnchor()),
        enlarge.topAnchor().constraintEqualToAnchor(&inner.topAnchor()),
        enlarge.bottomAnchor().constraintEqualToAnchor(&inner.bottomAnchor()),
    ] {
        constraint.setActive(true);
    }
    if let Some(remove) = remove {
        inner.addSubview(remove);
        remove.setTranslatesAutoresizingMaskIntoConstraints(false);
        remove
            .topAnchor()
            .constraintEqualToAnchor_constant(&tile.topAnchor(), 3.0)
            .setActive(true);
        remove
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&tile.trailingAnchor(), -3.0)
            .setActive(true);
    }
    Retained::into_super(tile)
}

pub fn insets(top: f64, left: f64, bottom: f64, right: f64) -> NSEdgeInsets {
    NSEdgeInsets {
        top,
        left,
        bottom,
        right,
    }
}

/// A thin line across a box.
pub fn separator(mtm: MainThreadMarker) -> Retained<NSBox> {
    let line = NSBox::new(mtm);
    line.setBoxType(NSBoxType::Separator);
    line.setTranslatesAutoresizingMaskIntoConstraints(false);
    line
}

/// An SF Symbol at `points`, tinted `color`.
pub fn icon(
    mtm: MainThreadMarker,
    name: &str,
    description: &str,
    points: f64,
    color: &NSColor,
) -> Retained<NSImageView> {
    let view = NSImageView::new(mtm);
    view.setImage(symbol(name, description).as_deref());
    view.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(
        points,
        constant!(objc2_app_kit::NSFontWeightRegular),
    )));
    view.setContentTintColor(Some(color));
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    view.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
    view
}

/// A small bordered push button that sends `action` with `tag` to `target`.
pub fn button(mtm: MainThreadMarker, title: &str, target: &AnyObject, action: Sel, tag: isize) -> Retained<NSButton> {
    // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
    set_action(&button, target, action);
    button.setTag(tag);
    button.setBezelStyle(NSBezelStyle::Push);
    button.setControlSize(NSControlSize::Small);
    button.setFont(Some(&NSFont::systemFontOfSize(NSFont::systemFontSizeForControlSize(
        NSControlSize::Small,
    ))));
    button
}

/// A borderless button in the accent colour, read as a link.
pub fn link(mtm: MainThreadMarker, title: &str, target: &AnyObject, action: Sel, tag: isize) -> Retained<NSButton> {
    // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
    set_action(&button, target, action);
    button.setTag(tag);
    button.setBordered(false);
    button.setFont(Some(&super::style::small()));
    button.setContentTintColor(Some(&NSColor::linkColor()));
    button
}

/// A borderless symbol button.
pub fn symbol_button(
    mtm: MainThreadMarker,
    name: &str,
    description: &str,
    points: f64,
    target: &AnyObject,
    action: Sel,
    tag: isize,
) -> Retained<NSButton> {
    let button = ide_ui_common::util::symbol_button(mtm, name, description, points, Some(target), action);
    button.setTag(tag);
    button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    button
}

/// A small spinning progress indicator, already turning.
pub fn spinner(mtm: MainThreadMarker) -> Retained<NSProgressIndicator> {
    let spinner = NSProgressIndicator::new(mtm);
    spinner.setStyle(NSProgressIndicatorStyle::Spinning);
    spinner.setControlSize(NSControlSize::Small);
    spinner.setIndeterminate(true);
    spinner.setDisplayedWhenStopped(false);
    spinner.setTranslatesAutoresizingMaskIntoConstraints(false);
    spinner.widthAnchor().constraintEqualToConstant(12.0).setActive(true);
    spinner.heightAnchor().constraintEqualToConstant(12.0).setActive(true);
    // SAFETY: The indicator is on the main thread, where it animates.
    unsafe { spinner.startAnimation(None) };
    spinner
}

/// Makes `view` one element for VoiceOver, read as `summary`.
pub fn group(view: &NSView, summary: &str) {
    view.setAccessibilityElement(true);
    view.setAccessibilityRole(Some(constant!(NSAccessibilityGroupRole)));
    view.setAccessibilityLabel(Some(&ns(summary)));
}

/// Fixes the width of `view`.
pub fn fixed_width(view: &NSView, width: f64) {
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    view.widthAnchor().constraintEqualToConstant(width).setActive(true);
}

/// Fixes the height of `view`.
pub fn fixed_height(view: &NSView, height: f64) {
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    view.heightAnchor().constraintEqualToConstant(height).setActive(true);
}

/// Has `view` keep its own width in a row rather than stretch.
pub fn hug(view: &NSView) {
    view.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
    view.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
}

/// Any view as a plain view.
pub fn view(view: &NSView) -> Retained<NSView> {
    view.retain()
}
