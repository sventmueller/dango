//! The host clock both the posted events and ScreenCaptureKit's frames are stamped with.

/// The ratio of the host clock's ticks to nanoseconds, from `mach_timebase_info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timebase {
    pub(crate) numer: u32,
    pub(crate) denom: u32,
}

impl Timebase {
    /// `ticks` of the host clock in nanoseconds.
    pub(crate) fn ns(self, ticks: u64) -> u64 {
        let denom = u128::from(self.denom.max(1));
        u64::try_from(u128::from(ticks) * u128::from(self.numer) / denom).unwrap_or(u64::MAX)
    }
}

/// Seconds of a `CMTime` on the host clock in nanoseconds; none for a time that is no time.
pub(crate) fn seconds_to_ns(seconds: f64) -> Option<u64> {
    (seconds.is_finite() && seconds >= 0.0).then(|| {
        let ns = (seconds * 1e9).round();
        u64::try_from(crate::compare::stats::whole(ns)).unwrap_or(u64::MAX)
    })
}

#[cfg(target_os = "macos")]
mod host {
    use super::Timebase;

    #[repr(C)]
    struct MachTimebaseInfo {
        numer: u32,
        denom: u32,
    }

    // SAFETY: both functions are part of libSystem with these C signatures and touch no memory but the out-parameter.
    unsafe extern "C" {
        safe fn mach_absolute_time() -> u64;
        fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
    }

    /// This Mac's timebase; 1/1, as on Intel, if the kernel refuses to say.
    pub(crate) fn timebase() -> Timebase {
        let mut info = MachTimebaseInfo { numer: 1, denom: 1 };
        // SAFETY: `info` is a valid, writable MachTimebaseInfo for the duration of the call.
        let status = unsafe { mach_timebase_info(&raw mut info) };
        if status != 0 || info.denom == 0 {
            return Timebase { numer: 1, denom: 1 };
        }
        Timebase {
            numer: info.numer,
            denom: info.denom,
        }
    }

    /// The host clock now, in its ticks.
    pub(crate) fn ticks() -> u64 {
        mach_absolute_time()
    }

    /// The host clock now, in nanoseconds.
    pub(crate) fn now_ns() -> u64 {
        timebase().ns(ticks())
    }
}

#[cfg(target_os = "macos")]
pub(crate) use host::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_silicon_ticks_are_converted_to_nanoseconds() {
        let base = Timebase { numer: 125, denom: 3 };
        assert_eq!(base.ns(24_000_000), 1_000_000_000);
        assert_eq!(base.ns(0), 0);
        assert_eq!(Timebase { numer: 1, denom: 1 }.ns(42), 42);
    }

    #[test]
    fn a_large_tick_count_neither_overflows_nor_wraps() {
        let base = Timebase { numer: 125, denom: 3 };
        assert_eq!(base.ns(u64::MAX), u64::MAX);
        assert_eq!(Timebase { numer: 1, denom: 0 }.ns(7), 7);
    }

    #[test]
    fn seconds_of_the_host_clock_become_nanoseconds() {
        assert_eq!(seconds_to_ns(1.5), Some(1_500_000_000));
        assert_eq!(seconds_to_ns(f64::NAN), None);
        assert_eq!(seconds_to_ns(-1.0), None);
    }
}
