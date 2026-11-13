//! What the self-test reads from and does with the parts of `source.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// A replay into an empty thread, as the self-test plays it.
pub fn replay(thread: Thread) -> Source {
    Source::Replay(Box::new(Replay::new(
        thread,
        SystemTime::now(),
        Duration::from_millis(40),
    )))
}

/// The program the self-test starts as `claude`; process-wide, since the lookup runs on a worker thread.
pub(super) static TEST_PROGRAM: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Starts every Claude tab with `program` instead of the `claude` on the Mac, as the self-test runs a stand-in.
pub fn set_test_program(program: PathBuf) {
    *TEST_PROGRAM.lock_ignoring_poison() = Some(program);
}

impl Source {
    /// Plays recorded lines into the replay; what changed.
    pub fn feed(&mut self, lines: &str) -> Vec<ide_thread::Change> {
        match self {
            Source::Replay(replay) => replay.feed_all(lines),
            _ => Vec::new(),
        }
    }
}
