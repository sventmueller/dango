//! The rows of the panel's lists: a session with its status dot and age, a group, a running subagent or task.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSColor, NSFont, NSImageSymbolConfiguration, NSImageView, NSLayoutConstraintOrientation, NSLineBreakMode,
    NSTableCellView, NSTextField, NSView,
};

use crate::util::{constant, ns, symbol};

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
    label.setFont(Some(&crate::agent::style::small()));
    label.setTextColor(Some(&NSColor::secondaryLabelColor()));
    label
}

/// The head of a section, in small capitals as the sidebar's sections read.
pub fn section_label(mtm: MainThreadMarker, title: &str) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&ns(&title.to_uppercase()), mtm);
    label.setFont(Some(&crate::agent::style::bold(10.0)));
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
    /// The symbol in front, with its size and colour.
    pub symbol: Option<(&'a str, f64, Retained<NSColor>)>,
    pub title: &'a str,
    pub bold: bool,
    pub dim: bool,
    /// The quieter text at the end.
    pub detail: &'a str,
    /// A second symbol after the title, such as a bookmark.
    pub badge: Option<(&'a str, Retained<NSColor>)>,
}

/// Puts a chevron in front of the row `cell` made, pointing down when its item is open, where a row that does not
/// open shows its symbol.
pub fn add_chevron(mtm: MainThreadMarker, cell: &NSView, open: bool) {
    let Some(stack) = cell
        .subviews()
        .iter()
        .find_map(|view| view.downcast::<objc2_app_kit::NSStackView>().ok())
    else {
        return;
    };
    let view = NSImageView::new(mtm);
    view.setImage(crate::window::file_outline::chevron(open).as_deref());
    view.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
    stack.insertArrangedSubview_atIndex(&view, 0);
}

/// A row of the panel's lists.
pub fn cell(mtm: MainThreadMarker, look: &Look<'_>) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let title = NSTextField::labelWithString(&ns(look.title), mtm);
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let font = if look.bold {
        crate::agent::style::bold(11.0)
    } else {
        NSFont::systemFontOfSize(12.0)
    };
    title.setFont(Some(&font));
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
    let mut views: Vec<Retained<NSView>> = Vec::new();
    if let Some((name, points, color)) = &look.symbol {
        views.push(Retained::into_super(Retained::into_super(icon(
            mtm, name, *points, color,
        ))));
    }
    views.push(Retained::into_super(Retained::into_super(title.clone())));
    if let Some((name, color)) = &look.badge {
        views.push(Retained::into_super(Retained::into_super(icon(mtm, name, 9.0, color))));
    }
    views.push(spacer(mtm));
    views.push(Retained::into_super(Retained::into_super(detail)));
    let stack = objc2_app_kit::NSStackView::new(mtm);
    stack.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setAlignment(objc2_app_kit::NSLayoutAttribute::CenterY);
    stack.setSpacing(5.0);
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
        stack
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
        stack.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ] {
        constraint.setActive(true);
    }
    Retained::into_super(cell)
}
