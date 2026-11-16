//! What an order's log records (E167): every change of an order is one [`Event`], and the order's state is nothing
//! but these events applied in turn, so restoring after a restart replays the same events the running factory
//! applied.

use serde::{Deserialize, Serialize};

use crate::budget::{Budget, Spend};
use crate::criteria::Criterion;
use crate::evidence::{Deviation, Evidence, GateRecord, Invalidation};
use crate::ids::{HunkId, OperationId, PromptId, RequestId, SessionKey};
use crate::limits::Overdue;
use crate::options::{OptionChange, RiskChoice};
use crate::relevance::Classification;
use crate::routing::{Agent, RankEntry, Readiness, TieBreak, Vendor};
use crate::station::{Condition, RiskClass, Role, Stage, Station, TakeOverBlocker};
use crate::verdict::Round;
use crate::workflow::WorktreeLocation;

/// The branch and commit an order starts from and is taken over into (F49).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseRef {
    /// The base branch.
    pub branch: String,
    /// Its commit when the order started, or when its target was last merged in.
    pub commit: String,
}

/// The options of an order (E185).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrderOptions {
    /// The expected risk class.
    pub risk: RiskChoice,
    /// The path scope; empty for none.
    pub scope: Vec<String>,
    /// An own budget.
    pub budget: Option<Budget>,
    /// Fixed entries per role.
    pub casting: std::collections::BTreeMap<Role, RankEntry>,
    /// Skip G1.
    pub skip_spec_approval: bool,
    /// The issue the order came from (E176).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<crate::intake::IssueOrigin>,
    /// A scout's proposal of a rule change (B47): the scout writes the specification, which waits at G1 in every
    /// stage.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rule_proposal: bool,
}

/// Why a station or step was skipped, as the log says it (E185, F54).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipWhy {
    /// In stage 1 the user's text is the specification.
    UserWroteSpec,
    /// G1 was skipped for class low, as the project allows (E190).
    SpecApprovalSkipped,
    /// The order has no path scope, so the path scope gate does not run (F54).
    NoPathScope,
    /// No reviewer agent in stage 1.
    NoReviewerInStageOne,
}

/// What a prompt asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    /// The planner's specification.
    Plan,
    /// The builder's first turn.
    Build,
    /// Back to the builder with findings or a comment.
    Rework,
    /// Going on after a pause, a retry or a restart (E167).
    Resume,
    /// Another entry takes over the station, with a summary (E170).
    Handoff,
    /// The reviewer's verdict (E179).
    Review,
}

/// How a turn ended (E165, C60).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome", content = "detail")]
pub enum TurnOutcome {
    /// It succeeded; only this leads to the gate.
    Succeeded,
    /// It failed, with the agent's words.
    Failed(String),
    /// The user cancelled it.
    Cancelled,
    /// The connection to the agent was lost.
    ConnectionLost,
}

/// A side effect with an operation id (C66).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum Action {
    /// Create the worktree and branch, copy files, run the setup command (E188).
    PrepareWorktree {
        /// The branch to create.
        branch: String,
        /// The commit it starts at.
        base: String,
        /// Where the worktree goes.
        location: WorktreeLocation,
    },
    /// Commit the worktree's changes after a turn (E163).
    Commit {
        /// The branch.
        branch: String,
        /// Its head before.
        parent: String,
    },
    /// Merge the target branch into the order's branch (F49).
    MergeTarget {
        /// The order's branch.
        branch: String,
        /// Its head before.
        head: String,
        /// The target branch.
        target: String,
    },
    /// Take the order's branch over into the target branch (E164).
    TakeOver {
        /// The target branch.
        target: String,
        /// The commit taken over.
        head: String,
        /// The target's head the take-over expects.
        expected: String,
    },
    /// Remove the order's checkpoints (C70).
    RemoveCheckpoints {
        /// The checkpoint threads.
        threads: Vec<String>,
    },
    /// Remove the worktree and its build folder.
    RemoveWorktree {
        /// The worktree.
        path: String,
        /// The branch, which is deleted once taken over or aborted.
        branch: String,
    },
}

impl Action {
    /// The state a restart checks before it repeats the action, so nothing happens twice (E167).
    pub fn target(&self, op: &OperationId) -> TargetState {
        match self {
            Action::PrepareWorktree { branch, .. } => TargetState::WorktreeOn { branch: branch.clone() },
            Action::Commit { branch, parent } => TargetState::CommitWithTrailer {
                branch: branch.clone(),
                parent: parent.clone(),
                operation: op.clone(),
            },
            Action::MergeTarget { branch, target, .. } => TargetState::BranchContains {
                branch: branch.clone(),
                other: target.clone(),
            },
            Action::TakeOver { target, head, .. } => TargetState::BranchContains {
                branch: target.clone(),
                other: head.clone(),
            },
            Action::RemoveCheckpoints { threads } => TargetState::CheckpointsGone {
                threads: threads.clone(),
            },
            Action::RemoveWorktree { path, .. } => TargetState::WorktreeGone { path: path.clone() },
        }
    }

    /// The state the action expects before it starts.
    pub fn pre(&self) -> PreState {
        match self {
            Action::PrepareWorktree { base, .. } => PreState::Commit(base.clone()),
            Action::Commit { parent, .. } => PreState::Head(parent.clone()),
            Action::MergeTarget { head, .. } => PreState::Head(head.clone()),
            Action::TakeOver { expected, .. } => PreState::Head(expected.clone()),
            Action::RemoveCheckpoints { .. } | Action::RemoveWorktree { .. } => PreState::Nothing,
        }
    }
}

/// The state an action expects before it starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "pre", content = "commit")]
pub enum PreState {
    /// This commit exists.
    Commit(String),
    /// The branch's head is this commit.
    Head(String),
    /// Nothing in particular.
    Nothing,
}

/// A state the app can check to tell whether an action happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "target")]
pub enum TargetState {
    /// A worktree is checked out on this branch.
    WorktreeOn {
        /// The branch.
        branch: String,
    },
    /// The branch has a commit on `parent` whose trailer names the operation.
    CommitWithTrailer {
        /// The branch.
        branch: String,
        /// The commit's parent.
        parent: String,
        /// The operation in its trailer ([`crate::OPERATION_TRAILER`]).
        operation: OperationId,
    },
    /// The branch contains `other`, a branch or commit.
    BranchContains {
        /// The branch.
        branch: String,
        /// What it contains.
        other: String,
    },
    /// The checkpoint threads are gone.
    CheckpointsGone {
        /// The threads.
        threads: Vec<String>,
    },
    /// The worktree folder is gone.
    WorktreeGone {
        /// The path.
        path: String,
    },
}

/// What an action achieved, as the app reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum OpResult {
    /// The worktree is ready; with the setup command's outcome when there was one.
    Prepared {
        /// The worktree's path.
        path: String,
        /// The tree of the base commit.
        tree: String,
        /// The setup command's outcome.
        setup: Option<crate::gates::CommandOutcome>,
        /// The files copied, with their hashes, for the manifest (C69).
        copied: Vec<crate::evidence::ManifestEntry>,
    },
    /// A commit was made.
    Committed {
        /// The commit.
        commit: String,
        /// Its tree.
        tree: String,
    },
    /// There was nothing to commit.
    NothingToCommit,
    /// The target was merged in, or there was nothing to merge.
    Merged {
        /// The order's head after.
        head: String,
        /// Its tree.
        tree: String,
        /// The target's commit merged in, the order's new base.
        base: String,
        /// Whether the target had moved, so a merge commit was made.
        moved: bool,
    },
    /// Merging the target conflicted; nothing was changed.
    Conflict {
        /// The conflicting paths.
        paths: Vec<String>,
    },
    /// The order is in the target branch.
    TakenOver {
        /// The target's head after.
        commit: String,
    },
    /// The take-over cannot go ahead now.
    Blocked {
        /// Why.
        blocker: TakeOverBlocker,
    },
    /// The target branch moved since the merge; it is merged in again.
    TargetMoved,
    /// Removed.
    Removed,
    /// It failed, with the app's words.
    Failed {
        /// What went wrong.
        detail: String,
    },
}

/// One change of an order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum Event {
    /// The order was created.
    Created {
        /// Title.
        title: String,
        /// Goal.
        goal: String,
        /// Criteria.
        criteria: Vec<Criterion>,
        /// Options.
        options: OrderOptions,
        /// Where it starts.
        base: BaseRef,
        /// The stage the app ran in.
        stage: Stage,
    },
    /// An option changed (E185).
    OptionChanged {
        /// The change.
        change: OptionChange,
    },
    /// The criteria changed (E184).
    CriteriaChanged {
        /// The new list.
        criteria: Vec<Criterion>,
    },
    /// The order entered a station.
    Entered {
        /// The station.
        station: Station,
    },
    /// A station or step was skipped.
    Skipped {
        /// The station.
        station: Station,
        /// Why.
        why: SkipWhy,
    },
    /// The condition changed.
    Condition {
        /// The new condition.
        condition: Condition,
    },
    /// The factory now waits for this.
    Phase {
        /// The phase.
        phase: Phase,
    },
    /// A side effect begins.
    Begun {
        /// Its operation id.
        op: OperationId,
        /// What it does.
        action: Action,
        /// The state it expects before.
        pre: PreState,
        /// The state that shows it happened.
        target: TargetState,
    },
    /// A side effect finished.
    Finished {
        /// Its operation id.
        op: OperationId,
        /// What it achieved.
        result: OpResult,
    },
    /// A side effect did not happen: its log entry failed, or a restart found its target state missing.
    Dropped {
        /// Its operation id.
        op: OperationId,
    },
    /// The agent configuration was frozen outside the builder's reach (E172).
    Frozen {
        /// Where it lies.
        dir: String,
    },
    /// A session started for a role (E170).
    SessionStarted {
        /// The role.
        role: Role,
        /// The entry of the ranked list.
        entry: RankEntry,
        /// Its place in the list.
        index: usize,
        /// Its vendor.
        vendor: Vendor,
        /// The app's id of the session.
        session: SessionKey,
        /// What resumes it after its process ended (A15).
        resume: Option<String>,
        /// Its quota was unknown when routing chose it.
        quota_unknown: bool,
        /// The reviewer is of a builder's vendor.
        same_vendor: bool,
        /// The entry routing's tie-break passed over for this one (E170).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tie_break: Option<TieBreak>,
    },
    /// A session could not start; routing passes to the next entry (E170).
    SessionFailed {
        /// The role.
        role: Role,
        /// The entry's place.
        index: usize,
        /// Why.
        readiness: Readiness,
    },
    /// A role's session ended.
    SessionEnded {
        /// The role.
        role: Role,
    },
    /// A prompt was sent.
    Prompted {
        /// Its id.
        prompt: PromptId,
        /// The role.
        role: Role,
        /// What it asks for.
        kind: PromptKind,
    },
    /// A turn ended (E189).
    TurnEnded {
        /// Its prompt.
        prompt: PromptId,
        /// How.
        outcome: TurnOutcome,
    },
    /// The factory stopped a turn (E168).
    TurnStopped {
        /// Its prompt.
        prompt: PromptId,
        /// Why, when a limit stopped it; `None` when the user or a pause did.
        why: Option<Overdue>,
    },
    /// Background work still running after the turn was stopped before the gate (E189).
    LeftoversStopped {
        /// How many entries.
        background: u32,
    },
    /// A retry is scheduled (E168).
    Retry {
        /// The attempt it will be, counted from one.
        attempt: u32,
        /// Whether the failure counted as an attempt; not within the grace after waking.
        counted: bool,
        /// The wait.
        delay_ms: u64,
    },
    /// A role's session used this much since the last report.
    Spent {
        /// The role.
        role: Role,
        /// The billing service.
        service: Option<String>,
        /// What it spent.
        spend: Spend,
        /// The path had no reliable figure.
        unmetered: bool,
    },
    /// The diff was classified (E172).
    Classified {
        /// What changed that matters.
        classification: Classification,
    },
    /// The user let a classification, a ref change or a late G1 pass.
    Acknowledged,
    /// A step of the review row ran.
    Gate {
        /// Its record.
        record: GateRecord,
    },
    /// A review round ended (E179).
    Reviewed {
        /// The round.
        round: Round,
    },
    /// The review row failed and the order goes back to the builder with this.
    Findings {
        /// What the builder gets.
        text: String,
        /// Whether it counts as a review round.
        round: bool,
    },
    /// The evidence was sealed.
    Sealed {
        /// The evidence.
        evidence: Box<Evidence>,
    },
    /// The evidence became invalid.
    Invalidated {
        /// Why.
        why: Invalidation,
    },
    /// A deviation for the evidence.
    Deviated {
        /// The deviation.
        deviation: Deviation,
    },
    /// The planner's specification (E178).
    Specified {
        /// Its Markdown.
        markdown: String,
        /// The risk class it proposes.
        risk: Option<RiskClass>,
    },
    /// G1: the user approved the specification.
    Approved,
    /// The user commented the specification or sent the order back to the builder.
    Commented {
        /// The role it goes to.
        role: Role,
        /// The comment.
        text: String,
    },
    /// The user opened the diff, which G2 needs at class high (R70).
    DiffOpened,
    /// G2: the user accepted, rejecting these hunks (C55).
    Accepted {
        /// The fingerprint accepted.
        fingerprint: String,
        /// The hunks rejected.
        rejected: Vec<HunkId>,
    },
    /// G2 stands after the base moved without changing the diff (C56).
    AcceptanceKept,
    /// The user keeps the order's branch instead of taking it over (E177).
    BranchKept,
    /// The user took the station over by hand (E184).
    ByHand,
    /// The user handed the station back to the Leitstand (E184).
    BackToLeitstand,
    /// The user wrote into a session (E184).
    Intervened {
        /// The role.
        role: Role,
    },
    /// The user switched a session's model (E184).
    ModelSwitched {
        /// The role.
        role: Role,
        /// The model.
        model: String,
        /// Its vendor.
        vendor: Vendor,
    },
    /// Another entry took over a station (E170).
    AgentSwitched {
        /// The role.
        role: Role,
        /// The agent before.
        from: Agent,
        /// The entry before, which this station run does not use again.
        index: usize,
        /// Whether its quota ran out, as opposed to a failure.
        quota: bool,
    },
    /// A session waits on the user (E189).
    Asked {
        /// The request.
        request: RequestId,
    },
    /// The request was answered.
    Answered {
        /// The request.
        request: RequestId,
    },
    /// The builder confirmed the criteria, objected to them, or said neither, in a turn that asked for its word
    /// (E178).
    CriteriaReplied {
        /// What it said.
        reply: crate::contract::CriteriaReply,
    },
    /// The user overruled the builder's objection: the criteria stand as they are (E178).
    ObjectionOverruled,
    /// The order's issue reads differently from what the order froze; it is taken over only on the user's word
    /// (E176).
    IssueChanged {
        /// The issue's description as it reads now.
        body: String,
    },
    /// The user took the issue's change over: the order's origin, and its goal, are the issue's new text (E176).
    IssueTaken {
        /// The issue as it reads now, frozen with its hash.
        origin: crate::intake::IssueOrigin,
    },
    /// The base was merged in and is now this commit (F49, C56).
    Rebased {
        /// The new base commit.
        base: String,
    },
    /// The worktree gained a commit made by the factory or the merge.
    Head {
        /// The commit.
        commit: String,
        /// Its tree.
        tree: String,
    },
}

/// What the factory waits for while an order runs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "phase")]
pub enum Phase {
    /// Nothing; the next step is decided from the station.
    #[default]
    Idle,
    /// A free slot (E168).
    Slot,
    /// The worktree (E188).
    Preparing {
        /// The operation.
        op: OperationId,
    },
    /// The frozen agent configuration (E172).
    Freezing,
    /// A session of `role`, from the entry at `index`.
    Starting {
        /// The role.
        role: Role,
        /// The entry.
        index: usize,
    },
    /// A turn.
    Turn {
        /// Its prompt.
        prompt: PromptId,
    },
    /// A retry's wait.
    Retrying,
    /// The secrets scan of a turn's changes before the commit.
    TurnScan,
    /// The commit after a turn.
    Committing {
        /// The operation.
        op: OperationId,
    },
    /// The ref watch after a turn (E173).
    Refs,
    /// The diff's paths.
    Diffing,
    /// The secrets scan of the diff.
    Scanning,
    /// A gate command.
    Gate {
        /// Its index.
        index: usize,
    },
    /// The reviewer's input: diff and commit messages.
    ReviewInput,
    /// Hunks rejected at G2 (C55).
    Rejecting,
    /// The manifest of inputs outside the tree, for sealing or checking the evidence (C69).
    Manifest,
    /// The take-over queue of the target branch (E174).
    Queue,
    /// Merging the target in.
    Merging {
        /// The operation.
        op: OperationId,
    },
    /// Comparing the diff against the old and the new base (C56).
    Comparing,
    /// The take-over.
    TakingOver {
        /// The operation.
        op: OperationId,
    },
    /// Removing checkpoints and the worktree.
    Cleaning {
        /// The operation.
        op: OperationId,
    },
    /// A restart checks whether an operation happened.
    Verifying {
        /// The operation.
        op: OperationId,
    },
}
