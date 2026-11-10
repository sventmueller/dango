//! Updates through Sparkle, which only the distributed disk image carries (E2). Sparkle asks on the second launch
//! whether it may check on its own; until then, and in builds without it, the app makes no request.

use std::cell::OnceCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::NSMenuItem;
use objc2_foundation::NSBundle;
use splat_i18n::t;

use crate::util::{item, ns, set_target};

thread_local! {
    static CONTROLLER: OnceCell<Option<Retained<AnyObject>>> = const { OnceCell::new() };
}

/// Loads Sparkle from the app's Frameworks folder and starts the updater, which runs the checks the user allowed.
/// Without Sparkle, as in local builds, nothing happens.
pub fn start(_mtm: MainThreadMarker) {
    CONTROLLER.with(|c| {
        c.get_or_init(load);
    });
}

fn load() -> Option<Retained<AnyObject>> {
    let url = NSBundle::mainBundle()
        .privateFrameworksURL()?
        .URLByAppendingPathComponent(&ns("Sparkle.framework"))?;
    let bundle = NSBundle::bundleWithURL(&url)?;
    // SAFETY: Loading runs no initializers that need the app. The hardened runtime refuses the framework unless it
    // carries the app's Team ID, which only the Developer ID signature has.
    if !unsafe { bundle.load() } {
        return None;
    }
    let class = AnyClass::get(c"SPUStandardUpdaterController")?;
    // SAFETY: Sparkle's documented initializer; both delegates are optional.
    unsafe {
        let allocated: Allocated<AnyObject> = msg_send![class, alloc];
        msg_send![
            allocated,
            initWithStartingUpdater: true,
            updaterDelegate: None::<&AnyObject>,
            userDriverDelegate: None::<&AnyObject>
        ]
    }
}

/// The app menu's item that checks for updates now, or none without Sparkle; Sparkle enables it while no check runs.
pub fn menu_item(mtm: MainThreadMarker) -> Option<Retained<NSMenuItem>> {
    let controller = CONTROLLER.with(|c| c.get().cloned().flatten())?;
    let check = item(mtm, t("Nach Updates suchen …"), Some(sel!(checkForUpdates:)), "");
    set_target(&check, &controller);
    Some(check)
}
