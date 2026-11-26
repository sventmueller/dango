//! The ring a window tab shows where its status dot sits while the program of its terminal reports progress (Tapa's
//! F56): filled as far as the work is in the colour of how it goes, or AppKit's spinning indicator while the program
//! does not know how far it is.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSBezierPath, NSColor, NSControlSize, NSLineCapStyle, NSProgressIndicator,
    NSProgressIndicatorStyle, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect};

use crate::terminal::{Ring, Tone};
use crate::util::{constant, guarded, ns, pin_edges};
use ide_ui_common::ui_size;

/// The ring's side at the default interface size, a little larger than the status dot it stands in for.
const SIDE: f64 = 12.0;
/// The width of its line against its side.
const LINE: f64 = 0.18;

pub(crate) struct Ivars {
    pub(super) shown: Cell<Option<Ring>>,
    spinner: Retained<NSProgressIndicator>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `ProgressRing` does not implement Drop.
    /// A window tab's progress ring.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDETabProgressRing"]
    #[ivars = Ivars]
    pub(crate) struct ProgressRing;

    unsafe impl NSObjectProtocol for ProgressRing {}

    impl ProgressRing {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            guarded((), || self.draw());
        }
    }
);

impl ProgressRing {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<ProgressRing> {
        let spinner = NSProgressIndicator::new(mtm);
        spinner.setStyle(NSProgressIndicatorStyle::Spinning);
        spinner.setControlSize(NSControlSize::Small);
        spinner.setIndeterminate(true);
        spinner.setDisplayedWhenStopped(false);
        spinner.setHidden(true);
        spinner.setAccessibilityElement(false);
        let this = Self::alloc(mtm).set_ivars(Ivars {
            shown: Cell::new(None),
            spinner: spinner.clone(),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let ring: Retained<Self> = unsafe { msg_send![super(this), init] };
        ring.addSubview(&spinner);
        pin_edges(&spinner, &ring);
        ring.setTranslatesAutoresizingMaskIntoConstraints(false);
        for side in [ring.widthAnchor(), ring.heightAnchor()] {
            let constraint = side.constraintEqualToConstant(ui_size::scaled(SIDE));
            ui_size::follow_length(&constraint, SIDE);
            constraint.setActive(true);
        }
        ring.setAccessibilityElement(true);
        ring.setAccessibilityRole(Some(constant!(objc2_app_kit::NSAccessibilityProgressIndicatorRole)));
        ring
    }

    pub(crate) fn show(&self, ring: Ring) {
        if self.ivars().shown.replace(Some(ring)) == Some(ring) {
            return;
        }
        let spinner = &self.ivars().spinner;
        let turning = ring.filled.is_none();
        spinner.setHidden(!turning);
        // SAFETY: A nil sender is what AppKit's own callers pass.
        unsafe {
            if turning {
                spinner.startAnimation(None);
            } else {
                spinner.stopAnimation(None);
            }
        }
        let words = ns(&ring.spoken());
        self.setAccessibilityLabel(Some(&words));
        self.setToolTip(Some(&words));
        self.setNeedsDisplay(true);
    }

    /// Shows nothing until the next ring, for a tab that no longer shows it.
    pub(crate) fn clear(&self) {
        self.ivars().shown.set(None);
        // SAFETY: A nil sender is what AppKit's own callers pass.
        unsafe { self.ivars().spinner.stopAnimation(None) };
    }

    fn draw(&self) {
        let Some(ring) = self.ivars().shown.get() else { return };
        let Some(filled) = ring.filled else { return };
        let bounds = self.bounds();
        let side = bounds.size.width.min(bounds.size.height);
        let line = side * LINE;
        let radius = (side - line) / 2.0;
        let center = NSPoint::new(
            bounds.origin.x + bounds.size.width / 2.0,
            bounds.origin.y + bounds.size.height / 2.0,
        );
        let track = NSBezierPath::bezierPath();
        track.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(center, radius, 0.0, 360.0, false);
        track.setLineWidth(line);
        NSColor::tertiaryLabelColor().setStroke();
        track.stroke();
        if filled <= 0.0 {
            return;
        }
        // AppKit counts angles counterclockwise from the right; the work fills from the top clockwise.
        let arc = NSBezierPath::bezierPath();
        let end = 90.0 - 360.0 * filled.min(1.0);
        arc.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(center, radius, 90.0, end, true);
        arc.setLineWidth(line);
        arc.setLineCapStyle(NSLineCapStyle::Round);
        tone_color(ring.tone).setStroke();
        arc.stroke();
    }
}

/// The colour of work that goes as `tone` says.
pub(crate) fn tone_color(tone: Tone) -> Retained<NSColor> {
    match tone {
        Tone::Normal => NSColor::controlAccentColor(),
        Tone::Error => NSColor::systemRedColor(),
        Tone::Paused => NSColor::systemYellowColor(),
    }
}
