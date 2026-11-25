//! Ctrl-C ends the comparison through its normal path, so the instances it started quit and the defaults it touched
//! come back.

use std::sync::atomic::{AtomicBool, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_signal: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Catches SIGINT, SIGTERM and SIGHUP instead of ending at once.
pub(crate) fn install() {
    let handler = on_signal as extern "C" fn(libc::c_int);
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        // SAFETY: the handler only stores into an atomic, which is async-signal-safe.
        unsafe { libc::signal(signal, handler as libc::sighandler_t) };
    }
}

/// An error once a signal asked the comparison to end.
pub(crate) fn check() -> Result<(), String> {
    if INTERRUPTED.load(Ordering::SeqCst) {
        Err("interrupted".into())
    } else {
        Ok(())
    }
}

/// Sleeps for `duration` in short steps, ending early with an error on a signal.
pub(crate) fn sleep(duration: std::time::Duration) -> Result<(), String> {
    let end = std::time::Instant::now() + duration;
    loop {
        check()?;
        let left = end.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Ok(());
        }
        std::thread::sleep(left.min(std::time::Duration::from_millis(20)));
    }
}
