//! Where an order stands (E166): its station, the condition it is in there and why, its risk class, and the roles
//! agents take in it (E162).

use serde::{Deserialize, Serialize};

use crate::ids::RequestId;

/// A station of an order, in the order an order passes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Station {
    /// Waiting for a free slot (Eingang).
    Intake,
    /// The planner writes the specification and G1 approves it (Spezifikation).
    Spec,
    /// The builder works in the worktree (Bau).
    Build,
    /// Path scope, secrets, gate commands and the reviewer (Prüfung).
    Review,
    /// G2: the user accepts the evidence (Abnahme).
    Acceptance,
    /// Merging into the base branch and cleaning up (Übernahme).
    TakeOver,
    /// Taken over (Erledigt).
    Done,
}

impl Station {
    /// Every station, in their order.
    pub const ALL: [Station; 7] = [
        Station::Intake,
        Station::Spec,
        Station::Build,
        Station::Review,
        Station::Acceptance,
        Station::TakeOver,
        Station::Done,
    ];
}

/// The condition of an order within its station, with why where there is a why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "condition", content = "why")]
pub enum Condition {
    /// The factory works on it.
    Running,
    /// It waits for the user (red dot, E71).
    Waiting(WaitReason),
    /// It stopped by itself and goes on once the reason is gone or the user resumes it.
    Paused(PauseReason),
    /// The station failed; the order goes on only on the user's word.
    Failed(FailReason),
    /// The user aborted it.
    Aborted,
}

impl Condition {
    /// Whether the order waits for the user.
    pub fn waits(&self) -> bool {
        matches!(self, Condition::Waiting(_))
    }
}

/// Why an order waits for the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum WaitReason {
    /// G1: the specification waits for approval.
    SpecApproval,
    /// G2: the evidence waits for acceptance.
    Acceptance,
    /// The agent asked a question in free text, or a request outside the order's bounds (E189, C72).
    Question {
        /// The request.
        request: RequestId,
    },
    /// The user cancelled the turn (E189).
    CancelledTurn,
    /// The builder objected to the criteria before it built (E178).
    CriteriaObjection {
        /// Its reasons, in its words.
        text: String,
    },
    /// The diff changes agent or scanner configuration (E172).
    AgentConfigChanged {
        /// The changed paths.
        paths: Vec<String>,
    },
    /// The measured risk class exceeds the class G1 was skipped for, or the diff leaves the path scope (E163).
    LateSpecApproval,
    /// Refs of the main repository changed in a way Dango cannot account for (E173).
    RefsChanged {
        /// The refs.
        refs: Vec<String>,
    },
    /// The gate command set changed outside the Leitstand and needs the user's confirmation (E190).
    GateCommandsChanged,
    /// No tool was found and the workflow file names no gate command (E163).
    NoGatePlan,
    /// The reviewer rule requires another vendor and none is ready (E162).
    NoReviewer,
    /// No entry of the role's list is ready (E170).
    NoAgent,
    /// The reviewer could not decide a criterion (E179).
    Unclear,
    /// Three review rounds did not pass (E168, E179).
    ReviewRoundsSpent,
    /// The user works on the station by hand (E184).
    ByHand,
    /// The take-over waits: the working copy is on another branch or not clean (F49).
    TakeOverBlocked {
        /// Why.
        blocker: TakeOverBlocker,
    },
}

/// What keeps a take-over from going ahead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeOverBlocker {
    /// The working copy is on this branch, not the order's base branch.
    OtherBranch(String),
    /// The working copy has uncommitted changes.
    Dirty,
}

/// Why an order paused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum PauseReason {
    /// A budget is spent (E168).
    Budget {
        /// Which one.
        scope: crate::budget::BudgetScope,
    },
    /// No entry of the role has quota left (E170).
    Quota,
    /// The Mac sleeps (E169).
    Sleep,
    /// The app quit or crashed; only the user resumes it, unless the setting says otherwise (E167).
    AppEnded,
    /// The order's log could not be written, so no side effect started (E167).
    LogUnwritable,
    /// The workflow file is invalid, so no new station starts (E190).
    InvalidWorkflow,
    /// The folder has no window open (E166).
    NoWindow,
    /// A session's event stream had a gap that could not be reconciled (C62).
    StreamGap,
}

/// Why a station failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum FailReason {
    /// Every attempt failed (E168).
    AttemptsSpent,
    /// The station used up its turns (E168).
    TurnsSpent,
    /// A side effect failed, with the app's words for it.
    Effect {
        /// Which effect.
        effect: String,
        /// What went wrong.
        detail: String,
    },
}

/// A risk class (E163).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    /// Low.
    #[default]
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
}

impl RiskClass {
    /// The key in the workflow file.
    pub fn key(self) -> &'static str {
        match self {
            RiskClass::Low => "low",
            RiskClass::Medium => "medium",
            RiskClass::High => "high",
        }
    }

    /// The class of `key`.
    pub fn from_key(key: &str) -> Option<RiskClass> {
        [RiskClass::Low, RiskClass::Medium, RiskClass::High]
            .into_iter()
            .find(|class| class.key() == key)
    }
}

/// A role an agent takes in an order (E162).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Writes the specification, read-only.
    Planner,
    /// Writes in the worktree.
    Builder,
    /// Judges against the criteria, read-only.
    Reviewer,
    /// Reads and sums up, cheap or local.
    Scout,
}

impl Role {
    /// Every role.
    pub const ALL: [Role; 4] = [Role::Planner, Role::Builder, Role::Reviewer, Role::Scout];

    /// The key in the workflow file and settings.
    pub fn key(self) -> &'static str {
        match self {
            Role::Planner => "planner",
            Role::Builder => "builder",
            Role::Reviewer => "reviewer",
            Role::Scout => "scout",
        }
    }

    /// The role of `key`.
    pub fn from_key(key: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|role| role.key() == key)
    }

    /// Whether the role only reads (E173).
    pub fn reads_only(self) -> bool {
        self != Role::Builder
    }
}

/// Which stage of phase 13 the app ships, which decides the stations and roles that run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// One builder, the user's text as the specification, no reviewer agent.
    #[default]
    One,
    /// Planner, reviewer agent and routing across vendors.
    Two,
    /// Path scopes, the merge queue and parallel orders.
    Three,
}
