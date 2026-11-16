//! The limits of E168: when a turn hangs, how long turns, stations and gates may take, how often a station is retried
//! and how long it waits in between, and how many orders run at once.

use serde::{Deserialize, Serialize};

use crate::time::{MINUTE, Millis, SECOND};

/// The limits an order runs under; the defaults are E168's, and a project can only tighten them (E190).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    stall_minutes: u64,
    tool_call_minutes: u64,
    wake_grace_minutes: u64,
    turn_minutes: u64,
    turns_per_station: u32,
    review_rounds: u32,
    gate_minutes: u64,
    setup_minutes: u64,
    gate_output_mib: u64,
    retry_base_seconds: u64,
    retry_cap_seconds: u64,
    attempts: u32,
    window_start_percent: u32,
    parallel_orders: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            stall_minutes: 5,
            tool_call_minutes: 30,
            wake_grace_minutes: 2,
            turn_minutes: 60,
            turns_per_station: 20,
            review_rounds: 3,
            gate_minutes: 60,
            setup_minutes: 60,
            gate_output_mib: 64,
            retry_base_seconds: 10,
            retry_cap_seconds: 5 * 60,
            attempts: 3,
            window_start_percent: 70,
            parallel_orders: 2,
        }
    }
}

/// One limit by its key in the workflow file and settings, for changing and tightening them one at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Limit {
    /// Minutes without an event before a running turn hangs.
    StallMinutes,
    /// Minutes a single tool call may run.
    ToolCallMinutes,
    /// Minutes after waking in which failures do not count as attempts.
    WakeGraceMinutes,
    /// Minutes a turn may take.
    TurnMinutes,
    /// Turns a station may take.
    TurnsPerStation,
    /// Review rounds an order may take.
    ReviewRounds,
    /// Minutes a gate command may take.
    GateMinutes,
    /// Minutes the setup command may take.
    SetupMinutes,
    /// MiB of output a gate or setup command may print.
    GateOutputMib,
    /// Seconds before the first retry.
    RetryBaseSeconds,
    /// Seconds a retry waits at most.
    RetryCapSeconds,
    /// Attempts per station run.
    Attempts,
    /// Window utilisation in percent above which no station starts.
    WindowStartPercent,
    /// Orders that run at once.
    ParallelOrders,
}

impl Limit {
    /// Every limit.
    pub const ALL: [Limit; 14] = [
        Limit::StallMinutes,
        Limit::ToolCallMinutes,
        Limit::WakeGraceMinutes,
        Limit::TurnMinutes,
        Limit::TurnsPerStation,
        Limit::ReviewRounds,
        Limit::GateMinutes,
        Limit::SetupMinutes,
        Limit::GateOutputMib,
        Limit::RetryBaseSeconds,
        Limit::RetryCapSeconds,
        Limit::Attempts,
        Limit::WindowStartPercent,
        Limit::ParallelOrders,
    ];

    /// The key in the workflow file's `[limits]` and in the settings.
    pub fn key(self) -> &'static str {
        match self {
            Limit::StallMinutes => "stall_minutes",
            Limit::ToolCallMinutes => "tool_call_minutes",
            Limit::WakeGraceMinutes => "wake_grace_minutes",
            Limit::TurnMinutes => "turn_minutes",
            Limit::TurnsPerStation => "turns_per_station",
            Limit::ReviewRounds => "review_rounds",
            Limit::GateMinutes => "gate_minutes",
            Limit::SetupMinutes => "setup_minutes",
            Limit::GateOutputMib => "gate_output_mib",
            Limit::RetryBaseSeconds => "retry_base_seconds",
            Limit::RetryCapSeconds => "retry_cap_seconds",
            Limit::Attempts => "attempts",
            Limit::WindowStartPercent => "window_start_percent",
            Limit::ParallelOrders => "parallel_orders",
        }
    }

    /// The limit of `key`.
    pub fn from_key(key: &str) -> Option<Limit> {
        Limit::ALL.into_iter().find(|limit| limit.key() == key)
    }
}

impl Limits {
    /// The value of `limit`.
    pub fn get(&self, limit: Limit) -> u64 {
        match limit {
            Limit::StallMinutes => self.stall_minutes,
            Limit::ToolCallMinutes => self.tool_call_minutes,
            Limit::WakeGraceMinutes => self.wake_grace_minutes,
            Limit::TurnMinutes => self.turn_minutes,
            Limit::TurnsPerStation => u64::from(self.turns_per_station),
            Limit::ReviewRounds => u64::from(self.review_rounds),
            Limit::GateMinutes => self.gate_minutes,
            Limit::SetupMinutes => self.setup_minutes,
            Limit::GateOutputMib => self.gate_output_mib,
            Limit::RetryBaseSeconds => self.retry_base_seconds,
            Limit::RetryCapSeconds => self.retry_cap_seconds,
            Limit::Attempts => u64::from(self.attempts),
            Limit::WindowStartPercent => u64::from(self.window_start_percent),
            Limit::ParallelOrders => u64::from(self.parallel_orders),
        }
    }

    /// These limits with `limit` set to `value`; counts are capped at `u32::MAX`, and nothing goes below one.
    pub fn with(mut self, limit: Limit, value: u64) -> Limits {
        let value = value.max(1);
        let count = u32::try_from(value).unwrap_or(u32::MAX);
        match limit {
            Limit::StallMinutes => self.stall_minutes = value,
            Limit::ToolCallMinutes => self.tool_call_minutes = value,
            Limit::WakeGraceMinutes => self.wake_grace_minutes = value,
            Limit::TurnMinutes => self.turn_minutes = value,
            Limit::TurnsPerStation => self.turns_per_station = count,
            Limit::ReviewRounds => self.review_rounds = count,
            Limit::GateMinutes => self.gate_minutes = value,
            Limit::SetupMinutes => self.setup_minutes = value,
            Limit::GateOutputMib => self.gate_output_mib = value,
            Limit::RetryBaseSeconds => self.retry_base_seconds = value,
            Limit::RetryCapSeconds => self.retry_cap_seconds = value,
            Limit::Attempts => self.attempts = count,
            Limit::WindowStartPercent => self.window_start_percent = count.min(100),
            Limit::ParallelOrders => self.parallel_orders = count,
        }
        self
    }

    /// The stricter of `self` and `other` for every limit (E190): the smaller value, as every limit is a ceiling and
    /// a shorter grace after waking forgives fewer failures.
    pub fn tighten(&self, other: &Limits) -> Limits {
        Limit::ALL.into_iter().fold(self.clone(), |limits, limit| {
            limits.with(limit, self.get(limit).min(other.get(limit)))
        })
    }

    /// The limits of `self` that `other` loosens, for telling the user a project value had no effect.
    pub fn loosened_by(&self, other: &Limits) -> Vec<Limit> {
        Limit::ALL
            .into_iter()
            .filter(|&limit| other.get(limit) > self.get(limit))
            .collect()
    }

    /// Milliseconds without an event after which a running turn hangs.
    pub fn stall_ms(&self) -> u64 {
        self.stall_minutes * MINUTE
    }

    /// Milliseconds a tool call may run.
    pub fn tool_call_ms(&self) -> u64 {
        self.tool_call_minutes * MINUTE
    }

    /// Milliseconds after waking in which failures do not count.
    pub fn wake_grace_ms(&self) -> u64 {
        self.wake_grace_minutes * MINUTE
    }

    /// Milliseconds a turn may take.
    pub fn turn_ms(&self) -> u64 {
        self.turn_minutes * MINUTE
    }

    /// Turns a station may take.
    pub fn turns_per_station(&self) -> u32 {
        self.turns_per_station
    }

    /// Review rounds an order may take.
    pub fn review_rounds(&self) -> u32 {
        self.review_rounds
    }

    /// Milliseconds a gate command may take.
    pub fn gate_ms(&self) -> u64 {
        self.gate_minutes * MINUTE
    }

    /// Milliseconds the setup command may take.
    pub fn setup_ms(&self) -> u64 {
        self.setup_minutes * MINUTE
    }

    /// Bytes a gate or setup command may print.
    pub fn gate_output_bytes(&self) -> u64 {
        self.gate_output_mib * 1024 * 1024
    }

    /// Attempts per station run, the first one included.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// The window utilisation, from 0 to 1, at and above which no station starts (R11).
    pub fn window_start_share(&self) -> f64 {
        f64::from(self.window_start_percent) / 100.0
    }

    /// How many orders run at once from stage 3 on; a folder in stage 1 runs one ([`crate::Factory::parallel_limit`]).
    pub fn parallel_orders(&self) -> u32 {
        self.parallel_orders
    }

    /// The wait before retry `n`, counted from one: 10 s · 2^(n−1), at most the cap (E168).
    pub fn retry_delay_ms(&self, n: u32) -> u64 {
        let base = self.retry_base_seconds * SECOND;
        let cap = self.retry_cap_seconds * SECOND;
        let shift = n.saturating_sub(1).min(32);
        base.saturating_mul(1u64 << shift).min(cap)
    }
}

/// What a running turn's session shows, as far as the stall rule needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnWatch {
    started: Millis,
    last_event: Millis,
    running: bool,
    tool_since: Option<Millis>,
    subagents: u32,
    background: u32,
}

/// Why a turn is stopped by the factory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Overdue {
    /// Running, but no event for the stall time and nothing that could explain the silence.
    Stalled,
    /// A tool call ran longer than its own limit.
    ToolCall,
    /// The turn took longer than a turn may.
    Turn,
}

impl TurnWatch {
    /// The watch of a turn started at `now`.
    pub fn start(now: Millis) -> TurnWatch {
        TurnWatch {
            started: now,
            last_event: now,
            running: true,
            tool_since: None,
            subagents: 0,
            background: 0,
        }
    }

    /// When the turn started.
    pub fn started(&self) -> Millis {
        self.started
    }

    /// Notes an event at `now` and what the session showed with it: whether it runs, since when its oldest running
    /// tool call runs, and how many subagents and background entries run.
    pub fn saw(&mut self, now: Millis, running: bool, tool_since: Option<Millis>, subagents: u32, background: u32) {
        self.last_event = self.last_event.max(now);
        self.running = running;
        self.tool_since = tool_since;
        self.subagents = subagents;
        self.background = background;
    }

    /// Notes an event at `now` that changes nothing else.
    pub fn touch(&mut self, now: Millis) {
        self.last_event = self.last_event.max(now);
    }

    /// Moves every reading `ms` later, for time the Mac slept.
    pub fn shift(&mut self, ms: u64) {
        self.started = self.started.after(ms);
        self.last_event = self.last_event.after(ms);
        self.tool_since = self.tool_since.map(|at| at.after(ms));
    }

    /// Whether subagents run, which keeps a turn from ending (E189).
    pub fn subagents(&self) -> u32 {
        self.subagents
    }

    /// Whether background entries run, which are stopped before the gate (E189).
    pub fn background(&self) -> u32 {
        self.background
    }

    /// Whether the turn is overdue at `now`, and why; the turn limit comes first, then a long tool call, then
    /// silence (F57).
    pub fn overdue(&self, now: Millis, limits: &Limits) -> Option<Overdue> {
        if now.since(self.started) > limits.turn_ms() {
            return Some(Overdue::Turn);
        }
        if let Some(since) = self.tool_since {
            return (now.since(since) > limits.tool_call_ms()).then_some(Overdue::ToolCall);
        }
        let busy = self.subagents > 0 || self.background > 0;
        (self.running && !busy && now.since(self.last_event) > limits.stall_ms()).then_some(Overdue::Stalled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_double_from_ten_seconds_up_to_five_minutes() {
        let limits = Limits::default();
        let waits: Vec<u64> = (1..=7).map(|n| limits.retry_delay_ms(n) / SECOND).collect();
        assert_eq!(waits, [10, 20, 40, 80, 160, 300, 300]);
        assert_eq!(limits.retry_delay_ms(200), 300 * SECOND);
    }

    #[test]
    fn tightening_takes_the_smaller_value_of_every_limit() {
        let user = Limits::default();
        let project = Limits::default()
            .with(Limit::TurnMinutes, 30)
            .with(Limit::Attempts, 9)
            .with(Limit::ParallelOrders, 4);
        let both = user.tighten(&project);
        assert_eq!(both.turn_ms(), 30 * MINUTE);
        assert_eq!(both.attempts(), 3);
        assert_eq!(both.parallel_orders(), 2);
        assert_eq!(user.loosened_by(&project), [Limit::Attempts, Limit::ParallelOrders]);
    }

    #[test]
    fn every_key_names_its_limit() {
        for limit in Limit::ALL {
            assert_eq!(Limit::from_key(limit.key()), Some(limit));
        }
    }

    fn watch() -> TurnWatch {
        TurnWatch::start(Millis(0))
    }

    #[test]
    fn a_silent_running_turn_hangs_after_five_minutes() {
        let limits = Limits::default();
        let w = watch();
        assert_eq!(w.overdue(Millis(5 * MINUTE), &limits), None);
        assert_eq!(w.overdue(Millis(5 * MINUTE + 1), &limits), Some(Overdue::Stalled));
    }

    #[test]
    fn silence_is_no_stall_while_a_subagent_background_entry_or_tool_call_runs() {
        let limits = Limits::default();
        let later = Millis(20 * MINUTE);
        let mut w = watch();
        w.saw(Millis(0), true, None, 1, 0);
        assert_eq!(w.overdue(later, &limits), None);
        w.saw(Millis(0), true, None, 0, 1);
        assert_eq!(w.overdue(later, &limits), None);
        w.saw(Millis(0), true, Some(Millis(0)), 0, 0);
        assert_eq!(w.overdue(later, &limits), None);
        assert_eq!(w.overdue(Millis(30 * MINUTE + 1), &limits), Some(Overdue::ToolCall));
        w.saw(Millis(0), false, None, 0, 0);
        assert_eq!(
            w.overdue(later, &limits),
            None,
            "a turn that is not running does not stall"
        );
    }

    #[test]
    fn a_turn_may_take_an_hour_whatever_runs() {
        let limits = Limits::default();
        let mut w = watch();
        w.saw(Millis(59 * MINUTE), true, None, 2, 0);
        assert_eq!(w.overdue(Millis(60 * MINUTE), &limits), None);
        assert_eq!(w.overdue(Millis(60 * MINUTE + 1), &limits), Some(Overdue::Turn));
    }

    #[test]
    fn sleep_moves_the_readings() {
        let limits = Limits::default();
        let mut w = watch();
        w.shift(10 * MINUTE);
        assert_eq!(w.overdue(Millis(14 * MINUTE), &limits), None);
        assert_eq!(w.started(), Millis(10 * MINUTE));
    }
}
