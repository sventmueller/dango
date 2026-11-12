//! A picture's tile opening the picture large in a popover, as Claude's app does on a click.

use std::cell::{OnceCell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSImage, NSImageScaling, NSImageView, NSPopover, NSPopoverBehavior, NSView,
    NSViewController,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSRectEdge, NSSize};

use crate::util::{guarded, ns, set_action, set_label};

/// The share of the window's width and height the picture may take.
const SHARE: f64 = 0.8;
/// The most the picture may take without a window to measure.
const FALLBACK: NSSize = NSSize::new(800.0, 600.0);

thread_local! {
    static OPENER: OnceCell<Retained<Opener>> = const { OnceCell::new() };
    static SHOWN: RefCell<Option<Retained<NSPopover>>> = const { RefCell::new(None) };
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Opener` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentPictureOpener"]
    struct Opener;

    unsafe impl NSObjectProtocol for Opener {}

    impl Opener {
        #[unsafe(method(ideAgentEnlargePicture:))]
        fn open(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                    show(button);
                }
            });
        }
    }
);

fn opener(mtm: MainThreadMarker) -> Retained<Opener> {
    OPENER.with(|cell| {
        cell.get_or_init(|| {
            let this = Opener::alloc(mtm).set_ivars(());
            // SAFETY: NSObject's initializer, called once on the newly allocated object.
            unsafe { msg_send![super(this), init] }
        })
        .clone()
    })
}

/// A clear button over a tile's picture, named `name`, that opens the picture large.
pub fn button(mtm: MainThreadMarker, name: &str) -> Retained<NSButton> {
    // SAFETY: A nil target sends the action along the responder chain; `set_action` gives it the opener right after.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(""), None, None, mtm) };
    set_action(&button, &opener(mtm), sel!(ideAgentEnlargePicture:));
    button.setBordered(false);
    button.setTransparent(true);
    button.setToolTip(Some(&ns(name)));
    set_label(&button, Some(name));
    button
}

/// Opens the picture of the tile `button` lies on in a popover pointing at the tile.
fn show(button: &NSButton) {
    // SAFETY: Asking for the superview only reads; the tile holds the button while it is clicked.
    let Some(tile) = (unsafe { button.superview() }) else {
        return;
    };
    let Some(image) = picture_in(&tile) else { return };
    if let Some(shown) = SHOWN.with(|shown| shown.borrow_mut().take()) {
        shown.close();
    }
    let mtm = button.mtm();
    let room = button
        .window()
        .and_then(|window| window.contentView())
        .map_or(FALLBACK, |content| content.frame().size);
    let size = fitted(image.size(), NSSize::new(room.width * SHARE, room.height * SHARE));
    let view = NSImageView::new(mtm);
    view.setImage(Some(&image));
    view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    view.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), size));
    view.setAccessibilityLabel(button.accessibilityLabel().as_deref());
    let controller = NSViewController::new(mtm);
    controller.setView(&view);
    let popover = NSPopover::new(mtm);
    popover.setBehavior(NSPopoverBehavior::Transient);
    popover.setContentViewController(Some(&controller));
    popover.setContentSize(size);
    popover.showRelativeToRect_ofView_preferredEdge(tile.bounds(), &tile, NSRectEdge::MaxY);
    SHOWN.with(|shown| *shown.borrow_mut() = Some(popover));
}

fn picture_in(view: &NSView) -> Option<Retained<NSImage>> {
    view.subviews()
        .iter()
        .find_map(|child| child.downcast_ref::<NSImageView>().and_then(|v| v.image()))
}

/// `picture` scaled down to fit `room`, never up.
fn fitted(picture: NSSize, room: NSSize) -> NSSize {
    if picture.width <= 0.0 || picture.height <= 0.0 {
        return room;
    }
    let scale = (room.width / picture.width).min(room.height / picture.height).min(1.0);
    NSSize::new((picture.width * scale).round(), (picture.height * scale).round())
}

/// The size of the popover on show, which then closes, for the self-test.
#[cfg(feature = "selftest")]
pub fn take_shown() -> Option<NSSize> {
    let popover = SHOWN.with(|shown| shown.borrow_mut().take())?;
    let size = popover.isShown().then(|| popover.contentSize());
    popover.close();
    size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_picture_shrinks_to_the_room_keeping_its_shape() {
        let size = fitted(NSSize::new(2000.0, 1000.0), NSSize::new(800.0, 600.0));
        assert_eq!((size.width, size.height), (800.0, 400.0));
    }

    #[test]
    fn a_small_picture_keeps_its_size() {
        let size = fitted(NSSize::new(300.0, 200.0), NSSize::new(800.0, 600.0));
        assert_eq!((size.width, size.height), (300.0, 200.0));
    }
}
