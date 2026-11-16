//! The names that cross the boundary between the factory and the app: orders, side effects, prompts and sessions.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A work order's id, unique within the app's data folder; also the name of its log file and branch.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OrderId(pub String);

impl OrderId {
    /// The order's branch, under its own prefix so it never mixes with worktree threads (E188).
    pub fn branch(&self) -> String {
        format!("order/{}", self.0)
    }

    /// The name of the checkpoint thread of `role`'s session in this order (F67).
    pub fn checkpoint_thread(&self, role: crate::Role) -> String {
        format!("{}-{}", self.0, role.key())
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A side effect's id: `begun` and `finished` carry it, and the target state names it, such as a commit trailer, so a
/// restart can tell whether the effect happened (E167, C66).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OperationId(pub String);

impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The commit trailer that names the operation which made a commit.
pub const OPERATION_TRAILER: &str = "Dango-Operation";

/// One prompt the factory sent; the turn it starts ends with the same id, and an answer with an older id is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PromptId(pub u64);

/// The app's id of a running session (E165), opaque to the factory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionKey(pub String);

/// An id of a request a session waits on, such as a free-text question.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub String);

/// A hunk of the order's diff as the diff view names it, for partial acceptance (C55).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HunkId(pub String);

/// One run of a check the factory asked for: a gate command, a secrets scan, a diff or a comparison of diffs. Its
/// result names it again, so the result of a run the factory gave up, such as one still running from before a pause,
/// is told apart from the result of the run that replaced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct RunId(pub u64);
