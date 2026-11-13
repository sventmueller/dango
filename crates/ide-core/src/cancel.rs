//! A request to stop a long task, such as an installation, a search, a git command or a provider's answer.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A request to stop a long task, shared between the asker and the task; clones share it.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// A request that has not been made yet.
    pub fn new() -> Cancel {
        Cancel::default()
    }

    /// Asks the task to stop as soon as it can.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether stopping was asked for.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_hears_the_request() {
        let cancel = Cancel::new();
        let task = cancel.clone();
        assert!(!task.is_cancelled());
        cancel.cancel();
        assert!(task.is_cancelled());
    }
}
