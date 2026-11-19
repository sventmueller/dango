//! Views that know when they come into view and when they have stayed out of it: lists that load anew only while
//! they are in view, and views that let go of what they built while out of it.
//!
//! On macOS 27 every `reloadData` of an outline view leaves the disclosure button of each expandable row it shows
//! behind for good, so a list that reloads out of sight piles them up for nothing.

use std::cell::Cell;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSView;
use objc2_foundation::NSObjectProtocol;

use crate::util::guarded;

/// Whether `view` is out of view: in no window, or hidden itself or by an ancestor.
pub fn out_of_view(view: &NSView) -> bool {
    view.window().is_none() || view.isHiddenOrHasHiddenAncestor()
}

struct Watch {
    came: Box<dyn Fn()>,
    /// What runs once the view stayed out of view for the time given, in seconds.
    away: Option<(Box<dyn Fn()>, f64)>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `InViewWatch` does not implement Drop.
    /// An empty view that tells when it, or the view it sits in, comes into view and when it stayed out of it.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEInViewWatch"]
    #[ivars = Watch]
    struct InViewWatch;

    unsafe impl NSObjectProtocol for InViewWatch {}

    impl InViewWatch {
        #[unsafe(method(viewDidUnhide))]
        fn did_unhide(&self) {
            guarded((), || self.came());
        }

        #[unsafe(method(viewDidHide))]
        fn did_hide(&self) {
            guarded((), || self.went());
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn did_move_to_window(&self) {
            guarded((), || {
                if self.window().is_some() {
                    self.came();
                } else {
                    self.went();
                }
            });
        }

        #[unsafe(method(ideCameIntoView))]
        fn came_into_view(&self) {
            guarded((), || {
                if !out_of_view(self) {
                    (self.ivars().came)();
                }
            });
        }

        #[unsafe(method(ideStayedAway))]
        fn stayed_away(&self) {
            guarded((), || {
                if let Some((away, _)) = &self.ivars().away
                    && out_of_view(self)
                {
                    away();
                }
            });
        }
    }
);

impl InViewWatch {
    /// Runs `came` once the change of the view hierarchy that brought the view into view is done.
    fn came(&self) {
        self.cancel(sel!(ideStayedAway));
        self.cancel(sel!(ideCameIntoView));
        // SAFETY: `ideCameIntoView` takes no argument and returns nothing; the run loop keeps the watch until it fires.
        let _: () = unsafe {
            msg_send![self, performSelector: sel!(ideCameIntoView), withObject: None::<&AnyObject>, afterDelay: 0.0]
        };
    }

    fn went(&self) {
        self.cancel(sel!(ideCameIntoView));
        let Some((_, after)) = &self.ivars().away else { return };
        self.cancel(sel!(ideStayedAway));
        // SAFETY: `ideStayedAway` takes no argument and returns nothing; the run loop keeps the watch until it fires.
        let _: () = unsafe {
            msg_send![self, performSelector: sel!(ideStayedAway), withObject: None::<&AnyObject>, afterDelay: *after]
        };
    }

    fn cancel(&self, selector: objc2::runtime::Sel) {
        // SAFETY: NSObject's class method, with the target, selector and argument the watch scheduled.
        let _: () = unsafe {
            msg_send![NSObject::class(), cancelPreviousPerformRequestsWithTarget: self, selector: selector, object: None::<&AnyObject>]
        };
    }
}

/// Puts a watch into `view` that runs `came` whenever `view` or a view around it comes into view, and `away` once it
/// has stayed out of view for `after`.
pub fn watch_view(view: &NSView, came: impl Fn() + 'static, away: Option<(Duration, Box<dyn Fn()>)>) {
    let mtm = MainThreadMarker::from(view);
    let watch = Watch {
        came: Box::new(came),
        away: away.map(|(after, away)| (away, after.as_secs_f64())),
    };
    let watch = InViewWatch::alloc(mtm).set_ivars(watch);
    // SAFETY: NSView's initializer, called once on the newly allocated object.
    let watch: Retained<InViewWatch> = unsafe { msg_send![super(watch), init] };
    view.addSubview(&watch);
}

/// Whether a list missed a reload while it was out of view.
#[derive(Default)]
pub struct HeldReload {
    stale: Cell<bool>,
}

impl HeldReload {
    /// Runs `came` each time `page` or a view around it comes into view; `came` should catch up with
    /// [`HeldReload::catch_up`].
    pub fn watch(page: &NSView, came: impl Fn() + 'static) {
        watch_view(page, came, None);
    }

    /// Whether a reload of the list in `view` waits, because `view` is out of view; it then runs on coming back.
    pub fn holds(&self, view: &NSView) -> bool {
        let away = out_of_view(view);
        self.stale.set(away);
        away
    }

    /// Whether the list in `view` missed a reload and is in view again, so it should reload now.
    pub fn catch_up(&self, view: &NSView) -> bool {
        self.stale.get() && !out_of_view(view)
    }
}
