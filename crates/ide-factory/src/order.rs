//! One work order's state: nothing but its log's events applied in turn, so the running factory and
//! a restart after a crash arrive at the same order. Only the factory changes it; a view reads it through methods
//! (E98).

use std::collections::BTreeMap;

use serde::Serialize;

mod apply;

use crate::budget::{Metering, Spend};
use crate::criteria::Criterion;
use crate::event::{Action, BaseRef, Event, OrderOptions, Phase, PromptKind, TurnOutcome};
use crate::evidence::{Deviation, Evidence, GateRecord, ManifestEntry, Provenance, StationCost};
use crate::ids::{HunkId, OperationId, OrderId, PromptId, RequestId, SessionKey};
use crate::limits::TurnWatch;
use crate::log::{LOG_VERSION, LogEntry};
use crate::options::RiskChoice;
use crate::relevance::Classification;
use crate::routing::{Agent, RankEntry, Vendor};
use crate::station::{Condition, RiskClass, Role, Stage, Station};
use crate::time::{Millis, Stamp};
use crate::verdict::Round;

/// A role's session in an order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Slot {
    session: SessionKey,
    entry: RankEntry,
    index: usize,
    vendor: Vendor,
    resume: Option<String>,
    alive: bool,
}

impl Slot {
    /// The app's id of the session.
    pub fn session(&self) -> &SessionKey {
        &self.session
    }

    /// The entry of the ranked list it runs.
    pub fn entry(&self) -> &RankEntry {
        &self.entry
    }

    /// The entry's place in the list.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Its vendor.
    pub fn vendor(&self) -> Vendor {
        self.vendor
    }

    /// What resumes it after its process ended.
    pub fn resume(&self) -> Option<&str> {
        self.resume.as_deref()
    }

    /// Whether its process runs.
    pub fn alive(&self) -> bool {
        self.alive
    }
}

/// The running turn.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Turn {
    prompt: PromptId,
    role: Role,
    kind: PromptKind,
    watch: TurnWatch,
    queued: u32,
    ended: Option<TurnOutcome>,
    answer: Option<String>,
    started_wall: u64,
}

impl Turn {
    /// Its prompt.
    pub fn prompt(&self) -> PromptId {
        self.prompt
    }

    /// The role whose turn it is.
    pub fn role(&self) -> Role {
        self.role
    }

    /// What it asks for.
    pub fn kind(&self) -> PromptKind {
        self.kind
    }

    /// What the stall rule watches.
    pub fn watch(&self) -> &TurnWatch {
        &self.watch
    }

    /// Messages of the user waiting in the driver's queue (E184).
    pub fn queued(&self) -> u32 {
        self.queued
    }

    /// The outcome, once the agent reported it while subagents still ran (E189).
    pub fn ended(&self) -> Option<&TurnOutcome> {
        self.ended.as_ref()
    }
}

/// When the order entered a station.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Visit {
    /// The station.
    pub station: Station,
    /// When.
    pub at: Stamp,
}

/// A work order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Order {
    id: OrderId,
    title: String,
    goal: String,
    criteria: Vec<Criterion>,
    options: OrderOptions,
    base: BaseRef,
    stage: Stage,
    created: Stamp,
    updated: Stamp,
    station: Station,
    reached: Station,
    condition: Condition,
    phase: Phase,
    worktree: Option<String>,
    frozen: Option<String>,
    copied: Vec<ManifestEntry>,
    head: Option<(String, String)>,
    sessions: BTreeMap<Role, Slot>,
    turn: Option<Turn>,
    next_prompt: u64,
    next_op: u64,
    turns: BTreeMap<Station, u32>,
    attempts: u32,
    excluded: Vec<usize>,
    retry_due: Option<Millis>,
    rounds: u32,
    review_log: Vec<Round>,
    gates: Vec<GateRecord>,
    classification: Option<Classification>,
    acknowledged: bool,
    evidence: Option<Evidence>,
    accepted: Option<String>,
    rejected: Vec<HunkId>,
    partial: bool,
    kept: bool,
    diff_opened: bool,
    findings: Option<String>,
    comment: Option<String>,
    spec: Option<String>,
    spec_risk: Option<RiskClass>,
    approved: bool,
    builders: Vec<Vendor>,
    deviations: Vec<Deviation>,
    interventions: u32,
    spend: Spend,
    costs: BTreeMap<Station, Spend>,
    role_spend: BTreeMap<Role, Spend>,
    role_runtime: BTreeMap<Role, u64>,
    metering: Metering,
    ops: BTreeMap<OperationId, Action>,
    request: Option<RequestId>,
    waiting_since: Option<Stamp>,
    waited_ms: u64,
    visits: Vec<Visit>,
    provenance: Vec<Provenance>,
    switching: Option<(Role, Agent)>,
    switched: bool,
    by_hand: bool,
    taken_over: bool,
    branch_kept: bool,
    checkpoints_removed: bool,
    g1_skipped: bool,
    agreed: Option<Vec<Criterion>>,
    issue_update: Option<String>,
    publication: Option<crate::publish::Publication>,
    seq: u64,
}

impl Order {
    /// The order a `Created` event makes; `None` for any other event.
    pub(crate) fn create(id: OrderId, at: Stamp, event: &Event) -> Option<Order> {
        let Event::Created {
            title,
            goal,
            criteria,
            options,
            base,
            stage,
        } = event
        else {
            return None;
        };
        Some(Order {
            id,
            title: title.clone(),
            goal: goal.clone(),
            criteria: criteria.clone(),
            options: options.clone(),
            base: base.clone(),
            stage: *stage,
            created: at,
            updated: at,
            station: Station::Intake,
            reached: Station::Intake,
            condition: Condition::Running,
            phase: Phase::Idle,
            worktree: None,
            frozen: None,
            copied: Vec::new(),
            head: None,
            sessions: BTreeMap::new(),
            turn: None,
            next_prompt: 1,
            next_op: 1,
            turns: BTreeMap::new(),
            attempts: 0,
            excluded: Vec::new(),
            retry_due: None,
            rounds: 0,
            review_log: Vec::new(),
            gates: Vec::new(),
            classification: None,
            acknowledged: false,
            evidence: None,
            accepted: None,
            rejected: Vec::new(),
            partial: false,
            kept: false,
            diff_opened: false,
            findings: None,
            comment: None,
            spec: None,
            spec_risk: None,
            approved: false,
            builders: Vec::new(),
            deviations: Vec::new(),
            interventions: 0,
            spend: Spend::default(),
            costs: BTreeMap::new(),
            role_spend: BTreeMap::new(),
            role_runtime: BTreeMap::new(),
            metering: Metering::default(),
            ops: BTreeMap::new(),
            request: None,
            waiting_since: None,
            waited_ms: 0,
            visits: vec![Visit {
                station: Station::Intake,
                at,
            }],
            provenance: Vec::new(),
            switching: None,
            switched: false,
            by_hand: false,
            taken_over: false,
            branch_kept: false,
            checkpoints_removed: false,
            g1_skipped: false,
            agreed: None,
            issue_update: None,
            publication: None,
            seq: 1,
        })
    }

    /// The log entry for `event` at `at`, applied to the order.
    pub(crate) fn record(&mut self, at: Stamp, event: Event) -> LogEntry {
        let entry = LogEntry {
            v: LOG_VERSION,
            order: self.id.clone(),
            seq: self.seq,
            at,
            event,
        };
        self.apply(&entry);
        entry
    }

    /// Notes what the session shows now, for the stall rule.
    pub(crate) fn watch(
        &mut self,
        now: Millis,
        running: bool,
        tool: Option<Millis>,
        subagents: u32,
        background: u32,
        queued: u32,
    ) {
        if let Some(turn) = &mut self.turn {
            turn.watch.saw(now, running, tool, subagents, background);
            turn.queued = queued;
        }
    }

    /// Notes an event of the session at `now`.
    pub(crate) fn touch(&mut self, now: Millis) {
        if let Some(turn) = &mut self.turn {
            turn.watch.touch(now);
        }
    }

    /// Keeps the outcome and answer of a turn whose subagents still run (E189).
    pub(crate) fn hold_outcome(&mut self, outcome: TurnOutcome, answer: Option<String>) {
        if let Some(turn) = &mut self.turn {
            turn.ended = Some(outcome);
            turn.answer = answer;
        }
    }

    /// Takes the held answer of the running turn.
    pub(crate) fn take_answer(&mut self) -> Option<String> {
        self.turn.as_mut().and_then(|t| t.answer.take())
    }

    /// Moves the running turn's clock by `ms` the Mac slept, and a retry's due time with it.
    pub(crate) fn shift(&mut self, ms: u64) {
        if let Some(turn) = &mut self.turn {
            turn.watch.shift(ms);
        }
        self.retry_due = self.retry_due.map(|due| due.after(ms));
    }

    /// Forgets the running turn, whose prompt was never sent.
    pub(crate) fn drop_turn(&mut self) {
        self.turn = None;
    }

    /// Forgets what only a running process knew: the turn, its timers, and which sessions are alive (E167).
    pub(crate) fn forget_processes(&mut self) {
        self.turn = None;
        self.retry_due = None;
        for slot in self.sessions.values_mut() {
            slot.alive = false;
        }
    }

    /// The next prompt id.
    pub(crate) fn next_prompt(&self) -> PromptId {
        PromptId(self.next_prompt)
    }

    /// The next operation id, `<order>-<n>`.
    pub(crate) fn next_op(&self) -> OperationId {
        OperationId(format!("{}-{}", self.id, self.next_op))
    }

    /// The operations begun and not finished, after a restart the ones to verify.
    pub(crate) fn open_ops(&self) -> &BTreeMap<OperationId, Action> {
        &self.ops
    }

    /// The order's id.
    pub fn id(&self) -> &OrderId {
        &self.id
    }

    /// Its title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Its goal.
    pub fn goal(&self) -> &str {
        &self.goal
    }

    /// Its criteria.
    pub fn criteria(&self) -> &[Criterion] {
        &self.criteria
    }

    /// Its options.
    pub fn options(&self) -> &OrderOptions {
        &self.options
    }

    /// Its base branch and commit.
    pub fn base(&self) -> &BaseRef {
        &self.base
    }

    /// The stage it runs in.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Whether it is a scout's proposal of a rule change (B47).
    pub fn rule_proposal(&self) -> bool {
        self.options.rule_proposal
    }

    /// The role that writes its specification: the scout for a rule proposal, else the planner.
    pub fn spec_role(&self) -> Role {
        if self.options.rule_proposal {
            Role::Scout
        } else {
            Role::Planner
        }
    }

    /// When it was created.
    pub fn created(&self) -> Stamp {
        self.created
    }

    /// When it last changed.
    pub fn updated(&self) -> Stamp {
        self.updated
    }

    /// Its station.
    pub fn station(&self) -> Station {
        self.station
    }

    /// The furthest station it has begun.
    pub fn reached(&self) -> Station {
        self.reached
    }

    /// Its condition in the station.
    pub fn condition(&self) -> &Condition {
        &self.condition
    }

    /// What the factory waits for while it runs.
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Its worktree, once prepared.
    pub fn worktree(&self) -> Option<&str> {
        self.worktree.as_deref()
    }

    /// Where its agent configuration is frozen.
    pub fn frozen(&self) -> Option<&str> {
        self.frozen.as_deref()
    }

    /// The files copied into its worktree, with hashes.
    pub fn copied(&self) -> &[ManifestEntry] {
        &self.copied
    }

    /// The head commit of its branch and its tree.
    pub fn head(&self) -> Option<(&str, &str)> {
        self.head.as_ref().map(|(c, t)| (c.as_str(), t.as_str()))
    }

    /// The session of `role`.
    pub fn session(&self, role: Role) -> Option<&Slot> {
        self.sessions.get(&role)
    }

    /// The session that works now or worked last: of the running turn, else of the station's role.
    pub fn current(&self) -> Option<&Slot> {
        let role = self.turn.as_ref().map(Turn::role).unwrap_or(match self.station {
            Station::Spec => self.spec_role(),
            Station::Review => Role::Reviewer,
            _ => Role::Builder,
        });
        self.sessions.get(&role).or_else(|| self.sessions.get(&Role::Builder))
    }

    /// The running turn.
    pub fn turn(&self) -> Option<&Turn> {
        self.turn.as_ref()
    }

    /// The turns taken in `station`.
    pub fn turns_in(&self, station: Station) -> u32 {
        self.turns.get(&station).copied().unwrap_or(0)
    }

    /// The failed attempts of the current station run.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Entries of the ranked list that failed in this station run.
    pub fn excluded(&self) -> &[usize] {
        &self.excluded
    }

    /// When the scheduled retry is due.
    pub fn retry_due(&self) -> Option<Millis> {
        self.retry_due
    }

    /// The review rounds that did not pass.
    pub fn review_rounds(&self) -> u32 {
        self.rounds
    }

    /// The reviewer's rounds.
    pub fn review_log(&self) -> &[Round] {
        &self.review_log
    }

    /// The review row of the current review.
    pub fn gates(&self) -> &[GateRecord] {
        &self.gates
    }

    /// The last classification of the diff.
    pub fn classification(&self) -> Option<&Classification> {
        self.classification.as_ref()
    }

    /// Whether the user let the last classification pass.
    pub fn acknowledged(&self) -> bool {
        self.acknowledged
    }

    /// The evidence, valid or not.
    pub fn evidence(&self) -> Option<&Evidence> {
        self.evidence.as_ref()
    }

    /// The fingerprint G2 accepted.
    pub fn accepted(&self) -> Option<&str> {
        self.accepted.as_deref()
    }

    /// The hunks the last acceptance rejected.
    pub fn rejected(&self) -> Option<&[HunkId]> {
        Some(self.rejected.as_slice())
    }

    /// Whether the acceptance rejected hunks, so it closes once the new state passes (C55).
    pub fn partial(&self) -> bool {
        self.partial
    }

    /// Whether G2 stands after the base moved without changing the diff (C56).
    pub fn kept(&self) -> bool {
        self.kept
    }

    /// Whether the user opened the diff (R70).
    pub fn diff_opened(&self) -> bool {
        self.diff_opened
    }

    /// The findings that go to the builder next.
    pub fn findings(&self) -> Option<&str> {
        self.findings.as_deref()
    }

    /// The user's comment that goes to the builder next.
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    /// The specification.
    pub fn spec(&self) -> Option<&str> {
        self.spec.as_deref()
    }

    /// Whether G1 approved the specification.
    pub fn approved(&self) -> bool {
        self.approved
    }

    /// The expected risk class: the user's choice, else the planner's proposal, else low (E163).
    pub fn expected_risk(&self) -> RiskClass {
        match self.options.risk {
            RiskChoice::Fixed(class) => class,
            RiskChoice::Automatic => self.spec_risk.unwrap_or_default(),
        }
    }

    /// The risk class the gates run for: the measured one, which can only raise the expected one.
    pub fn risk(&self) -> RiskClass {
        let measured = self.classification.as_ref().map(|c| c.measured).unwrap_or_default();
        self.expected_risk().max(measured)
    }

    /// The vendors that built.
    pub fn builders(&self) -> &[Vendor] {
        &self.builders
    }

    /// The deviations so far, in E163's order.
    pub fn deviations(&self) -> &[Deviation] {
        &self.deviations
    }

    /// How often the user wrote into a session.
    pub fn interventions(&self) -> u32 {
        self.interventions
    }

    /// Whether another entry took over a station (E170), which keeps the order out of the first-pass rate (E180).
    pub fn switched(&self) -> bool {
        self.switched
    }

    /// Whether the user worked by hand.
    pub fn by_hand(&self) -> bool {
        self.by_hand
    }

    /// What it spent.
    pub fn spend(&self) -> &Spend {
        &self.spend
    }

    /// What each station spent.
    pub fn costs(&self) -> Vec<StationCost> {
        let mut out = Vec::new();
        for pair in self.visits.windows(2) {
            out.push((pair[0].station, pair[1].at.wall.saturating_sub(pair[0].at.wall)));
        }
        if let Some(last) = self.visits.last() {
            out.push((last.station, self.updated.wall.saturating_sub(last.at.wall)));
        }
        let mut costs: Vec<StationCost> = Vec::new();
        for (station, ms) in out {
            match costs.iter_mut().find(|c| c.station == Some(station)) {
                Some(cost) => cost.runtime_ms += ms,
                None => costs.push(StationCost {
                    station: Some(station),
                    spend: self.costs.get(&station).copied().unwrap_or_default(),
                    runtime_ms: ms,
                }),
            }
        }
        costs
    }

    /// What `role` spent.
    pub fn role_spend(&self, role: Role) -> Spend {
        self.role_spend.get(&role).copied().unwrap_or_default()
    }

    /// How long `role`'s turns ran, in milliseconds.
    pub fn role_runtime(&self, role: Role) -> u64 {
        self.role_runtime.get(&role).copied().unwrap_or(0)
    }

    /// Which units could be counted.
    pub fn metering(&self) -> Metering {
        self.metering
    }

    /// The request a session waits on.
    pub fn request(&self) -> Option<&RequestId> {
        self.request.as_ref()
    }

    /// Since when it waits for the user.
    pub fn waiting_since(&self) -> Option<Stamp> {
        self.waiting_since
    }

    /// How long it waited for the user in total, the running wait up to `now` included.
    pub fn waited_ms(&self, now: u64) -> u64 {
        self.waited_ms + self.waiting_since.map_or(0, |since| now.saturating_sub(since.wall))
    }

    /// When it entered each station.
    pub fn visits(&self) -> &[Visit] {
        &self.visits
    }

    /// Where each turn's work came from.
    pub fn provenance(&self) -> &[Provenance] {
        &self.provenance
    }

    /// Whether G1 was skipped: in stage 1, where the user wrote the specification, or for class low (E185).
    pub fn g1_skipped(&self) -> bool {
        self.g1_skipped
    }

    /// Whether the builder's next prompt asks it to confirm the criteria or object to them: until it gave its word
    /// on the criteria as they stand, or the user overruled its objection (E178).
    pub fn contract_due(&self) -> bool {
        self.agreed.as_deref() != Some(self.criteria.as_slice())
    }

    /// The issue's description as it reads now, while it differs from what the order froze and the user has not
    /// taken it over (E176).
    pub fn issue_update(&self) -> Option<&str> {
        self.issue_update.as_deref()
    }

    /// How many operations are begun and not finished.
    pub fn open_operations(&self) -> usize {
        self.ops.len()
    }

    /// Whether the take-over happened.
    pub fn taken_over(&self) -> bool {
        self.taken_over
    }

    /// Whether the user kept the order's branch instead of taking it over (E177).
    pub fn branch_kept(&self) -> bool {
        self.branch_kept
    }

    /// Where its way to GitHub or GitLab stands, once the user asked for it (E177).
    pub fn publication(&self) -> Option<&crate::publish::Publication> {
        self.publication.as_ref()
    }

    /// Whether a step of publishing runs now: the scan of the outgoing text, or an operation.
    pub fn publishing(&self) -> bool {
        matches!(self.phase, Phase::PublishScan | Phase::Publishing { .. })
    }

    /// Whether the user may publish it now (E177): done with its branch kept, nothing of it on its way, and not
    /// everything out yet.
    pub fn can_publish(&self) -> bool {
        self.station == Station::Done
            && self.condition != Condition::Aborted
            && self.branch_kept
            && !self.publishing()
            && self.ops.is_empty()
            && !self.published()
    }

    /// Whether everything went out: the merge request is there, and the issue's comment if the order came from one.
    pub fn published(&self) -> bool {
        self.publication
            .as_ref()
            .is_some_and(|p| p.change.is_some() && (self.options.issue.is_none() || p.comment.is_some()))
    }

    /// Whether its checkpoints are removed.
    pub fn checkpoints_removed(&self) -> bool {
        self.checkpoints_removed
    }

    /// Whether its log and evidence may go: 90 days after it was done or aborted, wall clock `now` (E167, F77).
    pub fn retention_over(&self, now: u64) -> bool {
        const KEEP_MS: u64 = 90 * 24 * 60 * 60 * 1000;
        self.finished() && now.saturating_sub(self.updated.wall) > KEEP_MS
    }

    /// Whether it is done or aborted.
    pub fn finished(&self) -> bool {
        self.station == Station::Done || self.condition == Condition::Aborted
    }

    /// The checkpoint threads of its sessions (F67).
    pub fn checkpoint_threads(&self) -> Vec<String> {
        self.sessions
            .keys()
            .map(|role| self.id.checkpoint_thread(*role))
            .collect()
    }
}
