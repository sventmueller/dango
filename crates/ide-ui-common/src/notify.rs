//! Telling the user that a session finished or waits for them while they look elsewhere (I9): a notification, and
//! what hears of a click on one.

use std::sync::atomic::{AtomicBool, Ordering};

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, define_class, msg_send};
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSBundle, NSError, NSObjectProtocol};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use crate::util::{guarded, ns};

static ASKED: AtomicBool = AtomicBool::new(false);
static NOTIFIED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Whether notifications may be posted: in a bundle, and not while the self-test runs, where macOS would ask for
/// permission with nobody at the Mac.
fn allowed() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some() && std::env::var_os("IDE_SELFTEST").is_none()
}

/// Whether another app is in front, the only time a session's notification is posted; a tab in the background of
/// the active app shows its dot instead.
pub fn app_in_background() -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    !NSApplication::sharedApplication(mtm).isActive()
}

/// Posts a notification; asks for permission the first time.
pub fn post(id: &str, title: &str, body: &str) {
    NOTIFIED.fetch_add(1, Ordering::Relaxed);
    if !allowed() {
        return;
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    if !ASKED.swap(true, Ordering::SeqCst) {
        let done = RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &done,
        );
    }
    center.addNotificationRequest_withCompletionHandler(&request(id, title, body), None);
}

/// Whether a notification would reach the user now; never while the self-test runs (R12).
pub fn may_post() -> bool {
    allowed()
}

/// What a click on a notification hands the identifier it was posted under.
type Clicked = Box<dyn Fn(String) + Send + Sync>;

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Clicks` does not implement Drop.
    /// What hears of the user's clicks on the app's notifications, as the notification center's delegate.
    #[unsafe(super(NSObject))]
    #[name = "IDENotificationClicks"]
    #[ivars = Clicked]
    pub struct Clicks;

    unsafe impl NSObjectProtocol for Clicks {}

    unsafe impl UNUserNotificationCenterDelegate for Clicks {
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &DynBlock<dyn Fn()>,
        ) {
            guarded((), || {
                (self.ivars())(response.notification().request().identifier().to_string());
            });
            completion.call(());
        }
    }
);

/// Hands `clicked` the identifier of each notification the user clicks, on whichever thread macOS tells of it, for as
/// long as the app keeps what this returns; none where notifications are not posted.
pub fn follow_clicks(clicked: impl Fn(String) + Send + Sync + 'static) -> Option<Retained<Clicks>> {
    if !allowed() {
        return None;
    }
    let clicked: Clicked = Box::new(clicked);
    let this = Clicks::alloc().set_ivars(clicked);
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let clicks: Retained<Clicks> = unsafe { msg_send![super(this), init] };
    UNUserNotificationCenter::currentNotificationCenter().setDelegate(Some(ProtocolObject::from_ref(&*clicks)));
    Some(clicks)
}

/// How many notifications were posted, or would have been where posting is not allowed, for the self-test.
pub fn posted() -> usize {
    NOTIFIED.load(Ordering::Relaxed)
}

/// The notification for the tab `id`, with a sound, shown at once.
fn request(id: &str, title: &str, body: &str) -> Retained<UNNotificationRequest> {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&ns(title));
    content.setBody(&ns(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    UNNotificationRequest::requestWithIdentifier_content_trigger(&ns(id), &content, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notification_names_its_tab_and_says_what_happened_at_once() {
        let request = request("agent-7", "Fix the parser", "Claude ist fertig.");
        let content = request.content();
        assert_eq!(request.identifier().to_string(), "agent-7");
        assert_eq!(content.title().to_string(), "Fix the parser");
        assert_eq!(content.body().to_string(), "Claude ist fertig.");
        assert!(content.sound().is_some());
        assert!(request.trigger().is_none(), "shown at once, not scheduled");
    }

    #[test]
    fn the_test_run_posts_nothing_and_counts_what_it_would_have_posted() {
        let before = NOTIFIED.load(Ordering::Relaxed);
        post("agent-8", "Title", "Body");
        assert!(!allowed());
        assert!(NOTIFIED.load(Ordering::Relaxed) > before);
    }
}
