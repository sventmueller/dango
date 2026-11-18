//! Keeping the Mac awake while orders run (E169, A18), and the Mac's sleep, wake and battery as inputs (E168): one
//! `NSProcessInfo` activity with `NSActivityUserInitiated`, which also keeps App Nap off, held while any folder's
//! factory asks for it; the workspace's sleep and wake notifications; and the power source, read with each tick.
//! Self-tests hold no activity (R12).

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use ide_factory::InputKind;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification};
use objc2_foundation::{
    NSActivityOptions, NSNotification, NSObjectProtocol, NSOperationQueue, NSProcessInfo, NSString,
};

use super::{Delivery, Leitstand};
use crate::context::AppContext;
use crate::util::guarded;

/// What keeps the Mac awake and tells of its sleep.
#[derive(Default)]
pub(super) struct Power {
    activity: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    /// How many orders run in each folder whose factory holds the wake lock.
    holders: BTreeMap<PathBuf, usize>,
    observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    battery: Option<bool>,
}

impl Drop for Power {
    fn drop(&mut self) {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        for observer in self.observers.drain(..) {
            // SAFETY: The token came from this center's block-based `addObserverForName` and is removed only here.
            unsafe { center.removeObserver(observer.as_ref()) };
        }
        end(self.activity.take());
    }
}

fn end(activity: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>) {
    if let Some(activity) = activity {
        // SAFETY: The activity came from this process info's `beginActivityWithOptions` and ends only once.
        unsafe { NSProcessInfo::processInfo().endActivity(&activity) };
    }
}

/// Whether a self-test runs, which must not keep the Mac awake.
fn in_selftest() -> bool {
    std::env::var_os("IDE_SELFTEST").is_some()
}

#[cfg(feature = "selftest")]
impl Power {
    /// Whether a factory holds the wake lock now, which a self-test reads without the activity behind it.
    pub(super) fn requested(&self) -> bool {
        !self.holders.is_empty()
    }

    /// Whether the activity that keeps the Mac awake is held.
    pub(super) fn holds(&self) -> bool {
        self.activity.is_some()
    }
}

/// Whether the folder's factory at `root` holds the wake lock for `running` orders; the activity follows the sum.
pub(super) fn hold(leitstand: &Leitstand, root: &Path, hold: bool, running: usize) {
    #[cfg(feature = "selftest")]
    leitstand.probe.note_wake(hold);
    let mut shared = leitstand.shared.borrow_mut();
    let power = &mut shared.power;
    if hold {
        power.holders.insert(root.to_path_buf(), running);
    } else {
        power.holders.remove(root);
    }
    let total: usize = power.holders.values().sum();
    end(power.activity.take());
    if total == 0 || in_selftest() {
        return;
    }
    let reason = splat_i18n::tf("Aufträge laufen: {}", &[&total.to_string()]);
    let activity = NSProcessInfo::processInfo()
        .beginActivityWithOptions_reason(NSActivityOptions::UserInitiated, &NSString::from_str(&reason));
    power.activity = Some(activity);
}

/// Watches the workspace for the Mac's sleep and wake, once.
pub(super) fn watch_sleep(leitstand: &Leitstand) {
    let mut shared = leitstand.shared.borrow_mut();
    if !shared.power.observers.is_empty() {
        return;
    }
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let queue = NSOperationQueue::mainQueue();
    let back = leitstand.back();
    for (asleep, name) in [
        // SAFETY: The names are the framework's own constants, which live as long as the process.
        (true, unsafe { NSWorkspaceWillSleepNotification }),
        // SAFETY: As above.
        (false, unsafe { NSWorkspaceDidWakeNotification }),
    ] {
        let back = back.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            guarded((), || back.deliver(Delivery::Sleep(asleep)));
        });
        // SAFETY: The block captures a `bool` and the way back, which may go to any thread; the main queue runs it on
        // the main thread, and the token is removed when the Leitstand goes.
        let token =
            unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, Some(&queue), &block) };
        shared.power.observers.push(token);
    }
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> *const c_void;
    fn IOPSGetProvidingPowerSourceType(snapshot: *const c_void) -> *const c_void;
}

unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// Whether the Mac runs on battery now; `None` when the power sources cannot be read.
fn on_battery() -> Option<bool> {
    // SAFETY: The function takes nothing and returns a snapshot the caller owns, or null.
    let snapshot = unsafe { IOPSCopyPowerSourcesInfo() };
    if snapshot.is_null() {
        return None;
    }
    // SAFETY: `snapshot` is the live snapshot from above; the string returned belongs to it and is read before it is
    // released.
    let kind = unsafe { IOPSGetProvidingPowerSourceType(snapshot) };
    let battery = (!kind.is_null()).then(|| {
        // SAFETY: A CFString is toll-free bridged to NSString, and `kind` lives as long as `snapshot`.
        let name = unsafe { &*kind.cast::<NSString>() };
        name.to_string() == "Battery Power"
    });
    // SAFETY: The snapshot was copied above and is released once.
    unsafe { CFRelease(snapshot) };
    battery
}

/// Reads the power source and tells every folder when it changed (E169).
pub(super) fn poll_battery(leitstand: &Leitstand, cx: &Rc<AppContext>) {
    let Some(now) = on_battery() else { return };
    let changed = leitstand.shared.borrow_mut().power.battery.replace(now) != Some(now);
    if changed {
        leitstand.broadcast(cx, &InputKind::Battery(now));
    }
}

/// The power source as last read, for a folder whose factory starts now.
pub(super) fn battery(leitstand: &Leitstand) -> Option<bool> {
    leitstand.shared.borrow().power.battery.or_else(on_battery)
}
