//! Updates through Sparkle, which only the distributed disk image carries (E2). Sparkle asks on the second launch
//! whether it may check on its own; until then, and in builds without it, the app makes no request.

use std::cell::OnceCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::NSMenuItem;
use objc2_foundation::{NSBundle, NSURL};
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
    let frameworks = NSBundle::mainBundle().privateFrameworksURL()?;
    load_from(&frameworks)
}

/// Starts the updater of the `Sparkle.framework` in the folder `frameworks`.
fn load_from(frameworks: &NSURL) -> Option<Retained<AnyObject>> {
    let url = frameworks.URLByAppendingPathComponent(&ns("Sparkle.framework"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ide-updates-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn url(dir: &std::path::Path) -> Retained<NSURL> {
        NSURL::fileURLWithPath_isDirectory(&ns(&dir.to_string_lossy()), true)
    }

    #[test]
    fn without_sparkle_there_is_no_updater() {
        let dir = folder("none");
        assert!(load_from(&url(&dir)).is_none());
    }

    #[test]
    fn a_sparkle_that_does_not_load_starts_no_updater() {
        let dir = folder("broken");
        let resources = dir.join("Sparkle.framework/Resources");
        std::fs::create_dir_all(&resources).unwrap();
        std::fs::write(
            resources.join("Info.plist"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
             <key>CFBundleExecutable</key><string>Sparkle</string>\
             <key>CFBundleIdentifier</key><string>local.test.sparkle</string>\
             <key>CFBundlePackageType</key><string>FMWK</string></dict></plist>\n",
        )
        .unwrap();
        assert!(NSBundle::bundleWithURL(&url(&dir.join("Sparkle.framework"))).is_some());
        assert!(load_from(&url(&dir)).is_none());
    }
}
