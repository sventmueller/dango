//! Telling the user that a session finished or waits for them while they look elsewhere (I9): a notification and
//! the number of such sessions on the Dock icon.

use std::sync::atomic::{AtomicBool, Ordering};

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::runtime::Bool;
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSBundle, NSError};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest, UNNotificationSound,
    UNUserNotificationCenter,
};

use crate::util::ns;

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
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound | UNAuthorizationOptions::Badge,
            &done,
        );
    }
    let content = UNMutableNotificationContent::new();
    content.setTitle(&ns(title));
    content.setBody(&ns(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&ns(id), &content, None);
    center.addNotificationRequest_withCompletionHandler(&request, None);
}

/// How many notifications were posted, or would have been where posting is not allowed.
#[cfg(feature = "selftest")]
pub fn posted() -> usize {
    NOTIFIED.load(Ordering::Relaxed)
}

/// Shows on the Dock icon how many sessions wait for the user or finished unseen.
pub fn refresh_badge() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let count = super::all().iter().filter(|t| t.wants_attention()).count();
    let label = (count > 0).then(|| ns(&count.to_string()));
    NSApplication::sharedApplication(mtm)
        .dockTile()
        .setBadgeLabel(label.as_deref());
}
