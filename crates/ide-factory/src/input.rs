//! What the app hands the factory: user commands, session events mapped into the factory's own types, results of
//! effects, and the clock. Every input comes with a [`Stamp`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::budget::{DailyAccount, UsageReport};
use crate::event::{BaseRef, OpResult, TurnOutcome};
use crate::gates::{CommandOutcome, SecretScan};
use crate::ids::{HunkId, OperationId, OrderId, PromptId, RequestId, RunId, SessionKey};
use crate::log::LogFailure;
use crate::options::{NewOrder, OptionChange};
use crate::quota::QuotaObservation;
use crate::relevance::Change;
use crate::routing::{Agent, AgentStatus, Readiness};
use crate::station::Role;
use crate::time::{Millis, Stamp};
use crate::workflow::rules::Rules;

/// An input with the time it happened.
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    /// When.
    pub at: Stamp,
    /// What.
    pub kind: InputKind,
}

impl Input {
    /// `kind` at `at`.
    pub fn new(at: Stamp, kind: InputKind) -> Input {
        Input { at, kind }
    }
}

/// What a session asks of the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    /// A question in free text: the order waits for the user (E189).
    Question,
    /// A permission outside the order's bounds that the path asks instead of refusing, without O1 (C72).
    OutsideBounds,
    /// A plan request, which the mode "Auftrag" refuses (E189).
    Plan,
}

/// An event of an order's session, as the app maps it from the session driver (E165).
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// Something happened; what the session shows now.
    Activity {
        /// The status is `Running`.
        running: bool,
        /// Since when the oldest running tool call runs.
        tool_since: Option<Millis>,
        /// Subagents that run.
        subagents: u32,
        /// Background entries that run.
        background: u32,
        /// Messages waiting in the driver's queue (E184).
        queued: u32,
    },
    /// The turn of `prompt` ended with its typed outcome (C60), with the agent's last message; `None` for a turn
    /// the user's own message started (E184).
    TurnEnded {
        /// The prompt.
        prompt: Option<PromptId>,
        /// How it ended.
        outcome: TurnOutcome,
        /// The agent's final message, for the planner and the reviewer.
        answer: Option<String>,
    },
    /// The session waits on a request.
    Request {
        /// The request.
        request: RequestId,
        /// What kind.
        kind: RequestKind,
    },
    /// A request was answered elsewhere, such as in the AI tab.
    Answered {
        /// The request.
        request: RequestId,
    },
    /// What the session used so far.
    Usage {
        /// The report.
        report: UsageReport,
    },
    /// The stream had a gap; `reconciled` says whether messages, requests and usage were reconciled (C62).
    Gap {
        /// Whether reconciling worked.
        reconciled: bool,
    },
    /// The session's process ended.
    Exited,
    /// The user wrote into the session (E184).
    Intervened,
    /// The user switched the session's model (E184).
    ModelSwitched {
        /// The model.
        model: String,
    },
}

/// A ref of the main repository that changed during a turn (E173).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefChange {
    /// The ref, such as `refs/heads/main`.
    pub name: String,
    /// Its value before, `None` when it was made.
    pub before: Option<String>,
    /// Its value after, `None` when it was deleted.
    pub after: Option<String>,
    /// The app made the change itself: a commit of the user, another order, a checkpoint.
    pub by_dango: bool,
}

/// What happened.
#[derive(Debug, Clone, PartialEq)]
pub enum InputKind {
    /// New order from the sheet (E185); `id` is the app's, unique in its data folder, `dirty` whether the working
    /// copy has uncommitted changes.
    NewOrder {
        /// The id.
        id: OrderId,
        /// What the user asked for.
        order: NewOrder,
        /// The branch and commit it starts from.
        base: BaseRef,
        /// Whether the working copy had uncommitted changes.
        dirty: bool,
    },
    /// G1: approve the specification.
    Approve(OrderId),
    /// Comment the specification; the comment goes to the planner.
    CommentSpec(OrderId, String),
    /// The user opened the order's diff (R70).
    DiffOpened(OrderId),
    /// G2: accept the evidence with `fingerprint`, rejecting `rejected` hunks (C55).
    Accept {
        /// The order.
        order: OrderId,
        /// The fingerprint the user saw.
        fingerprint: String,
        /// The hunks rejected.
        rejected: Vec<HunkId>,
    },
    /// G2 without the take-over: accept the evidence with `fingerprint` and keep the order's branch, removing its
    /// worktree, so the user can bring it in by a merge request of their own while O2 is open (E177).
    KeepBranch {
        /// The order.
        order: OrderId,
        /// The fingerprint the user saw.
        fingerprint: String,
        /// The hunks rejected, as with [`InputKind::Accept`].
        rejected: Vec<HunkId>,
    },
    /// The user's click on "Als Merge Request" (E177): with `fingerprint`, G2 that keeps the branch like
    /// [`InputKind::KeepBranch`] and publishes once the worktree is gone; without, publishing an order that is done
    /// with its branch kept, or trying again where it stopped. Pushes the branch, opens a merge request with `texts`,
    /// and comments the order's issue, after the secrets scan of everything that goes out.
    Publish {
        /// The order.
        order: OrderId,
        /// The fingerprint the user saw, for G2 in the same click.
        fingerprint: Option<String>,
        /// The hunks G2 rejects, as with [`InputKind::Accept`]; only with `fingerprint`.
        rejected: Vec<HunkId>,
        /// The texts in the user's words.
        texts: crate::publish::Outgoing,
    },
    /// Back to the builder with a comment.
    BackToBuilder(OrderId, String),
    /// Change the criteria, one per line (E184).
    ChangeCriteria(OrderId, String),
    /// Change an option (E185).
    ChangeOption(OrderId, OptionChange),
    /// Take the station over by hand (E184).
    ByHand(OrderId),
    /// Hand the station back to the Leitstand (E184).
    BackToLeitstand(OrderId),
    /// Abort the order.
    Abort(OrderId),
    /// Delete the finished order: it leaves the factory, and its log, specification and kept reports go.
    Delete(OrderId),
    /// Resume one order, or all paused ones (E167).
    Resume(Option<OrderId>),
    /// Let a wait pass: a changed agent configuration, a ref change, a late G1, a cancelled turn, a failed station.
    Continue(OrderId),
    /// Answer a request of a session.
    Answer {
        /// The order.
        order: OrderId,
        /// The request.
        request: RequestId,
        /// The user's text.
        text: String,
    },
    /// The user confirmed the gate command set of this hash (E190).
    ConfirmGates(String),
    /// Take the change of the order's issue over, as the user's word (E176).
    TakeIssueChange(OrderId),
    /// The order's issue, read again, has this description; never its comments (E176).
    IssueRead {
        /// The order.
        order: OrderId,
        /// The issue's description as it reads now.
        body: String,
    },
    /// The rule set: user defaults, workflow file and confirmations merged (E190).
    Rules(Box<Rules>),
    /// What the app knows about each path (E58, E170).
    Agents(BTreeMap<Agent, AgentStatus>),
    /// A quota was observed (C65).
    Quota(QuotaObservation),
    /// The daily account across projects (E168).
    Day(DailyAccount),
    /// The figures of the folder's order logs, for routing's tie-break (E170, E180).
    Metrics(Box<crate::metrics::Metrics>),
    /// An event of an order's session.
    Session(OrderId, SessionKey, SessionEvent),
    /// A session started.
    SessionStarted {
        /// The order.
        order: OrderId,
        /// The role.
        role: Role,
        /// The session.
        session: SessionKey,
        /// What resumes it after its process ended.
        resume: Option<String>,
    },
    /// A session could not start: sign-in or start failure (E170).
    SessionFailed {
        /// The order.
        order: OrderId,
        /// The role.
        role: Role,
        /// Why.
        readiness: Readiness,
    },
    /// An operation finished.
    Done {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// What it achieved.
        result: OpResult,
    },
    /// After a restart: whether an operation's target state holds, with what it achieved if so.
    Verified {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// What it achieved, when its target state holds.
        reached: Option<OpResult>,
    },
    /// The agent configuration is frozen at `dir`, or freezing failed.
    Frozen(OrderId, Result<String, String>),
    /// The result of the secrets scan of a run.
    Scanned(OrderId, RunId, SecretScan),
    /// The ref watch's changes.
    Refs(OrderId, Vec<RefChange>),
    /// The diff's paths, listed by a run.
    Diff(OrderId, RunId, Vec<Change>),
    /// A gate command ended.
    GateDone {
        /// The order.
        order: OrderId,
        /// The run.
        run_id: RunId,
        /// Its index.
        index: usize,
        /// What it did.
        outcome: CommandOutcome,
    },
    /// The reviewer's input.
    ReviewInput {
        /// The order.
        order: OrderId,
        /// The diff.
        diff: String,
        /// The commit messages.
        commits: String,
    },
    /// The manifest of inputs outside the tree (C69).
    Manifest(OrderId, crate::evidence::Manifest),
    /// Whether the diff against the new base is byte for byte the old one (C56), as a run compared them.
    Compared(OrderId, RunId, bool),
    /// The step's log could not be written for this order, so none of its effects ran (E167).
    LogFailed(LogFailure),
    /// The Mac goes to sleep (E168).
    Sleep,
    /// The Mac woke up.
    Wake,
    /// The Mac runs on battery, or not (E169).
    Battery(bool),
    /// The folder lost its last window, or got one again (E166).
    Windows(bool),
    /// The app quits: running orders pause with "app ended" before the supervisor stops their processes (E167).
    Quit,
    /// Time passed.
    Tick,
}
