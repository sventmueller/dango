//! Telling the user that a session finished or waits for them while they look elsewhere (I9): a notification and
//! the number of such sessions on the Dock icon.

use std::sync::atomic::{AtomicBool, Ordering};

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSBundle, NSError};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest, UNNotificationSound,
    UNUserNotificationCenter,
};

use ide_ui_common::util::ns;

static ASKED: AtomicBool = AtomicBool::new(false);
static NOTIFIED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Whether notifications may be posted: in a bundle, and not while the self-test runs, where macOS would ask for
/// permission with nobody at the Mac.
fn allowed() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some() && std::env::var_os("IDE_SELFTEST").is_none()
}

/// Whether the user would miss what happens in a tab that is `shown` now.
pub fn user_looks_elsewhere(shown: bool) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    !NSApplication::sharedApplication(mtm).isActive() || !shown
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

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;
