use std::ptr::NonNull;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSNotificationName, NSObjectProtocol};

use super::guarded;

/// A watch on the default notification center that ends when the value drops, so it lives exactly as long as what
/// keeps it.
#[must_use = "the watch ends when this drops"]
pub struct Observation {
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for Observation {
    fn drop(&mut self) {
        // SAFETY: The token came from this center's block-based `addObserverForName` and is removed only here, once.
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(self.token.as_ref()) };
    }
}

/// Runs `f`, guarded, for each notification `name` that `object` posts on the main thread, or any sender when
/// `None`; the watch ends when the returned value drops.
pub fn observe(
    _mtm: MainThreadMarker,
    name: &NSNotificationName,
    object: Option<&AnyObject>,
    f: impl Fn(&NSNotification) + 'static,
) -> Observation {
    let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        // SAFETY: The center hands the block a notification it keeps alive for the length of the call.
        let notification = unsafe { notification.as_ref() };
        guarded((), || f(notification));
    });
    // SAFETY: The center copies the block, which owns everything it captures; without a queue it runs on the posting
    // thread, and the names observed here are view and window notifications, which AppKit posts on the main thread
    // that the marker shows this is.
    let token = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(name),
            object,
            None,
            &block,
        )
    };
    Observation { token }
}
