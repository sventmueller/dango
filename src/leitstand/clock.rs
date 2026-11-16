//! The two clocks every input carries (E168): a monotonic one for limits, which starts with the app, and the wall
//! clock for display, quota windows and the day of the daily account.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ide_factory::Stamp;

/// The app's clocks.
pub(super) struct Clock {
    origin: Instant,
    started: u64,
}

impl Default for Clock {
    fn default() -> Self {
        Clock {
            origin: Instant::now(),
            started: wall_now(),
        }
    }
}

/// The wall clock in milliseconds since the Unix epoch.
pub(super) fn wall_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl Clock {
    /// Both readings now.
    pub(super) fn now(&self) -> Stamp {
        let mono = u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        Stamp::new(mono, wall_now())
    }

    /// The wall clock when the app started, which keeps the session keys of two runs apart.
    pub(super) fn run(&self) -> u64 {
        self.started
    }
}

/// The local day of `wall`, such as `2026-11-16`, and the wall clock of its start, in the user's time zone (E168).
pub(super) fn local_day(wall: u64) -> (String, u64) {
    let seconds = libc::time_t::try_from(wall / 1000).unwrap_or(0);
    // SAFETY: An all-zero `tm` is a valid value of the plain C struct, which `localtime_r` overwrites.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: Both pointers are to live locals of the right types for the call, which keeps neither.
    let filled = unsafe { libc::localtime_r(&raw const seconds, &raw mut tm) };
    if filled.is_null() {
        return ("1970-01-01".into(), 0);
    }
    let day = format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday);
    let into_day = i64::from(tm.tm_hour) * 3600 + i64::from(tm.tm_min) * 60 + i64::from(tm.tm_sec);
    let start = (i64::try_from(wall / 1000).unwrap_or(0) - into_day).max(0);
    (day, u64::try_from(start).unwrap_or(0) * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_monotonic_clock_never_goes_back_and_the_day_starts_before_now() {
        let clock = Clock::default();
        let (a, b) = (clock.now(), clock.now());
        assert!(b.mono >= a.mono && a.wall > 1_700_000_000_000);
        let (day, start) = local_day(a.wall);
        assert_eq!(day.len(), 10);
        assert!(start <= a.wall && a.wall - start < 25 * 60 * 60 * 1000);
    }
}
