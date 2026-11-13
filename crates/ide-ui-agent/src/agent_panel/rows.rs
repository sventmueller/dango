//! The rows of the panel's list, a session with its circle, green while it is open, its title and age, and the small
//! views other panels borrow.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSColor, NSImageSymbolConfiguration, NSImageView, NSLayoutConstraintOrientation, NSLineBreakMode, NSTableCellView,
    NSTextField, NSView,
};

use ide_ui_common::constant;
use ide_ui_common::util::{ns, symbol};

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Flipped` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFlippedView"]
    pub struct Flipped;

    impl Flipped {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

/// A view whose origin is at its top, as the document of a scroll view that starts at the top.
pub fn flipped(mtm: MainThreadMarker) -> Retained<NSView> {
    // SAFETY: NSView's initializer, called once on the newly allocated object.
    let view: Retained<Flipped> = unsafe { msg_send![Flipped::alloc(mtm), init] };
    Retained::into_super(view)
}

pub fn spacer(mtm: MainThreadMarker) -> Retained<NSView> {
    let view = NSView::new(mtm);
    view.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
    view
}

pub fn small_label(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&ns(text), mtm);
    label.setFont(Some(&crate::style::small()));
    label.setTextColor(Some(&NSColor::secondaryLabelColor()));
    label
}

/// The head of a section, in small capitals as the sidebar's sections read.
pub fn section_label(mtm: MainThreadMarker, title: &str) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&ns(&title.to_uppercase()), mtm);
    label.setFont(Some(&crate::style::bold(10.0)));
    label.setTextColor(Some(&NSColor::secondaryLabelColor()));
    label
}

fn icon(mtm: MainThreadMarker, name: &str, points: f64, color: &NSColor) -> Retained<NSImageView> {
    let view = NSImageView::new(mtm);
    view.setImage(symbol(name, "").as_deref());
    view.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(
        points,
        constant!(objc2_app_kit::NSFontWeightRegular),
    )));
    view.setContentTintColor(Some(color));
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    view.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
    view
}

/// What a row shows.
pub struct Look<'a> {
    /// Whether the session is open in a tab: a green circle in front, else an empty one.
    pub open: bool,
    pub title: &'a str,
    pub dim: bool,
    /// The quieter text at the end.
    pub detail: &'a str,
    /// A symbol after the title, such as a bookmark.
    pub badge: Option<(&'a str, Retained<NSColor>)>,
}

/// The size of the circle in front of a session.
const DOT_POINTS: f64 = 7.0;

/// A row of the panel's list.
pub fn cell(mtm: MainThreadMarker, look: &Look<'_>) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let title = NSTextField::labelWithString(&ns(look.title), mtm);
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let color = if look.dim {
        NSColor::secondaryLabelColor()
    } else {
        NSColor::labelColor()
    };
    title.setTextColor(Some(&color));
    title.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Horizontal);
    let detail = small_label(mtm, look.detail);
    detail.setTextColor(Some(&NSColor::tertiaryLabelColor()));
    detail.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    detail.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
    let dot = if look.open {
        icon(mtm, "circle.fill", DOT_POINTS, &NSColor::systemGreenColor())
    } else {
        icon(mtm, "circle", DOT_POINTS, &NSColor::tertiaryLabelColor())
    };
    let mut views: Vec<Retained<NSView>> = vec![Retained::into_super(Retained::into_super(dot))];
    views.push(Retained::into_super(Retained::into_super(title.clone())));
    if let Some((name, color)) = &look.badge {
        views.push(Retained::into_super(Retained::into_super(icon(mtm, name, 9.0, color))));
    }
    views.push(spacer(mtm));
    views.push(Retained::into_super(Retained::into_super(detail)));
    let stack = objc2_app_kit::NSStackView::new(mtm);
    stack.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setAlignment(objc2_app_kit::NSLayoutAttribute::CenterY);
    stack.setSpacing(ide_ui_common::style::space::S);
    for view in &views {
        stack.addArrangedSubview(view);
    }
    cell.addSubview(&stack);
    // SAFETY: The label is a subview of the cell, which keeps it.
    unsafe { cell.setTextField(Some(&title)) };
    stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        stack
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&cell.leadingAnchor(), 2.0),
        stack.trailingAnchor().constraintEqualToAnchor(&cell.trailingAnchor()),
        stack.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ] {
        constraint.setActive(true);
    }
    Retained::into_super(cell)
}
