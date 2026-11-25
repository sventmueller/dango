//! Hands memory the app let go of back to macOS.

use std::time::Duration;

unsafe extern "C" {
    /// libmalloc: hands the free pages of `zone`, or of every zone when null, back to the system, up to `goal` bytes
    /// or all of them when 0.
    fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
}

/// How long after a large release its pages are handed back, so that what went with it later is gone too.
const SETTLE: Duration = Duration::from_secs(2);

/// Hands the pages that no longer hold anything back to macOS shortly, on a thread of its own; for after a large
/// release such as a closed tab, since the allocator otherwise keeps them until the system runs short..
pub fn give_back_memory_soon() {
    let spawned = super::spawn("give-back", || {
        std::thread::sleep(SETTLE);
        // SAFETY: A null zone stands for every zone; the call returns only pages that no allocation uses.
        unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
    });
    if let Err(why) = spawned {
        crate::diag::report("give-back", why);
    }
}
