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
    /// The workflow's name, or the pipeline's name, else `Pipeline`.
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

/// A GitLab stage with its jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage<'a> {
    /// Its name.
    pub name: &'a str,
    /// Its jobs, in the order they were created.
    pub jobs: Vec<&'a Job>,
}

impl Stage<'_> {
    /// How far its jobs got, together.
    pub fn state(&self) -> State {
        State::summary(self.jobs.iter().map(|j| j.state)).unwrap_or(State::Neutral)
    }
}

/// The stages of `jobs` in the order the pipeline runs them; none if a job has no stage, as on GitHub.
pub fn stages(jobs: &[Job]) -> Vec<Stage<'_>> {
    let mut stages: Vec<Stage<'_>> = Vec::new();
    for job in jobs {
        let Some(name) = job.stage.as_deref() else {
            return Vec::new();
        };
        match stages.iter_mut().find(|s| s.name == name) {
            Some(stage) => stage.jobs.push(job),
            None => stages.push(Stage { name, jobs: vec![job] }),
        }
    }
    for stage in &mut stages {
        stage.jobs.sort_by_key(|j| j.id);
    }
    // GitLab creates a pipeline's jobs stage by stage, so the lowest id tells a stage's place.
    stages.sort_by_key(|s| s.jobs.first().map(|j| j.id));
    stages
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

    fn job(id: u64, stage: Option<&str>, state: State) -> Job {
        Job {
            id,
            run: 1,
            name: format!("job-{id}"),
            stage: stage.map(str::to_string),
            state,
            url: String::new(),
            started: None,
            finished: None,
            steps: Vec::new(),
        }
    }

    #[test]
    fn stages_follow_the_pipeline_even_when_the_service_lists_newest_first() {
        let jobs = [
            job(9, Some("deploy"), State::Queued),
            job(7, Some("test"), State::Failed),
            job(5, Some("test"), State::Success),
            job(3, Some("build"), State::Success),
        ];
        let stages = stages(&jobs);
        let seen: Vec<_> = stages
            .iter()
            .map(|s| (s.name, s.jobs.iter().map(|j| j.id).collect::<Vec<_>>(), s.state()))
            .collect();
        assert_eq!(
            seen,
            [
                ("build", vec![3], State::Success),
                ("test", vec![5, 7], State::Failed),
                ("deploy", vec![9], State::Queued),
            ]
        );
        assert!(super::stages(&[job(1, None, State::Success), job(2, Some("x"), State::Success)]).is_empty());
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
