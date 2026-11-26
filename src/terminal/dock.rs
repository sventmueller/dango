//! The bar on the Dock icon while programs in terminals report progress (Tapa's F56): the app's icon with one bar for
//! all of them over its lower edge. It draws as the Dock tile's content view and leaves the tile's badge alone, which
//! stays empty (E72).

use std::cell::{Cell, OnceCell};

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSBezierPath, NSColor, NSGraphicsContext, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::progress::Ring;
use crate::util::guarded;

/// How far the bar keeps from the icon's left and right edge, against the icon's width.
const SIDE_INSET: f64 = 0.12;
/// How far the bar sits above the icon's lower edge, against the icon's height.
const BOTTOM: f64 = 0.1;
/// The bar's height against the icon's.
const HEIGHT: f64 = 0.11;
/// The dark edge around the filled part, against the bar's height.
const EDGE: f64 = 0.18;
/// How dark the bar's track is.
const TRACK_ALPHA: f64 = 0.55;

define_class!(
    // SAFETY: NSView has no subclassing requirements and `DockProgress` does not implement Drop.
    /// The Dock tile's picture while terminals report progress.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEDockProgress"]
    #[ivars = Cell<Option<Ring>>]
    pub(crate) struct DockProgress;

    unsafe impl NSObjectProtocol for DockProgress {}

    impl DockProgress {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            guarded((), || self.draw());
        }
    }
);

impl DockProgress {
    fn new(mtm: MainThreadMarker) -> Retained<DockProgress> {
        let this = Self::alloc(mtm).set_ivars(Cell::new(None));
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// Shows `ring` on the Dock icon, made in `slot` the first time, or the icon alone without one.
    pub(super) fn show(slot: &OnceCell<Retained<DockProgress>>, mtm: MainThreadMarker, ring: Option<Ring>) {
        let tile = NSApplication::sharedApplication(mtm).dockTile();
        let Some(ring) = ring else {
            if slot.get().is_some_and(|view| view.ivars().replace(None).is_some()) {
                tile.setContentView(None);
                tile.display();
            }
            return;
        };
        let view = slot.get_or_init(|| DockProgress::new(mtm));
        if view.ivars().replace(Some(ring)) == Some(ring) {
            return;
        }
        view.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), tile.size()));
        tile.setContentView(Some(view));
        tile.display();
    }

    fn draw(&self) {
        let bounds = self.bounds();
        if let Some(icon) = NSApplication::sharedApplication(self.mtm()).applicationIconImage() {
            icon.drawInRect(bounds);
        }
        let Some(ring) = self.ivars().get() else { return };
        let bar = NSRect::new(
            NSPoint::new(bounds.size.width * SIDE_INSET, bounds.size.height * BOTTOM),
            NSSize::new(
                bounds.size.width * (1.0 - 2.0 * SIDE_INSET),
                bounds.size.height * HEIGHT,
            ),
        );
        fill(&rounded(bar), &NSColor::colorWithWhite_alpha(0.0, TRACK_ALPHA));
        let edge = bar.size.height * EDGE;
        let inner = NSRect::new(
            NSPoint::new(bar.origin.x + edge, bar.origin.y + edge),
            NSSize::new(bar.size.width - 2.0 * edge, bar.size.height - 2.0 * edge),
        );
        let color = crate::window::tone_color(ring.tone);
        match ring.filled {
            Some(filled) if filled > 0.0 => {
                let width = (inner.size.width * filled.min(1.0)).max(inner.size.height);
                fill(
                    &rounded(NSRect::new(inner.origin, NSSize::new(width, inner.size.height))),
                    &color,
                );
            }
            Some(_) => {}
            None => stripes(inner, &color),
        }
    }
}

/// A rectangle with its short sides round.
fn rounded(rect: NSRect) -> Retained<NSBezierPath> {
    let radius = rect.size.height / 2.0;
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius)
}

fn fill(path: &NSBezierPath, color: &NSColor) {
    color.setFill();
    path.fill();
}

/// Slanted stripes along the whole bar for work that does not know how far it is; a Dock tile is a still picture,
/// so it cannot turn as the tab's ring does.
fn stripes(inner: NSRect, color: &NSColor) {
    let height = inner.size.height;
    if height <= 0.0 {
        return;
    }
    NSGraphicsContext::saveGraphicsState_class();
    rounded(inner).addClip();
    color.setFill();
    let mut x = inner.origin.x - height;
    while x < inner.origin.x + inner.size.width {
        let stripe = NSBezierPath::bezierPath();
        stripe.moveToPoint(NSPoint::new(x, inner.origin.y));
        stripe.lineToPoint(NSPoint::new(x + height / 2.0, inner.origin.y));
        stripe.lineToPoint(NSPoint::new(x + height * 1.5, inner.origin.y + height));
        stripe.lineToPoint(NSPoint::new(x + height, inner.origin.y + height));
        stripe.closePath();
        stripe.fill();
        x += height;
    }
    NSGraphicsContext::restoreGraphicsState_class();
}
