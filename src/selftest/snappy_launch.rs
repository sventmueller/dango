//! `snappy-launch`: how long a cold launch takes from the process's start to the first window's first frame and to the
//! first keystroke the app takes in. Reported against the budget of docs/snappy.md (SN-G3), not yet enforced.

use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};
use objc2_foundation::NSPoint;

use super::{check, finish, helpers};
use crate::util::{guarded, ns};

pub(super) const NAME: &str = "snappy-launch";
/// The launch's budget for both, which the scenario reports but does not enforce yet.
const BUDGET: Duration = Duration::from_millis(300);
/// F19, which no shortcut of the app uses; the posted key carries it so the monitor knows its own.
const KEY_CODE: u16 = 0x50;
const KEY_CHARACTERS: &str = "\u{F716}";

/// Microseconds from the process's start to the first frame and to the first key taken in; 0 until then.
static FIRST_FRAME: AtomicU64 = AtomicU64::new(0);
static FIRST_KEY: AtomicU64 = AtomicU64::new(0);

/// How long the process has run, from the start time the kernel keeps for it.
fn since_start() -> Option<Duration> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: The buffer is a `proc_bsdinfo` of the size passed, which PROC_PIDTBSDINFO fills at most.
    let read = unsafe {
        libc::proc_pidinfo(
            i32::try_from(std::process::id()).ok()?,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if read != size {
        return None;
    }
    // SAFETY: proc_pidinfo filled the whole struct, as `read` says.
    let info = unsafe { info.assume_init() };
    let started = UNIX_EPOCH + Duration::new(info.pbi_start_tvsec, 0) + Duration::from_micros(info.pbi_start_tvusec);
    SystemTime::now().duration_since(started).ok()
}

fn note(slot: &AtomicU64) {
    let micros = since_start().map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX));
    let _ = slot.compare_exchange(0, micros.max(1), Ordering::Relaxed, Ordering::Relaxed);
}

fn read(slot: &AtomicU64) -> Option<Duration> {
    let micros = slot.load(Ordering::Relaxed);
    (micros > 0).then(|| Duration::from_micros(micros))
}

/// Notes the first frame once the launch committed it, and posts a key that notes when the app takes it in.
pub(super) fn watch(mtm: MainThreadMarker) {
    crate::util::after_frame(move || {
        note(&FIRST_FRAME);
        let block = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
            guarded(event.as_ptr(), || {
                // SAFETY: AppKit passes a live event for the call.
                let event_ref = unsafe { event.as_ref() };
                if event_ref.keyCode() != KEY_CODE
                    || event_ref.characters().is_none_or(|c| c.to_string() != KEY_CHARACTERS)
                {
                    return event.as_ptr();
                }
                note(&FIRST_KEY);
                std::ptr::null_mut()
            })
        });
        // SAFETY: The block returns the event it was given, or none for the key it posted itself.
        let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block) };
        // The monitor stays for the run, which ends with the scenario.
        std::mem::forget(monitor);
        let app = NSApplication::sharedApplication(mtm);
        let window = app.keyWindow().map_or(0, |window| window.windowNumber());
        let key = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown,
            NSPoint::ZERO,
            NSEventModifierFlags::empty(),
            0.0,
            window,
            None,
            &ns(KEY_CHARACTERS),
            &ns(KEY_CHARACTERS),
            false,
            KEY_CODE,
        );
        if let Some(key) = key {
            app.postEvent_atStart(&key, false);
        }
    });
}

fn shown(what: Option<Duration>) -> String {
    what.map_or_else(|| "never".to_string(), |d| format!("{} ms", d.as_millis()))
}

/// Reports what `watch` noted.
pub(crate) fn run() {
    helpers::wait_within(
        Duration::from_secs(10),
        || read(&FIRST_KEY).is_some(),
        |_| {
            let (frame, key) = (read(&FIRST_FRAME), read(&FIRST_KEY));
            let within = |d: Option<Duration>| {
                if d.is_some_and(|d| d <= BUDGET) {
                    "within"
                } else {
                    "over"
                }
            };
            check(
                "a cold launch draws its first window and takes the first key",
                frame.is_some() && key.is_some(),
                format!(
                    "first frame after {} ({} the budget of {} ms), first key after {} ({} it)",
                    shown(frame),
                    within(frame),
                    BUDGET.as_millis(),
                    shown(key),
                    within(key),
                ),
            );
            finish(NAME);
        },
    );
}
