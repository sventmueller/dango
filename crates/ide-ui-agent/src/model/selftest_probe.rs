use ide_thread::{Change, Thread};

use super::State;

impl State {
    /// Plays recorded lines; what they changed, with the thread as it is then.
    pub(crate) fn feed(&mut self, lines: &str) -> (Vec<Change>, Option<Thread>) {
        self.run.source.feed(lines)
    }
}
