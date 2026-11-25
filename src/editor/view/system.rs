//! What the view asks of AppKit for itself: hearing the mouse move, and calls on a later turn of the run loop.

use objc2::AnyThread;
use objc2::runtime::Sel;
use objc2_app_kit::{NSTrackingArea, NSTrackingAreaOptions};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::EditorTextView;

impl EditorTextView {
    /// Hears the mouse move over the visible text while the window is key, for hover texts.
    pub(super) fn track_mouse(&self) {
        let options = NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        let ours = self.trackingAreas().iter().any(|area| {
            area.options().contains(options) && area.owner().is_some_and(|o| std::ptr::eq(&*o, self.as_ref()))
        });
        if ours {
            return;
        }
        // SAFETY: The view owns the area and outlives it; AppKit holds the owner weakly.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                options,
                Some(self),
                None,
            )
        };
        self.addTrackingArea(&area);
    }

    /// Runs `selector` on this view after `delay` seconds, replacing a request for it that has not run yet.
    pub(crate) fn later(&self, selector: Sel, delay: f64) {
        use objc2_foundation::NSObjectNSDelayedPerforming;
        let object: &objc2::runtime::NSObject = self;
        // SAFETY: A selector without arguments that this class answers; the run loop retains the target until the call.
        unsafe {
            objc2::runtime::NSObject::cancelPreviousPerformRequestsWithTarget_selector_object(object, selector, None);
            let modes = objc2_foundation::NSArray::from_slice(&[objc2_foundation::NSRunLoopCommonModes]);
            object.performSelector_withObject_afterDelay_inModes(selector, None, delay, &modes);
        }
    }

    /// Runs `selector` on this view on the next turn of the run loop, beside a request for it that has not run yet;
    /// cheaper than `later` for a caller that asks only once.
    pub(super) fn soon(&self, selector: Sel) {
        use objc2_foundation::NSObjectNSDelayedPerforming;
        let object: &objc2::runtime::NSObject = self;
        // SAFETY: A selector without arguments that this class answers; the run loop retains the target until the call.
        unsafe {
            let modes = objc2_foundation::NSArray::from_slice(&[objc2_foundation::NSRunLoopCommonModes]);
            object.performSelector_withObject_afterDelay_inModes(selector, None, 0.0, &modes);
        }
    }
}

/// Runs `f` as the interval `name` on Instruments' Points of Interest track (SN-M1).
pub(in crate::editor) fn step(name: ide_process::signpost::Name, f: impl FnOnce()) {
    let _interval = ide_process::signpost::begin(name);
    f();
}
