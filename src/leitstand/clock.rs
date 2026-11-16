//! The two clocks every input carries (E168): a monotonic one for limits, which starts with the app, and the wall
//! clock for display, quota windows and the day of the daily account.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ide_factory::Stamp;
use ide_factory::budget::DailyAccount;

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

/// `account` if it is of the local day of `wall`, else a new, empty one for that day; either way with when the day
/// ends, so a folder's factory starts the next day by itself (E168).
pub(super) fn today(account: &DailyAccount, wall: u64) -> DailyAccount {
    let (day, start) = local_day(wall);
    let (_, ends) = local_day(start + 26 * 60 * 60 * 1000);
    if account.day() == day {
        account.clone().until(ends)
    } else {
        DailyAccount::new(day).until(ends)
    }
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

    #[test]
    fn the_daily_account_ends_with_its_day_and_a_new_day_starts_empty() {
        let now = wall_now();
        let (day, start) = local_day(now);
        let mut account = DailyAccount::new(day.clone());
        account.add(&day, None, &ide_factory::budget::Spend::new(2.0, 0, 0.0));
        let kept = today(&account, now);
        assert_eq!(kept.total().dollars(), 2.0);
        assert!(kept.current(now) && kept.ends() > now);
        let (next, next_start) = local_day(kept.ends());
        assert_ne!(next, day);
        assert_eq!(next_start, kept.ends());
        assert!(kept.ends() - start >= 23 * 60 * 60 * 1000);
        let fresh = today(&kept, kept.ends());
        assert_eq!((fresh.day(), fresh.total().dollars()), (next.as_str(), 0.0));
    }
}
