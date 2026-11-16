//! The orchestrator of one project folder (E160, E166): all its orders, and [`Factory::handle`], which takes one
//! input and returns the step it leads to: log entries to append first, then effects to carry out (E167).

mod advance;
mod commands;
mod issue;
mod publish;
mod restore;
mod review;
mod session;
mod takeover;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub use restore::{RestoreProblem, Restored};

use crate::budget::DailyAccount;
use crate::effect::{Effect, Notice};
use crate::event::{Event, Phase};
use crate::evidence::Manifest;
use crate::ids::{OrderId, RunId};
use crate::input::{Input, InputKind};
use crate::log::{LogEntry, LogFailure};
use crate::metrics::Metrics;
use crate::options::{ChangeRefusal, SheetCheck};
use crate::order::Order;
use crate::quota::QuotaBook;
use crate::relevance::Change;
use crate::routing::{Agent, AgentStatus, Choice};
use crate::station::{Condition, FailReason, PauseReason, Stage, Station, WaitReason};
use crate::time::{Millis, Stamp};
use crate::workflow::WorkflowError;
use crate::workflow::rules::Rules;

/// What one input led to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Step {
    /// Entries to append to their order's log before any effect runs (E167).
    pub log: Vec<LogEntry>,
    /// What to do, in order.
    pub effects: Vec<Effect>,
    /// User commands that were refused, with why.
    pub refusals: Vec<Refusal>,
}

/// A refused user command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    /// The order it was for.
    pub order: Option<OrderId>,
    /// Why.
    pub why: Refused,
}

/// Why a user command was refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "refused")]
pub enum Refused {
    /// No such order.
    UnknownOrder,
    /// An order of that id exists.
    Exists,
    /// The sheet does not let the order start.
    Sheet {
        /// What the sheet says.
        check: SheetCheck,
    },
    /// The command does not fit where the order stands.
    NotNow,
    /// G2 was given for evidence that is not the current, valid one.
    StaleEvidence,
    /// At class high, G2 needs the diff opened first (R70).
    OpenDiffFirst,
    /// The option cannot change now.
    Option {
        /// Why.
        refusal: ChangeRefusal,
    },
    /// The confirmed hash is not the current gate command set's.
    StaleConfirmation,
}

/// What the factory remembers about an order only while the app runs: none of it is needed after a restart, where
/// every step it serves is done again.
#[derive(Debug, Clone, Default)]
struct Scratch {
    /// The diff's paths and the head they belong to.
    diff: Option<(String, Vec<Change>)>,
    /// The reviewer's input.
    review_input: Option<(String, String)>,
    /// The manifest read last.
    manifest: Option<Manifest>,
    /// The target was merged in during this take-over.
    merged: bool,
    /// The head and base before the merge, for comparing the diffs (C56).
    before_merge: Option<(String, String)>,
    /// The choice routing made for the session being started.
    choice: Option<(Choice, bool)>,
    /// The session being started goes on with the role's earlier thread.
    resuming: bool,
    /// The run whose result the order waits for; results of other runs are stale.
    run: Option<RunId>,
    /// The station began and nothing of it has run yet.
    fresh: bool,
    /// The next prompt hands the station over from another entry (E170).
    handoff: bool,
    /// The current session's quota ran out; hand over at the turn's end.
    switch: bool,
    /// When the running gate started, for the guard over its deadline.
    gate_started: Option<Millis>,
    /// A stop of the running gate was asked for.
    gate_stopping: bool,
}

/// An order and what the factory remembers about it, as they were before a step.
#[derive(Debug, Clone)]
struct Kept {
    seq: u64,
    order: Order,
    scratch: Scratch,
}

/// All orders of one project folder.
#[derive(Debug, Clone)]
pub struct Factory {
    stage: Stage,
    rules: Rules,
    errors: Vec<WorkflowError>,
    agents: BTreeMap<Agent, AgentStatus>,
    quotas: QuotaBook,
    day: DailyAccount,
    metrics: Metrics,
    orders: BTreeMap<OrderId, Order>,
    scratch: BTreeMap<OrderId, Scratch>,
    /// Each order as it was before each step whose entries are not known to be on disk yet, by the first entry's
    /// place, to go back to when they cannot be written (E167).
    unlogged: BTreeMap<OrderId, Vec<Kept>>,
    runs: u64,
    asleep: Option<Millis>,
    woke: Option<Millis>,
    battery: bool,
    windows: bool,
    lock: bool,
    now: Stamp,
}

/// The step being built while one input is handled.
pub(crate) struct Cx {
    now: Stamp,
    step: Step,
    /// The orders kept as they were before this step.
    kept: BTreeSet<OrderId>,
}

impl Cx {
    fn new(now: Stamp) -> Cx {
        Cx {
            now,
            step: Step::default(),
            kept: BTreeSet::new(),
        }
    }

    fn effect(&mut self, effect: Effect) {
        self.step.effects.push(effect);
    }

    fn refuse(&mut self, order: Option<&OrderId>, why: Refused) {
        self.step.refusals.push(Refusal {
            order: order.cloned(),
            why,
        });
    }
}

impl Factory {
    /// An empty factory for a folder, in `stage`, under `rules`.
    pub fn new(stage: Stage, rules: Rules) -> Factory {
        Factory {
            stage,
            errors: rules.errors().to_vec(),
            rules,
            agents: BTreeMap::new(),
            quotas: QuotaBook::default(),
            day: DailyAccount::default(),
            metrics: Metrics::default(),
            orders: BTreeMap::new(),
            scratch: BTreeMap::new(),
            unlogged: BTreeMap::new(),
            runs: 0,
            asleep: None,
            woke: None,
            battery: false,
            windows: true,
            lock: false,
            now: Stamp::default(),
        }
    }

    /// The stage new orders run in; each order keeps the stage it was created in.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Lets new orders run in `stage`, such as once paths of two vendors are ready ([`crate::routing::stage_for`]).
    pub fn set_stage(&mut self, stage: Stage) {
        self.stage = stage;
    }

    /// The rule set.
    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// The workflow file's errors; while there are any, no new station starts (E190).
    pub fn errors(&self) -> &[WorkflowError] {
        &self.errors
    }

    /// Every order, by id.
    pub fn orders(&self) -> impl Iterator<Item = &Order> {
        self.orders.values()
    }

    /// One order.
    pub fn order(&self, id: &OrderId) -> Option<&Order> {
        self.orders.get(id)
    }

    /// The quotas observed, for the app to keep in its state (C65).
    pub fn quotas(&self) -> &QuotaBook {
        &self.quotas
    }

    /// The daily account as the factory knows it.
    pub fn day(&self) -> &DailyAccount {
        &self.day
    }

    /// The figures routing's tie-break reads, as the app last handed them in (E180).
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// What the app said about each path.
    pub fn agents(&self) -> &BTreeMap<Agent, AgentStatus> {
        &self.agents
    }

    /// The latest time an input carried.
    pub fn now(&self) -> Stamp {
        self.now
    }

    /// Handles one input.
    pub fn handle(&mut self, input: Input) -> Step {
        self.now = input.at;
        let mut cx = Cx::new(input.at);
        self.dispatch(&mut cx, input.kind);
        self.fill_slots(&mut cx);
        self.update_lock(&mut cx);
        cx.step
    }

    fn dispatch(&mut self, cx: &mut Cx, kind: InputKind) {
        match kind {
            InputKind::Rules(rules) => self.set_rules(cx, *rules),
            InputKind::Agents(agents) => {
                self.agents = agents;
                let ids: Vec<OrderId> = self
                    .orders
                    .values()
                    .filter(|o| {
                        matches!(
                            o.condition(),
                            Condition::Waiting(WaitReason::NoAgent | WaitReason::NoReviewer)
                        )
                    })
                    .filter(|o| self.can_route(o))
                    .map(|o| o.id().clone())
                    .collect();
                for id in ids {
                    self.run(cx, &id);
                    self.advance(cx, &id);
                }
            }
            InputKind::Quota(observation) => self.quota(cx, observation),
            InputKind::Day(day) => self.day = day,
            InputKind::Metrics(metrics) => self.metrics = *metrics,
            InputKind::Sleep => self.sleep(cx),
            InputKind::Wake => self.wake(cx),
            InputKind::Battery(on) => self.battery = on,
            InputKind::Windows(open) => self.windows_changed(cx, open),
            InputKind::Quit => self.quit(cx),
            InputKind::Tick => self.tick(cx),
            InputKind::LogFailed(failure) => self.log_failed(failure),
            other if self.stale(&other) => {}
            other => self.command_or_result(cx, other),
        }
    }

    fn scratch(&mut self, id: &OrderId) -> &mut Scratch {
        self.scratch.entry(id.clone()).or_default()
    }

    /// Records `event` for order `id` and applies it.
    fn emit(&mut self, cx: &mut Cx, id: &OrderId, event: Event) {
        let Some(order) = self.orders.get_mut(id) else { return };
        if cx.kept.insert(id.clone()) {
            let kept = Kept {
                seq: order.seq(),
                order: order.clone(),
                scratch: self.scratch.get(id).cloned().unwrap_or_default(),
            };
            self.unlogged.entry(id.clone()).or_default().push(kept);
        }
        let entry = order.record(cx.now, event);
        cx.step.log.push(entry);
    }

    /// The app wrote the log of order `id` up to the entry before `next` (E167).
    pub fn logged(&mut self, id: &OrderId, next: u64) {
        if let Some(kept) = self.unlogged.get_mut(id) {
            kept.retain(|k| k.seq >= next);
            if kept.is_empty() {
                self.unlogged.remove(id);
            }
        }
    }

    /// A new run for order `id`, whose result is the only one the order takes from now on.
    fn start_run(&mut self, id: &OrderId) -> RunId {
        self.runs += 1;
        let run = RunId(self.runs);
        self.scratch(id).run = Some(run);
        run
    }

    /// Whether `kind` is the result of a run the order no longer waits for.
    fn stale(&self, kind: &InputKind) -> bool {
        let (id, run) = match kind {
            InputKind::Scanned(id, run, _) | InputKind::Diff(id, run, _) | InputKind::Compared(id, run, _) => (id, run),
            InputKind::GateDone { order, run_id, .. } => (order, run_id),
            _ => return false,
        };
        self.scratch.get(id).and_then(|s| s.run) != Some(*run)
    }

    fn set_phase(&mut self, cx: &mut Cx, id: &OrderId, phase: Phase) {
        if self.orders.get(id).is_some_and(|o| *o.phase() != phase) {
            self.emit(cx, id, Event::Phase { phase });
        }
    }

    fn set_condition(&mut self, cx: &mut Cx, id: &OrderId, condition: Condition) {
        if self.orders.get(id).is_some_and(|o| *o.condition() != condition) {
            self.emit(cx, id, Event::Condition { condition });
        }
    }

    fn run(&mut self, cx: &mut Cx, id: &OrderId) {
        self.set_condition(cx, id, Condition::Running);
    }

    /// Lets order `id` wait for the user, with a notification unless the user works on it by hand.
    fn wait(&mut self, cx: &mut Cx, id: &OrderId, reason: WaitReason) {
        let notice = match reason {
            WaitReason::ByHand => None,
            WaitReason::Acceptance => Some(Notice::ReadyForAcceptance),
            _ => Some(Notice::Waiting),
        };
        let condition = Condition::Waiting(reason);
        let new = self.orders.get(id).is_some_and(|o| *o.condition() != condition);
        self.set_phase(cx, id, Phase::Idle);
        self.set_condition(cx, id, condition);
        if let Some(notice) = notice.filter(|_| new) {
            cx.effect(Effect::Notify {
                order: id.clone(),
                notice,
            });
        }
    }

    fn pause(&mut self, cx: &mut Cx, id: &OrderId, reason: PauseReason) {
        let budget = matches!(reason, PauseReason::Budget { .. });
        let condition = Condition::Paused(reason);
        let new = self.orders.get(id).is_some_and(|o| *o.condition() != condition);
        if budget && new {
            cx.effect(Effect::Notify {
                order: id.clone(),
                notice: Notice::BudgetSpent,
            });
        }
        self.set_condition(cx, id, condition);
    }

    fn fail(&mut self, cx: &mut Cx, id: &OrderId, reason: FailReason) {
        self.stop_work(cx, id);
        self.set_phase(cx, id, Phase::Idle);
        self.set_condition(cx, id, Condition::Failed(reason));
        cx.effect(Effect::Notify {
            order: id.clone(),
            notice: Notice::Failed,
        });
    }

    /// Stops the running turn and gate of order `id`, if any.
    fn stop_work(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        if let Some(turn) = order.turn()
            && let Some(slot) = order.session(turn.role())
        {
            cx.effect(Effect::StopTurn {
                order: id.clone(),
                session: slot.session().clone(),
            });
            let prompt = turn.prompt();
            self.emit(cx, id, Event::TurnStopped { prompt, why: None });
        }
        if matches!(self.orders.get(id).map(Order::phase), Some(Phase::Gate { .. })) {
            cx.effect(Effect::StopGate { order: id.clone() });
            self.scratch(id).run = None;
        }
    }

    /// Enters `station`; the next step comes from [`Factory::advance`].
    fn enter(&mut self, cx: &mut Cx, id: &OrderId, station: Station) {
        self.emit(cx, id, Event::Entered { station });
        self.set_phase(cx, id, Phase::Idle);
        let scratch = self.scratch(id);
        scratch.fresh = true;
        scratch.diff = None;
        scratch.review_input = None;
        scratch.merged = false;
        scratch.manifest = None;
    }

    /// Orders that run, for parallelism and the wake lock: past intake, not finished, condition running, and not held
    /// in a slot before the build (E174).
    fn running(&self) -> usize {
        self.orders
            .values()
            .filter(|o| o.station() != Station::Intake && !o.finished() && *o.condition() == Condition::Running)
            .filter(|o| *o.phase() != Phase::Slot)
            .count()
    }

    /// How many orders may run at once: the rule set's number, one while the folder runs in stage 1 (E168).
    pub fn parallel_limit(&self) -> u32 {
        if self.stage == Stage::One {
            1
        } else {
            self.rules.limits().parallel_orders().max(1)
        }
    }

    /// Lets orders out of intake and into the build while slots are free and their path scopes allow (E168, E174),
    /// oldest first.
    fn fill_slots(&mut self, cx: &mut Cx) {
        let mut waiting: Vec<(Stamp, OrderId)> = self
            .orders
            .values()
            .filter(|o| o.station() == Station::Intake || *o.phase() == Phase::Slot)
            .filter(|o| *o.condition() == Condition::Running && !o.finished())
            .map(|o| (o.created(), o.id().clone()))
            .collect();
        waiting.sort_by_key(|(at, id)| (at.wall, at.mono, id.clone()));
        for (_, id) in waiting {
            self.advance(cx, &id);
        }
    }

    /// Holds the wake lock while an order runs, on battery only if the setting says so (E169).
    fn update_lock(&mut self, cx: &mut Cx) {
        let running = self.running();
        let hold = running > 0 && self.asleep.is_none() && (!self.battery || self.rules.awake_on_battery());
        if hold != self.lock {
            self.lock = hold;
            cx.effect(Effect::WakeLock { hold, running });
        }
    }

    /// Whether the wake lock is held.
    pub fn holds_wake_lock(&self) -> bool {
        self.lock
    }

    fn set_rules(&mut self, cx: &mut Cx, rules: Rules) {
        if !rules.errors().is_empty() {
            self.errors = rules.errors().to_vec();
            return;
        }
        let changed = rules.hash() != self.rules.hash();
        self.errors.clear();
        self.rules = rules;
        let ids: Vec<OrderId> = self.orders.keys().cloned().collect();
        for id in ids {
            let Some(order) = self.orders.get(&id) else { continue };
            if order.finished() {
                continue;
            }
            match order.condition().clone() {
                Condition::Paused(PauseReason::InvalidWorkflow) => {
                    self.run(cx, &id);
                    self.advance(cx, &id);
                }
                Condition::Waiting(WaitReason::GateCommandsChanged | WaitReason::NoGatePlan) => {
                    self.run(cx, &id);
                    self.advance(cx, &id);
                }
                _ => {}
            }
            if changed {
                self.rules_changed(cx, &id);
            }
        }
    }

    /// Sends orders whose evidence the new rule set invalidates back to review (E163).
    fn rules_changed(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        let holds = order.evidence().is_some_and(|e| e.invalid().is_none());
        let back = matches!(order.condition(), Condition::Waiting(WaitReason::Acceptance))
            || (order.station() == Station::TakeOver && *order.condition() == Condition::Running);
        if holds
            && matches!(order.station(), Station::Acceptance | Station::TakeOver)
            && !order.taken_over()
            && !order.branch_kept()
        {
            self.emit(
                cx,
                id,
                Event::Invalidated {
                    why: crate::evidence::Invalidation::Rules,
                },
            );
            if back {
                self.run(cx, id);
                self.enter(cx, id, Station::Review);
                self.advance(cx, id);
            }
        }
    }

    /// Lets orders waiting for one of `reasons` try again.
    fn retry_waiting(&mut self, cx: &mut Cx, reasons: impl Fn(&WaitReason) -> bool) {
        let ids: Vec<OrderId> = self
            .orders
            .values()
            .filter(|o| matches!(o.condition(), Condition::Waiting(r) if reasons(r)))
            .map(|o| o.id().clone())
            .collect();
        for id in ids {
            self.run(cx, &id);
            self.advance(cx, &id);
        }
    }

    fn windows_changed(&mut self, cx: &mut Cx, open: bool) {
        self.windows = open;
        let ids: Vec<OrderId> = self.orders.keys().cloned().collect();
        for id in ids {
            let Some(order) = self.orders.get(&id) else { continue };
            match (open, order.condition()) {
                (false, Condition::Running) if !order.finished() => {
                    self.stop_work(cx, &id);
                    self.pause(cx, &id, PauseReason::NoWindow);
                }
                (true, Condition::Paused(PauseReason::NoWindow)) => self.resume(cx, &id),
                _ => {}
            }
        }
    }

    fn quit(&mut self, cx: &mut Cx) {
        let ids: Vec<OrderId> = self.orders.keys().cloned().collect();
        for id in ids {
            let running = self
                .orders
                .get(&id)
                .is_some_and(|o| !o.finished() && *o.condition() == Condition::Running);
            if running {
                self.pause(cx, &id, PauseReason::AppEnded);
            }
        }
    }

    fn sleep(&mut self, cx: &mut Cx) {
        self.asleep = Some(cx.now.mono);
        let ids: Vec<OrderId> = self.orders.keys().cloned().collect();
        for id in ids {
            let running = self
                .orders
                .get(&id)
                .is_some_and(|o| !o.finished() && *o.condition() == Condition::Running);
            if running {
                self.pause(cx, &id, PauseReason::Sleep);
            }
        }
    }

    fn wake(&mut self, cx: &mut Cx) {
        let slept = self.asleep.take().map_or(0, |at| cx.now.mono.since(at));
        self.woke = Some(cx.now.mono);
        let ids: Vec<OrderId> = self.orders.keys().cloned().collect();
        for id in ids {
            let Some(order) = self.orders.get_mut(&id) else {
                continue;
            };
            if *order.condition() == Condition::Paused(PauseReason::Sleep) {
                order.shift(slept);
                self.run(cx, &id);
                if self.orders.get(&id).is_some_and(|o| *o.phase() == Phase::Idle) {
                    self.advance(cx, &id);
                }
            }
        }
    }

    /// Whether a failure now falls within the grace after waking, so it does not count (E168).
    fn in_wake_grace(&self, now: Millis) -> bool {
        self.woke
            .is_some_and(|woke| now.since(woke) <= self.rules.limits().wake_grace_ms())
    }

    /// Takes the order back to what its log says, since its entries from `failure.seq` on are not on disk and none of
    /// their effects ran, and pauses it without an entry that could fail again; what earlier steps began goes on
    /// (E167).
    fn log_failed(&mut self, failure: LogFailure) {
        let LogFailure { order: id, seq } = failure;
        let kept = self.unlogged.remove(&id).unwrap_or_default();
        if let Some(before) = kept.into_iter().find(|k| k.seq == seq) {
            self.orders.insert(id.clone(), before.order);
            self.scratch.insert(id.clone(), before.scratch);
        } else if seq == 0 {
            self.orders.remove(&id);
            self.scratch.remove(&id);
            return;
        }
        let Some(order) = self.orders.get_mut(&id) else { return };
        if !order.finished() {
            order.pause_unlogged(PauseReason::LogUnwritable);
        }
    }

    /// Resumes a paused order where it stopped.
    fn resume(&mut self, cx: &mut Cx, id: &OrderId) {
        self.run(cx, id);
        self.redo(cx, id);
    }
}
