//! Runs, jobs and steps as both services report them, in one shape.

/// How far a run, job or step got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// Waiting to start, for a runner, a resource or an approval.
    Queued,
    /// Running.
    Running,
    /// Finished and passed.
    Success,
    /// Finished and failed, timed out or could not start.
    Failed,
    /// Stopped by someone.
    Cancelled,
    /// Left out by its conditions.
    Skipped,
    /// Waits for someone to start it by hand.
    Manual,
    /// Finished without a verdict, such as GitHub's `neutral`.
    Neutral,
}

impl State {
    /// Whether it may still change by itself, so its view is worth reading again.
    pub fn is_active(self) -> bool {
        matches!(self, State::Queued | State::Running)
    }

    /// The state a list of states adds up to: failed wins, then running, then queued, then success.
    pub fn summary(states: impl IntoIterator<Item = State>) -> Option<State> {
        let states: Vec<State> = states.into_iter().collect();
        [
            State::Failed,
            State::Running,
            State::Queued,
            State::Success,
            State::Cancelled,
            State::Manual,
        ]
        .into_iter()
        .find(|wanted| states.contains(wanted))
        .or_else(|| states.first().copied())
    }
}

/// A workflow run on GitHub, a pipeline on GitLab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The service's number for it.
    pub id: u64,
    /// The workflow's name, or the pipeline's name or source.
    pub name: String,
    /// How far it got.
    pub state: State,
    /// Its page.
    pub url: String,
    /// The branch or tag it ran for.
    pub branch: Option<String>,
    /// The commit it ran for.
    pub sha: String,
    /// What started it, such as `push`.
    pub event: Option<String>,
    /// The number GitHub shows, such as 12 in `CI #12`; the id on GitLab.
    pub number: u64,
    /// When it was created, in seconds since 1970.
    pub created: Option<i64>,
    /// When it last changed, in seconds since 1970.
    pub updated: Option<i64>,
}

/// One job of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// The service's number for it.
    pub id: u64,
    /// The run it belongs to.
    pub run: u64,
    /// Its name.
    pub name: String,
    /// GitLab's stage; `None` on GitHub.
    pub stage: Option<String>,
    /// How far it got.
    pub state: State,
    /// Its page.
    pub url: String,
    /// When it started, in seconds since 1970.
    pub started: Option<i64>,
    /// When it finished, in seconds since 1970.
    pub finished: Option<i64>,
    /// Its steps; GitHub lists them, GitLab's come from the sections of its log.
    pub steps: Vec<Step>,
}

impl Job {
    /// How long it ran, or has run until `now`.
    pub fn seconds(&self, now: i64) -> Option<i64> {
        let started = self.started?;
        Some((self.finished.unwrap_or(now) - started).max(0))
    }
}

/// One step of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Its position, from 1.
    pub number: u32,
    /// Its name.
    pub name: String,
    /// How far it got.
    pub state: State,
}

/// Seconds since 1970 of an RFC 3339 time such as `2026-11-09T12:34:56.789+01:00`.
pub fn seconds(text: &str) -> Option<i64> {
    let text = text.trim();
    let number = |range: std::ops::Range<usize>| -> Option<i64> { text.get(range)?.parse().ok() };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if text.get(4..5) != Some("-") || !matches!(text.get(10..11), Some("T" | "t" | " ")) {
        return None;
    }
    let rest = text.get(19..)?;
    let zone = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match zone {
        "Z" | "z" | "" => 0,
        _ => {
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let hours: i64 = zone.get(1..3)?.parse().ok()?;
            let minutes: i64 = zone.get(4..6)?.parse().ok()?;
            sign * (hours * 3600 + minutes * 60)
        }
    };
    Some(days_since_epoch(year, month, day)? * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days from 1970-01-01 to the date, in the proleptic Gregorian calendar.
fn days_since_epoch(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let shifted = (month + 9) % 12;
    let of_year = (153 * shifted + 2) / 5 + day - 1;
    let of_era_days = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    Some(era * 146_097 + of_era_days - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_from_both_services_read_as_seconds() {
        assert_eq!(seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(seconds("2026-11-09T12:00:00Z"), Some(1_794_225_600));
        assert_eq!(seconds("2026-11-09T13:00:00.123+01:00"), Some(1_794_225_600));
        assert_eq!(seconds("2024-02-29T00:00:00.000Z"), Some(1_709_164_800));
        assert_eq!(seconds("not a time"), None);
        assert_eq!(seconds("2026-13-01T00:00:00Z"), None);
    }

    #[test]
    fn a_failure_outweighs_everything_and_running_outweighs_success() {
        assert_eq!(
            State::summary([State::Success, State::Running, State::Failed]),
            Some(State::Failed)
        );
        assert_eq!(State::summary([State::Success, State::Queued]), Some(State::Queued));
        assert_eq!(State::summary([State::Skipped]), Some(State::Skipped));
        assert_eq!(State::summary([]), None);
        assert!(State::Queued.is_active() && !State::Failed.is_active());
    }
}
