//! Notices when the main thread holds up input: the time between two steps of its run loop, without the time it
//! sleeps, is how long a keystroke or a mouse event would have waited. Costs nothing while the app is idle, as the run
//! loop only calls the observer when it runs.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopActivity, CFRunLoopObserver, kCFRunLoopCommonModes};

/// Longer than this is a stall the user feels: three frames at 60 Hz.
pub(crate) const STALL: Duration = Duration::from_millis(50);

static START: OnceLock<Instant> = OnceLock::new();
/// When the run loop last stepped, in microseconds since `START`.
static LAST: AtomicU64 = AtomicU64::new(0);
/// The longest step since the last [`take_worst`], in microseconds.
static WORST: AtomicU64 = AtomicU64::new(0);
static OBSERVER: OnceLock<usize> = OnceLock::new();

fn now() -> u64 {
    let start = START.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// Starts watching the main run loop; once, on the main thread.
pub fn install() {
    if OBSERVER.get().is_some() {
        return;
    }
    let Some(main) = CFRunLoop::main() else { return };
    let block = block2::RcBlock::new(|_: *mut CFRunLoopObserver, activity: CFRunLoopActivity| {
        super::guarded((), || stepped(activity));
    });
    // SAFETY: The default allocator and a block that only touches atomics; the run loop calls it on the main thread.
    let observer: Option<CFRetained<CFRunLoopObserver>> =
        unsafe { CFRunLoopObserver::with_handler(None, CFRunLoopActivity::AllActivities.0, true, 0, Some(&block)) };
    let Some(observer) = observer else { return };
    // SAFETY: Reading a constant CoreFoundation exports.
    let common = unsafe { kCFRunLoopCommonModes };
    main.add_observer(Some(&observer), common);
    LAST.store(now(), Ordering::Relaxed);
    // The run loop holds the observer as long as the app runs; this keeps the one reference that made it.
    let _ = OBSERVER.set(CFRetained::into_raw(observer).as_ptr() as usize);
}

fn stepped(activity: CFRunLoopActivity) {
    let now = now();
    let last = LAST.swap(now, Ordering::Relaxed);
    // Waking up ends a sleep, which is not work; entering starts a run whose wait before is the caller's.
    if activity.contains(CFRunLoopActivity::AfterWaiting) || activity.contains(CFRunLoopActivity::Entry) {
        return;
    }
    let took = now.saturating_sub(last);
    WORST.fetch_max(took, Ordering::Relaxed);
    if took >= u64::try_from(STALL.as_micros()).unwrap_or(u64::MAX) {
        ide_core::diag::report("main thread", format!("held up input for {} ms", took / 1000));
    }
}

/// The longest the main thread held up input since the last call, which starts the count again.
pub fn take_worst() -> Duration {
    Duration::from_micros(WORST.swap(0, Ordering::Relaxed))
}
