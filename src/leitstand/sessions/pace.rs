//! When what a session reported reaches the main thread: on the run loop's next turn, so it shows in the next frame,
//! but at most once a frame for each session, and less often while taking it in costs much of a frame.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ide_core::LockIgnoringPoison;
use ide_session::SessionId;

/// A frame at 120 Hz.
const FRAME: Duration = Duration::from_millis(8);

/// The longest a session waits between two hops, however much taking its events in costs.
const SLOWEST: Duration = Duration::from_millis(30);

/// A streaming session's hops take at most about a quarter of the main thread.
const SHARE: u32 = 4;

/// How long a session that reports nothing is remembered.
const FORGOTTEN: Duration = Duration::from_secs(60);

#[derive(Clone, Copy)]
struct Paced {
    /// When the latest hop was due.
    due: Instant,
    /// How long taking in the latest hop took on the main thread.
    cost: Duration,
}

/// The hops of each session, shared between the threads that wake them and the main thread that takes them.
pub(super) struct Pace<K = SessionId> {
    sessions: Mutex<HashMap<K, Paced>>,
}

impl<K> Default for Pace<K> {
    fn default() -> Pace<K> {
        Pace {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Hash + Eq + Copy> Pace<K> {
    /// How long the hop of `session` that is asked for at `now` waits.
    pub(super) fn wait(&self, session: K, now: Instant) -> Duration {
        let mut sessions = self.sessions.lock_ignoring_poison();
        let due = match sessions.get(&session) {
            Some(paced) => (paced.due + (paced.cost * SHARE).clamp(FRAME, SLOWEST)).max(now),
            None => {
                sessions.retain(|_, paced| now.saturating_duration_since(paced.due) < FORGOTTEN);
                now
            }
        };
        let cost = sessions.get(&session).map_or(Duration::ZERO, |paced| paced.cost);
        sessions.insert(session, Paced { due, cost });
        due - now
    }

    /// Notes that taking in the latest hop of `session` took `cost`.
    pub(super) fn took(&self, session: K, cost: Duration) {
        if let Some(paced) = self.sessions.lock_ignoring_poison().get_mut(&session) {
            paced.cost = cost;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn the_first_hop_goes_at_once_and_the_next_one_a_frame_after_it() {
        let pace = Pace::<u32>::default();
        let start = Instant::now();
        let session = 1;
        assert_eq!(pace.wait(session, start), Duration::ZERO);
        assert_eq!(pace.wait(session, start + 2 * MS), 6 * MS);
        assert_eq!(pace.wait(session, start + 20 * MS), Duration::ZERO);
        assert_eq!(
            pace.wait(2, start + 20 * MS),
            Duration::ZERO,
            "each session has its own frame"
        );
    }

    #[test]
    fn a_costly_hop_spaces_the_next_ones_up_to_the_slowest() {
        let pace = Pace::<u32>::default();
        let start = Instant::now();
        let session = 1;
        pace.wait(session, start);
        pace.took(session, 5 * MS);
        assert_eq!(pace.wait(session, start + 5 * MS), 15 * MS);
        pace.took(session, 50 * MS);
        assert_eq!(pace.wait(session, start + 20 * MS), SLOWEST);
    }

    #[test]
    fn a_quiet_session_is_forgotten() {
        let pace = Pace::<u32>::default();
        let start = Instant::now();
        pace.wait(1, start);
        pace.wait(2, start + FORGOTTEN + MS);
        assert_eq!(pace.sessions.lock_ignoring_poison().len(), 1);
    }
}
