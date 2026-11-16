//! The pieces the Leitstand's views are built from: labels that wrap, code that stays left to right in a right-to-left
//! layout (E3), headings, buttons that carry a command, the shape of a condition, and a page that scrolls from the top.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSImageView, NSLayoutAttribute, NSLineBreakMode, NSScrollView, NSStackView,
    NSTextField, NSUserInterfaceLayoutDirection, NSUserInterfaceLayoutOrientation, NSView, NSWritingDirection,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};

use crate::model::words::Tone;
use ide_ui_common::style;
use ide_ui_common::util::{label, ns, set_action, symbol};

define_class!(
    // SAFETY: NSView has no subclassing requirements and `TopDown` does not implement Drop.
    /// A view whose origin is at its top, so a stack in a scroll view starts at the top.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryTopDown"]
    pub(crate) struct TopDown;

    unsafe impl NSObjectProtocol for TopDown {}

    impl TopDown {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl TopDown {
    fn new(mtm: MainThreadMarker) -> Retained<TopDown> {
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        unsafe { msg_send![TopDown::alloc(mtm), init] }
    }
}

/// A vertical stack of `spacing` that lines its views up at the leading edge.
pub(crate) fn column(mtm: MainThreadMarker, spacing: f64) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::Leading);
    stack.setSpacing(spacing);
    stack
}

/// A horizontal stack of `spacing`, centred on its views' middle.
pub(crate) fn row(mtm: MainThreadMarker, spacing: f64) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    stack.setAlignment(NSLayoutAttribute::CenterY);
    stack.setSpacing(spacing);
    stack
}

/// `content` in a scroll view that starts at its top and makes it as wide as itself.
pub(crate) fn scrolling(mtm: MainThreadMarker, content: &NSView) -> Retained<NSScrollView> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    let page = TopDown::new(mtm);
    page.addSubview(content);
    content.setTranslatesAutoresizingMaskIntoConstraints(false);
    page.setTranslatesAutoresizingMaskIntoConstraints(false);
    scroll.setDocumentView(Some(&page));
    let clip = scroll.contentView();
    ide_ui_common::util::activate([
        content.topAnchor().constraintEqualToAnchor(&page.topAnchor()),
        content.leadingAnchor().constraintEqualToAnchor(&page.leadingAnchor()),
        content.trailingAnchor().constraintEqualToAnchor(&page.trailingAnchor()),
        content.bottomAnchor().constraintEqualToAnchor(&page.bottomAnchor()),
        page.topAnchor().constraintEqualToAnchor(&clip.topAnchor()),
        page.leadingAnchor().constraintEqualToAnchor(&clip.leadingAnchor()),
        page.widthAnchor().constraintEqualToAnchor(&clip.widthAnchor()),
    ]);
    scroll
}

/// A label of `text` that wraps at `width`, which the user can select and copy.
pub(crate) fn text(mtm: MainThreadMarker, text: &str, width: f64) -> Retained<NSTextField> {
    let field = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    field.setSelectable(true);
    field.setPreferredMaxLayoutWidth(width);
    field
}

/// Grey text in the small size, wrapped at `width`.
pub(crate) fn note(mtm: MainThreadMarker, words: &str, width: f64) -> Retained<NSTextField> {
    let field = text(mtm, words, width);
    field.setFont(Some(&style::small_system()));
    field.setTextColor(Some(&NSColor::secondaryLabelColor()));
    field
}

/// Code or a log, monospaced and left to right even where the layout runs right to left (E3).
pub(crate) fn code(mtm: MainThreadMarker, words: &str, width: f64) -> Retained<NSTextField> {
    let field = text(mtm, words, width);
    field.setFont(Some(&style::mono(style::CODE)));
    field.setBaseWritingDirection(NSWritingDirection::LeftToRight);
    field.setUserInterfaceLayoutDirection(NSUserInterfaceLayoutDirection::LeftToRight);
    field.setAlignment(objc2_app_kit::NSTextAlignment::Left);
    field
}

/// A section's heading in a tab.
pub(crate) fn heading(mtm: MainThreadMarker, words: &str) -> Retained<NSTextField> {
    let field = label(mtm, words);
    field.setFont(Some(&style::bold(style::HEADING)));
    field.setAccessibilityRole(Some(ide_ui_common::constant!(
        objc2_app_kit::NSAccessibilityStaticTextRole
    )));
    field
}

/// A tab's title.
pub(crate) fn title(mtm: MainThreadMarker, words: &str) -> Retained<NSTextField> {
    let field = label(mtm, words);
    field.setFont(Some(&style::bold(style::PAGE_HEADING)));
    field.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    field
}

/// A push button titled `title` that sends `action` with `tag` to `target`.
pub(crate) fn button(
    mtm: MainThreadMarker,
    title: &str,
    target: &AnyObject,
    action: Sel,
    tag: isize,
) -> Retained<NSButton> {
    // SAFETY: A nil target sends nothing until `set_action` sets the target that answers the action.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
    set_action(&button, target, action);
    button.setTag(tag);
    button
}

/// The symbol of `tone`, red for a wait and named by its word for VoiceOver (E101).
pub(crate) fn tone_image(mtm: MainThreadMarker, tone: Tone) -> Retained<NSImageView> {
    let image = NSImageView::new(mtm);
    image.setImage(symbol(tone.symbol(), tone.word()).as_deref());
    image.setContentTintColor(Some(&tone_color(tone)));
    image.setAccessibilityLabel(Some(&ns(tone.word())));
    image
}

/// The colour of `tone`, which goes with its shape and word, never alone.
pub(crate) fn tone_color(tone: Tone) -> Retained<NSColor> {
    match tone {
        Tone::Waiting | Tone::Failed => NSColor::systemRedColor(),
        Tone::Running => NSColor::systemGreenColor(),
        Tone::Done => NSColor::controlAccentColor(),
        Tone::Paused | Tone::Aborted => NSColor::secondaryLabelColor(),
    }
}

/// The small red dot of an order that waits for the user (E71), which VoiceOver skips as the row says "wartet".
pub(crate) fn dot(mtm: MainThreadMarker) -> Retained<NSImageView> {
    let image = NSImageView::new(mtm);
    image.setImage(symbol("circle.fill", "").as_deref());
    image.setContentTintColor(Some(&NSColor::systemRedColor()));
    image.setSymbolConfiguration(Some(
        &objc2_app_kit::NSImageSymbolConfiguration::configurationWithPointSize_weight(
            style::symbol::TAB_CLOSE,
            ide_ui_common::constant!(objc2_app_kit::NSFontWeightRegular),
        ),
    ));
    image.setAccessibilityElement(false);
    image
}

/// Removes every view of `stack`.
pub(crate) fn clear(stack: &NSStackView) {
    for view in stack.arrangedSubviews().iter() {
        stack.removeArrangedSubview(&view);
        view.removeFromSuperview();
    }
}
