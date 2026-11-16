//! What the factory asks the app to do. Effects are plain data; the app carries them out on its lanes, after it wrote
//! the step's log entries, and reports back with an [`crate::Input`]. Effects that change the repository carry an
//! [`OperationId`] that their result names again (E167).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::budget::Spend;
use crate::event::{Action, TargetState};
use crate::ids::{HunkId, OperationId, OrderId, PromptId, RequestId, SessionKey};
use crate::routing::RankEntry;
use crate::station::Role;
use crate::workflow::WorktreeLocation;

/// A command run in the gate sandbox (E187) under its deadline and output limit (C67).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxRun {
    /// The shell command.
    pub command: String,
    /// Extra environment variables over E186's allow list.
    pub env: BTreeMap<String, String>,
    /// Hosts it may reach; empty for no network.
    pub network: Vec<String>,
    /// Unix sockets it may reach.
    pub sockets: Vec<String>,
    /// When the whole process group is ended.
    pub deadline_ms: u64,
    /// How much output it may print before it is ended.
    pub output_limit: u64,
}

/// What a secrets scan covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "range")]
pub enum ScanRange {
    /// The worktree's uncommitted changes, before the factory commits them.
    Pending,
    /// The diff between two commits.
    Diff {
        /// The base.
        base: String,
        /// The head.
        head: String,
    },
}

/// How the factory answers a request of a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "answer", content = "text")]
pub enum Answer {
    /// The user's text.
    Text(String),
    /// Refused, with the reason the agent is told (E173, E189).
    Refuse(String),
}

/// What a notification is about (E175, I9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notice {
    /// The order waits for the user.
    Waiting,
    /// A station failed.
    Failed,
    /// The evidence is ready for acceptance.
    ReadyForAcceptance,
    /// A budget is spent (C64).
    BudgetSpent,
    /// The order is taken over.
    Done,
}

/// One thing for the app to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "effect")]
pub enum Effect {
    /// Create the order's worktree and branch at `base`, copy `copy` from the main checkout by `clonefile`, and run
    /// the setup command in the gate sandbox (E188). Answered by [`crate::Input::Done`].
    PrepareWorktree {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// The branch to create.
        branch: String,
        /// The commit to start at.
        base: String,
        /// Where.
        location: WorktreeLocation,
        /// What to copy.
        copy: Vec<String>,
        /// The setup command.
        setup: Option<SandboxRun>,
    },
    /// Freeze the effective agent and scanner configuration from the main checkout outside the builder's reach
    /// (E172). Answered by [`crate::Input::Frozen`].
    FreezeConfig {
        /// The order.
        order: OrderId,
        /// The paths of the fixed group, as patterns.
        paths: Vec<String>,
    },
    /// Start or resume a session for a role (E165, E170, E173). Answered by [`crate::Input::SessionStarted`] or
    /// [`crate::Input::SessionFailed`].
    StartSession {
        /// The order.
        order: OrderId,
        /// The role.
        role: Role,
        /// The entry routing chose.
        entry: RankEntry,
        /// What resumes an earlier session of the role (A15).
        resume: Option<String>,
        /// The worktree.
        worktree: String,
        /// The frozen configuration the path must load instead of the worktree's (E172).
        frozen: String,
        /// Whether the role may only read (E173).
        read_only: bool,
        /// Whether subagents and background work stay on; off where the path does not report them (E189).
        subagents: bool,
        /// Hosts the session may reach (E173).
        network: Vec<String>,
        /// Tools switched on for the role (E191).
        tools: Vec<String>,
        /// A ceiling in dollars for paths that take one at their start (C64).
        budget_usd: Option<f64>,
        /// The name of the checkpoint thread (F67).
        checkpoint_thread: String,
    },
    /// Send a prompt; the turn it starts ends with [`crate::SessionEvent::TurnEnded`] for `prompt`.
    SendPrompt {
        /// The order.
        order: OrderId,
        /// The session.
        session: SessionKey,
        /// The prompt's id.
        prompt: PromptId,
        /// The text.
        text: String,
        /// A JSON schema the answer must follow, where the path takes one (C63).
        schema: Option<String>,
    },
    /// Stop the running turn.
    StopTurn {
        /// The order.
        order: OrderId,
        /// The session.
        session: SessionKey,
    },
    /// Stop background work still running after a turn (E189).
    StopBackground {
        /// The order.
        order: OrderId,
        /// The session.
        session: SessionKey,
    },
    /// End a session's process (E189).
    EndSession {
        /// The order.
        order: OrderId,
        /// The session.
        session: SessionKey,
    },
    /// Answer a request of a session.
    AnswerRequest {
        /// The order.
        order: OrderId,
        /// The session.
        session: SessionKey,
        /// The request.
        request: RequestId,
        /// The answer.
        answer: Answer,
    },
    /// Remember the main repository's refs before a turn, for the ref watch (E173).
    SnapshotRefs {
        /// The order.
        order: OrderId,
    },
    /// Compare the main repository's refs with the snapshot. Answered by [`crate::Input::Refs`].
    CompareRefs {
        /// The order.
        order: OrderId,
    },
    /// Scan for secrets with the frozen gitleaks configuration (C52). Answered by [`crate::Input::Scanned`].
    ScanSecrets {
        /// The order.
        order: OrderId,
        /// What to scan.
        range: ScanRange,
    },
    /// Commit the worktree's changes, without hooks (E192), with the operation in a trailer. Answered by
    /// [`crate::Input::Done`].
    Commit {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// The message.
        message: String,
        /// The head it expects.
        parent: String,
    },
    /// List the paths the diff changes with their status. Answered by [`crate::Input::Diff`].
    DiffNames {
        /// The order.
        order: OrderId,
        /// The base.
        base: String,
        /// The head.
        head: String,
    },
    /// Run a gate command in the worktree under the gate sandbox. Answered by [`crate::Input::GateDone`].
    RunGate {
        /// The order.
        order: OrderId,
        /// The gate's index.
        index: usize,
        /// Its name.
        name: String,
        /// The command.
        run: SandboxRun,
        /// A folder of its own outside the worktree for pictures (C69).
        images: Option<String>,
    },
    /// Stop a running gate and wait until the supervisor confirms its end (C67).
    StopGate {
        /// The order.
        order: OrderId,
    },
    /// Collect the reviewer's input: diff and commit messages (E179). Answered by [`crate::Input::ReviewInput`].
    CollectReviewInput {
        /// The order.
        order: OrderId,
        /// The base.
        base: String,
        /// The head.
        head: String,
    },
    /// Read the manifest of inputs outside the tree: the copied files' hashes and the state of path dependencies
    /// outside the repository (C69). Answered by [`crate::Input::Manifest`].
    ReadManifest {
        /// The order.
        order: OrderId,
        /// The files copied into the worktree.
        copied: Vec<String>,
    },
    /// Reverse the hunks the user rejected at G2 in the worktree (C55). Answered by [`crate::Input::Rejected`].
    RejectHunks {
        /// The order.
        order: OrderId,
        /// The hunks.
        hunks: Vec<HunkId>,
    },
    /// Merge the target branch into the order's branch; on conflict, change nothing. Answered by
    /// [`crate::Input::Done`].
    MergeTarget {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// The target branch.
        target: String,
    },
    /// Tell whether the order's diff against the new base is byte for byte the one against the old (C56). Answered
    /// by [`crate::Input::Compared`].
    CompareDiffs {
        /// The order.
        order: OrderId,
        /// The old base.
        old_base: String,
        /// The head before the merge.
        old_head: String,
        /// The new base.
        new_base: String,
        /// The head after the merge.
        new_head: String,
    },
    /// Take the order over into its target branch (E164), only if the working copy is on it and clean and its head
    /// is `expected`. Answered by [`crate::Input::Done`].
    TakeOver {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// The target branch.
        target: String,
        /// The commit to take over.
        head: String,
        /// The target's expected head.
        expected: String,
    },
    /// Remove the order's checkpoints, before the worktree (C70). Answered by [`crate::Input::Done`].
    RemoveCheckpoints {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// The checkpoint threads.
        threads: Vec<String>,
    },
    /// Remove the worktree, its build folder and the branch. Answered by [`crate::Input::Done`].
    RemoveWorktree {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// The worktree.
        path: String,
        /// The branch.
        branch: String,
        /// Keep the branch, as after a take-over by hand.
        keep_branch: bool,
    },
    /// After a restart: check whether an operation's target state holds (E167). Answered by
    /// [`crate::Input::Verified`].
    Verify {
        /// The order.
        order: OrderId,
        /// The operation.
        op: OperationId,
        /// What it did.
        action: Action,
        /// The state to check.
        target: TargetState,
    },
    /// Write the specification into the order's folder (E178).
    WriteSpec {
        /// The order.
        order: OrderId,
        /// The Markdown.
        markdown: String,
    },
    /// Post a notification (I9); never during self-tests (R12).
    Notify {
        /// The order.
        order: OrderId,
        /// What about.
        notice: Notice,
    },
    /// Hold or release the activity that keeps the Mac awake (E169).
    WakeLock {
        /// Whether to hold it.
        hold: bool,
        /// How many orders run, for the activity's reason.
        running: usize,
    },
    /// Add spending to the daily account across projects, which the app keeps (E168).
    AddToDay {
        /// The billing service.
        service: Option<String>,
        /// What was spent.
        spend: Spend,
    },
    /// Store a confirmation for this project on this Mac (E190).
    Confirm {
        /// The hash.
        hash: String,
    },
}

impl Effect {
    /// The order the effect belongs to; the app skips it when that order's log could not be written.
    pub fn order(&self) -> Option<&OrderId> {
        match self {
            Effect::PrepareWorktree { order, .. }
            | Effect::FreezeConfig { order, .. }
            | Effect::StartSession { order, .. }
            | Effect::SendPrompt { order, .. }
            | Effect::StopTurn { order, .. }
            | Effect::StopBackground { order, .. }
            | Effect::EndSession { order, .. }
            | Effect::AnswerRequest { order, .. }
            | Effect::SnapshotRefs { order }
            | Effect::CompareRefs { order }
            | Effect::ScanSecrets { order, .. }
            | Effect::Commit { order, .. }
            | Effect::DiffNames { order, .. }
            | Effect::RunGate { order, .. }
            | Effect::StopGate { order }
            | Effect::CollectReviewInput { order, .. }
            | Effect::ReadManifest { order, .. }
            | Effect::RejectHunks { order, .. }
            | Effect::MergeTarget { order, .. }
            | Effect::CompareDiffs { order, .. }
            | Effect::TakeOver { order, .. }
            | Effect::RemoveCheckpoints { order, .. }
            | Effect::RemoveWorktree { order, .. }
            | Effect::Verify { order, .. }
            | Effect::WriteSpec { order, .. }
            | Effect::Notify { order, .. } => Some(order),
            Effect::WakeLock { .. } | Effect::AddToDay { .. } | Effect::Confirm { .. } => None,
        }
    }
}
