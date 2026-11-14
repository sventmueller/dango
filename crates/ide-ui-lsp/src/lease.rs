//! What keeps a language server running besides the editors that use it. A server ends five minutes after the last
//! editor let go of it (E10); the project symbol search holds servers through a lease of its own, which ends five
//! minutes after the last search (review C17), whether an editor ever used the server or not.

use std::time::{Duration, Instant};

/// How long a server keeps running after the last editor it serves closed, so reopening one soon costs no new
/// indexing; and how long a lease lasts after the last symbol search.
pub(super) const IDLE_STOP: Duration = Duration::from_secs(5 * 60);

/// When editors and searches last let go of a server.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Hold {
    released: Option<Instant>,
    leased: Option<Instant>,
}

impl Hold {
    /// An editor let go of the server at `now`.
    pub(super) fn release(&mut self, now: Instant) {
        self.released = Some(now);
    }

    /// A symbol search used the server at `now`.
    pub(super) fn lease(&mut self, now: Instant) {
        self.leased = Some(now);
    }

    /// The search gives the server up, as one that cannot search symbols.
    pub(super) fn end_lease(&mut self) {
        self.leased = None;
    }

    /// Whether a symbol search used the server within `IDLE_STOP` before `now`.
    pub(super) fn leased(&self, now: Instant) -> bool {
        self.leased.is_some_and(|at| now < at + IDLE_STOP)
    }

    /// Whether the server should end at `now`, with `editors` editors using it: none does, no lease holds it, and
    /// the grace after the last editor let go is over.
    pub(super) fn due(&self, now: Instant, editors: usize) -> bool {
        editors == 0 && !self.leased(now) && self.released.is_none_or(|at| now >= at + IDLE_STOP)
    }

    /// When to look again whether the server should end, if it should not at `now`: when the later of the lease and
    /// the grace runs out.
    pub(super) fn next_check(&self, now: Instant) -> Instant {
        [self.released, self.leased]
            .into_iter()
            .flatten()
            .map(|at| at + IDLE_STOP)
            .filter(|at| *at > now)
            .max()
            .unwrap_or(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    #[test]
    fn a_lease_without_an_editor_ends_five_minutes_after_the_last_search() {
        let start = Instant::now();
        let mut hold = Hold::default();
        hold.lease(start);
        assert!(!hold.due(start + 4 * MINUTE, 0));
        hold.lease(start + 4 * MINUTE);
        assert!(!hold.due(start + 8 * MINUTE, 0), "the second search renews the lease");
        assert_eq!(hold.next_check(start + 8 * MINUTE), start + 9 * MINUTE);
        assert!(hold.due(start + 9 * MINUTE, 0));
    }

    #[test]
    fn an_editor_or_a_lease_each_keep_the_server() {
        let start = Instant::now();
        let mut hold = Hold::default();
        hold.lease(start);
        assert!(
            !hold.due(start + 10 * MINUTE, 1),
            "an open editor holds it after the lease ran out"
        );
        hold.release(start + 10 * MINUTE);
        assert!(!hold.due(start + 14 * MINUTE, 0), "the editor's grace holds it");
        assert!(hold.due(start + 15 * MINUTE, 0));
        hold.lease(start + 15 * MINUTE);
        assert!(!hold.due(start + 16 * MINUTE, 0));
    }

    #[test]
    fn a_server_without_symbol_search_loses_its_lease_at_once() {
        let start = Instant::now();
        let mut hold = Hold::default();
        hold.lease(start);
        hold.end_lease();
        assert!(hold.due(start, 0));
        assert_eq!(hold.next_check(start), start);
        assert!(
            !Hold::default().due(start, 1),
            "a server only editors use runs while they do"
        );
    }
}
