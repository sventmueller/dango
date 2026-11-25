//! Time as the app hands it in: a monotonic clock for every limit and the wall clock only for display, quota windows
//! and the day of the daily account (E168).

use serde::{Deserialize, Serialize};

/// A reading of the monotonic clock in milliseconds; it starts over with every boot of the app.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Millis(pub u64);

impl Millis {
    /// The reading `ms` milliseconds later.
    pub fn after(self, ms: u64) -> Millis {
        Millis(self.0.saturating_add(ms))
    }

    /// The milliseconds since `earlier`, zero if `earlier` lies ahead.
    pub fn since(self, earlier: Millis) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// When something happened: the monotonic reading and the wall clock in milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    /// The monotonic reading, for limits.
    pub mono: Millis,
    /// The wall clock, for display and windows that reset at a time of day.
    pub wall: u64,
}

impl Stamp {
    /// A stamp from both readings.
    pub fn new(mono: u64, wall: u64) -> Stamp {
        Stamp {
            mono: Millis(mono),
            wall,
        }
    }
}

/// Milliseconds in a second.
pub(crate) const SECOND: u64 = 1000;
/// Milliseconds in a minute.
pub(crate) const MINUTE: u64 = 60 * SECOND;
