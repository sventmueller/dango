//! Quotas per account, billing service and window (C65), with the time each was observed. A value older than its
//! window, or from a window that has since reset, counts as unknown; the app keeps the book in its state so the
//! fleet line shows it while no session runs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which quota: an account at a billing service, in one of its windows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QuotaKey {
    /// The account, such as an e-mail address or an organisation id.
    pub account: String,
    /// The billing service, such as `claude-subscription`; the vendor of the model is a separate thing (E171).
    pub service: String,
    /// The window's name, such as `five_hour`.
    pub window: String,
}

/// Whether a window still lets requests through, as the path reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaStatus {
    /// Requests go through.
    Allowed,
    /// Requests go through, but the limit is close.
    Warning,
    /// Requests are refused until the window resets.
    Rejected,
}

/// One observation of a quota.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaObservation {
    /// Which quota.
    pub key: QuotaKey,
    /// The share used, from 0 to 1, when the path says.
    pub utilization: Option<f64>,
    /// Whether requests go through.
    pub status: QuotaStatus,
    /// How long the window is, in seconds.
    pub window_seconds: u64,
    /// When the window starts over, wall clock in milliseconds.
    pub resets_at: Option<u64>,
    /// When it was observed, wall clock in milliseconds.
    pub observed_at: u64,
}

/// What is known about the quota of an account and service.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum QuotaState {
    /// Nothing current is known; the evidence says so (E170).
    Unknown,
    /// The fullest current window.
    Known {
        /// Its share used, from 0 to 1; a refused window counts as full.
        utilization: f64,
        /// Whether it refuses requests.
        rejected: bool,
    },
}

impl QuotaState {
    /// Whether a station may start under `limit`, the share from 0 to 1 of E168's start rule (R11); unknown allows
    /// it, as nothing says otherwise.
    pub fn allows_start(&self, limit: f64) -> bool {
        match *self {
            QuotaState::Unknown => true,
            QuotaState::Known { utilization, rejected } => !rejected && utilization < limit,
        }
    }

    /// Whether the quota is spent, which hands a running station to the next entry (E170).
    pub fn spent(&self) -> bool {
        matches!(*self, QuotaState::Known { rejected: true, .. })
            || matches!(*self, QuotaState::Known { utilization, .. } if utilization >= 1.0)
    }
}

/// The quotas the app observed, newest observation per key.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QuotaBook {
    entries: BTreeMap<QuotaKey, QuotaObservation>,
}

impl QuotaBook {
    /// Keeps `observation` unless the book holds a newer one of its key.
    pub fn observe(&mut self, observation: QuotaObservation) {
        let newer = self
            .entries
            .get(&observation.key)
            .is_some_and(|known| known.observed_at > observation.observed_at);
        if !newer {
            self.entries.insert(observation.key.clone(), observation);
        }
    }

    /// Every observation, for the fleet line.
    pub fn observations(&self) -> impl Iterator<Item = &QuotaObservation> {
        self.entries.values()
    }

    /// Whether `observation` is still current at wall-clock `now`.
    pub fn current(observation: &QuotaObservation, now: u64) -> bool {
        let age = now.saturating_sub(observation.observed_at);
        let reset = observation.resets_at.is_some_and(|at| at <= now);
        age <= observation.window_seconds.saturating_mul(1000) && !reset
    }

    /// The quota of `account` at `service` at wall-clock `now`: the fullest of its current windows.
    pub fn state(&self, account: &str, service: &str, now: u64) -> QuotaState {
        self.entries
            .values()
            .filter(|o| o.key.account == account && o.key.service == service && QuotaBook::current(o, now))
            .filter_map(|o| match o.status {
                QuotaStatus::Rejected => Some((1.0, true)),
                _ => o.utilization.map(|u| (u, false)),
            })
            .fold(QuotaState::Unknown, |state, (utilization, rejected)| match state {
                QuotaState::Known {
                    utilization: u,
                    rejected: r,
                } => QuotaState::Known {
                    utilization: u.max(utilization),
                    rejected: r || rejected,
                },
                QuotaState::Unknown => QuotaState::Known { utilization, rejected },
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3_600_000;

    fn seen(window: &str, utilization: f64, observed_at: u64, resets_at: Option<u64>) -> QuotaObservation {
        QuotaObservation {
            key: QuotaKey {
                account: "me".into(),
                service: "claude-subscription".into(),
                window: window.into(),
            },
            utilization: Some(utilization),
            status: QuotaStatus::Allowed,
            window_seconds: 5 * 3600,
            resets_at,
            observed_at,
        }
    }

    #[test]
    fn the_fullest_current_window_counts() {
        let mut book = QuotaBook::default();
        book.observe(seen("five_hour", 0.4, 0, None));
        book.observe(seen("seven_day", 0.8, 0, None));
        assert_eq!(
            book.state("me", "claude-subscription", HOUR),
            QuotaState::Known {
                utilization: 0.8,
                rejected: false
            }
        );
        assert_eq!(book.state("me", "other", HOUR), QuotaState::Unknown);
    }

    #[test]
    fn a_value_older_than_its_window_or_past_its_reset_is_unknown() {
        let mut book = QuotaBook::default();
        book.observe(seen("five_hour", 0.9, 0, None));
        assert_ne!(book.state("me", "claude-subscription", 5 * HOUR), QuotaState::Unknown);
        assert_eq!(
            book.state("me", "claude-subscription", 5 * HOUR + 1),
            QuotaState::Unknown
        );
        let mut book = QuotaBook::default();
        book.observe(seen("five_hour", 0.9, 0, Some(HOUR)));
        assert_eq!(book.state("me", "claude-subscription", HOUR), QuotaState::Unknown);
    }

    #[test]
    fn an_older_observation_does_not_replace_a_newer_one() {
        let mut book = QuotaBook::default();
        book.observe(seen("five_hour", 0.5, 10, None));
        book.observe(seen("five_hour", 0.1, 5, None));
        assert_eq!(
            book.state("me", "claude-subscription", 20),
            QuotaState::Known {
                utilization: 0.5,
                rejected: false
            }
        );
    }

    #[test]
    fn no_station_starts_at_seventy_percent_or_on_a_refused_window() {
        let limit = 0.7;
        assert!(QuotaState::Unknown.allows_start(limit));
        let known = |utilization, rejected| QuotaState::Known { utilization, rejected };
        assert!(known(0.69, false).allows_start(limit));
        assert!(!known(0.7, false).allows_start(limit));
        assert!(!known(0.1, true).allows_start(limit));
        assert!(known(1.0, false).spent() && known(0.2, true).spent() && !known(0.99, false).spent());
    }
}
