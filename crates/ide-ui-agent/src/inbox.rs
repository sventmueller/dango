//! Coalesced invalidations between the connection and the UI. Control events retain their order.

use std::collections::HashSet;

use ide_thread::Change;

use super::tab::Incoming;

/// Above this many distinct invalidations a fresh view is cheaper than retaining their identities.
const MAX_CHANGES: usize = 4096;

#[derive(Default)]
pub(super) struct Pending {
    pub(crate) events: Vec<Incoming>,
    pub(crate) changes: HashSet<Change>,
}

impl Pending {
    pub(crate) fn push(&mut self, event: Incoming) {
        let changes = match event {
            Incoming::Claude(ide_claude::Event::Changed(changes))
            | Incoming::Acp(ide_acp::Event::Changed(changes))
            | Incoming::Own(ide_agent::Event::Changed(changes))
            | Incoming::Codex(ide_codex::Event::Changed(changes))
            | Incoming::OpenCode(ide_opencode::Event::Changed(changes)) => changes,
            other => {
                self.events.push(other);
                return;
            }
        };
        for change in changes {
            if self.changes.contains(&Change::Cleared) {
                break;
            }
            self.changes.insert(change);
            if self.changes.len() > MAX_CHANGES || self.changes.contains(&Change::Cleared) {
                self.changes.clear();
                self.changes.insert(Change::Cleared);
            }
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.events.is_empty() && self.changes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_invalidations_stay_bounded_without_losing_control_events() {
        let mut pending = Pending::default();
        for _ in 0..10_000 {
            pending.push(Incoming::Claude(ide_claude::Event::Changed(vec![Change::Usage])));
        }
        assert_eq!(pending.changes.len(), 1);
        for n in 0..MAX_CHANGES + 1 {
            pending.push(Incoming::Claude(ide_claude::Event::Changed(vec![Change::Subagent(
                n.to_string(),
            )])));
        }
        pending.push(Incoming::Claude(ide_claude::Event::Capabilities));
        assert_eq!(pending.changes, HashSet::from([Change::Cleared]));
        assert!(matches!(
            pending.events.as_slice(),
            [Incoming::Claude(ide_claude::Event::Capabilities)]
        ));
    }
}
