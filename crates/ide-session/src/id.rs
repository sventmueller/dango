//! The ids the app keeps of sessions, their turns and the views that follow them; answers from other threads carry
//! these, never the objects (E97, E143).

/// The app's id of a session in the [`Registry`](crate::Registry), unique while the registry lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub(crate) u64);

impl SessionId {
    /// The number as such.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "s{}", self.0)
    }
}

/// A turn of one session: the prompt that starts it, counted from 1 in the order the prompts were handed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TurnId(pub(crate) u64);

impl TurnId {
    /// The number as such.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for TurnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "turn {}", self.0)
    }
}

/// A view, or the orchestrator, that follows one session's events (C71).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubscriberId(pub(crate) u64);

impl SubscriberId {
    /// The number as such.
    pub fn get(self) -> u64 {
        self.0
    }
}
