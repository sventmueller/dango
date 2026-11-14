//! The self-test's hooks of `notify.rs`.

use super::*;

/// How many notifications were posted, or would have been where posting is not allowed.
pub fn posted() -> usize {
    NOTIFIED.load(Ordering::Relaxed)
}
