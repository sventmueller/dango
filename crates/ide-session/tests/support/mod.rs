//! What the driver's tests share: a registry whose hop to the main thread is a channel the test thread empties, and
//! the signals each subscriber was handed.

#![allow(dead_code, reason = "each test file uses its own part")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_process::Supervisor;
use ide_session::{Batch, Outcome, Registry, SessionId, Signal, SubscriberId, TurnEnd, TurnId};

/// The longest a test waits for anything.
pub(crate) const WAIT: Duration = Duration::from_secs(20);

/// A registry and the test thread as its main thread.
pub(crate) struct Harness {
    pub(crate) registry: Registry,
    woken: Receiver<SessionId>,
    /// What each subscriber was handed, batch by batch.
    pub(crate) batches: HashMap<SubscriberId, Vec<Batch>>,
    /// The subscribers the test takes from as soon as they are told; others wait until the test takes.
    pub(crate) eager: Vec<SubscriberId>,
}

impl Harness {
    pub(crate) fn new() -> Harness {
        let (tx, woken) = channel();
        let tx = Mutex::new(tx);
        let wake = Arc::new(move |id: SessionId| {
            let _ = tx.lock().unwrap().send(id);
        });
        Harness {
            registry: Registry::new(Supervisor::new(), wake),
            woken,
            batches: HashMap::new(),
            eager: Vec::new(),
        }
    }

    /// Subscribes to `session` and takes from the subscriber whenever it is told.
    pub(crate) fn follow(&mut self, session: SessionId) -> SubscriberId {
        let driver = self.registry.get_mut(session).expect("the session");
        let subscriber = driver.subscribe();
        let first = driver.take(subscriber).expect("the first batch");
        self.batches.entry(subscriber).or_default().push(first);
        self.eager.push(subscriber);
        subscriber
    }

    /// Takes what waits for `subscriber` of `session` now.
    pub(crate) fn take(&mut self, session: SessionId, subscriber: SubscriberId) -> Option<Batch> {
        let batch = self.registry.get_mut(session)?.take(subscriber)?;
        self.batches.entry(subscriber).or_default().push(batch.clone());
        Some(batch)
    }

    /// Every signal `subscriber` was handed, in order.
    pub(crate) fn signals(&self, subscriber: SubscriberId) -> Vec<Signal> {
        self.batches
            .get(&subscriber)
            .into_iter()
            .flatten()
            .flat_map(|b| b.signals.clone())
            .collect()
    }

    /// Plays the main thread until `done` holds for the signals of `subscriber`.
    pub(crate) fn until(&mut self, subscriber: SubscriberId, what: &str, done: impl Fn(&[Signal]) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done(&self.signals(subscriber)) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "waited in vain for {what}: {:?}",
                self.signals(subscriber)
            );
            self.step(left.min(Duration::from_millis(50)));
        }
    }

    /// Plays the main thread until `done` holds for the registry.
    pub(crate) fn until_state(&mut self, what: &str, done: impl Fn(&Registry) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done(&self.registry) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "waited in vain for {what}");
            self.step(left.min(Duration::from_millis(50)));
        }
    }

    /// One hop: waits at most `most` for a session to ask, flushes it and takes for the eager subscribers.
    fn step(&mut self, most: Duration) {
        let Ok(session) = self.woken.recv_timeout(most) else {
            return;
        };
        let told = self.registry.flush(session);
        for subscriber in told {
            if self.eager.contains(&subscriber) {
                self.take(session, subscriber);
            }
        }
    }

    /// Waits for the end of `turn` as `subscriber` hears it.
    pub(crate) fn end_of(&mut self, subscriber: SubscriberId, turn: TurnId) -> TurnEnd {
        self.until(subscriber, &format!("the end of {turn}"), |signals| {
            ended(signals, turn).is_some()
        });
        ended(&self.signals(subscriber), turn).unwrap()
    }
}

/// The end of `turn` among `signals`.
pub(crate) fn ended(signals: &[Signal], turn: TurnId) -> Option<TurnEnd> {
    signals.iter().find_map(|s| match s {
        Signal::TurnEnded(end) if end.turn == turn => Some(end.clone()),
        _ => None,
    })
}

/// The request that waits for the user among `signals`, the last one.
pub(crate) fn waiting(signals: &[Signal]) -> Option<String> {
    signals.iter().rev().find_map(|s| match s {
        Signal::Waiting { request, .. } => Some(request.clone()),
        _ => None,
    })
}

/// Whether `outcome` is a failure whose words contain `part`.
pub(crate) fn failed_with(outcome: &Outcome, part: &str) -> bool {
    matches!(outcome, Outcome::Failed(message) if message.contains(part))
}

/// A folder of its own for one test.
pub(crate) fn folder(name: &str) -> ide_core::testing::Scratch {
    ide_core::testing::scratch(&format!("session-{name}"))
}

/// The stand-in program of the Codex and OpenCode servers.
pub(crate) fn stand_in() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_session-stand-in"))
}

/// The environment of a stand-in `which` that logs what it is sent to `log`.
pub(crate) fn stand_in_env(which: &str, log: &Path) -> Vec<(String, String)> {
    let log_variable = if which == "codex" {
        "CODEX_TEST_LOG"
    } else {
        "OPENCODE_TEST_LOG"
    };
    vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("SESSION_STAND_IN".into(), which.into()),
        (log_variable.into(), log.to_string_lossy().into_owned()),
    ]
}

/// The lines a stand-in logged.
pub(crate) fn logged(log: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}
