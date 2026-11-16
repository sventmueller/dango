//! The next step of an order that runs and waits for nothing (E166): from its station and what its log says is done.
//! A station counts as done only when its gate passes; no agent can declare it done.

use super::{Cx, Factory};
use crate::criteria::Criterion;
use crate::effect::{Effect, SandboxRun, ScanRange};
use crate::event::{Action, Event, Phase, PromptKind, SkipWhy};
use crate::gates::scopes_overlap;
use crate::ids::OrderId;
use crate::order::Order;
use crate::relevance::AGENT_CONFIG;
use crate::station::{Condition, FailReason, PauseReason, RiskClass, Role, Stage, Station, WaitReason};

impl Factory {
    /// Takes order `id` one step further, if it runs and waits for nothing.
    pub(super) fn advance(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        let idle = matches!(order.phase(), Phase::Idle | Phase::Slot);
        if *order.condition() != Condition::Running || !idle || order.finished() {
            return;
        }
        let station = order.station();
        let fresh = self.scratch.get(id).is_some_and(|s| s.fresh);
        if fresh && station != Station::Intake && !self.errors.is_empty() {
            self.pause(cx, id, PauseReason::InvalidWorkflow);
            return;
        }
        self.scratch(id).fresh = false;
        match station {
            Station::Intake => self.intake(cx, id),
            Station::Spec => self.spec(cx, id),
            Station::Build => self.build(cx, id),
            Station::Review => self.review_next(cx, id),
            Station::Acceptance => self.acceptance(cx, id),
            Station::TakeOver => self.take_over_next(cx, id),
            Station::Done => {}
        }
    }

    /// Whether `order` may leave intake now: fewer orders run than may run at once (E168, R68).
    fn slot_free(&self) -> bool {
        self.running() < usize::try_from(self.parallel_limit()).unwrap_or(usize::MAX)
    }

    /// The first other order `order` must not build beside: one that builds, is reviewed or is taken over, in any
    /// condition, and whose path scope may touch the same files (E174). Without a scope an order overlaps every other.
    /// An order that waits for acceptance holds no scope: its diff is done, and the take-over queue checks again
    /// against what was taken over before it (B45a).
    pub fn overlapping(&self, order: &Order) -> Option<&Order> {
        let scope = |o: &Order| (!o.options().scope.is_empty()).then(|| o.options().scope.clone());
        let mine = scope(order);
        self.orders.values().find(|other| {
            other.id() != order.id()
                && matches!(other.station(), Station::Build | Station::Review | Station::TakeOver)
                && !other.finished()
                && scopes_overlap(mine.as_deref(), scope(other).as_deref())
        })
    }

    /// Whether `order` may begin to build now: as many orders as may run at once, and none of them in its path scope
    /// (E168, E174). Orders that build already count; `order` itself does not.
    fn build_free(&self, order: &Order) -> bool {
        let limit = usize::try_from(self.parallel_limit()).unwrap_or(usize::MAX);
        let building = self
            .orders
            .values()
            .filter(|o| {
                o.id() != order.id()
                    && o.station() >= Station::Build
                    && !o.finished()
                    && *o.condition() == Condition::Running
            })
            .count();
        building < limit && self.overlapping(order).is_none()
    }

    /// Enters the build, or holds `id` in its slot while it may not build yet (E174).
    fn enter_build(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        if !self.build_free(order) {
            self.set_phase(cx, id, Phase::Slot);
            return;
        }
        self.enter(cx, id, Station::Build);
        self.advance(cx, id);
    }

    fn intake(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        if !self.slot_free() {
            self.set_phase(cx, id, Phase::Slot);
            return;
        }
        if let Some(origin) = order.options().issue.clone().filter(|_| order.stage() == Stage::One) {
            let markdown = crate::intake::spec_of(order.title(), order.goal(), order.criteria(), &origin);
            self.enter(cx, id, Station::Spec);
            self.emit(
                cx,
                id,
                Event::Specified {
                    markdown: markdown.clone(),
                    risk: None,
                },
            );
            cx.effect(Effect::WriteSpec {
                order: id.clone(),
                markdown,
            });
            self.advance(cx, id);
            return;
        }
        if order.stage() == Stage::One && !order.rule_proposal() {
            if !order.g1_skipped() {
                self.emit(
                    cx,
                    id,
                    Event::Skipped {
                        station: Station::Spec,
                        why: SkipWhy::UserWroteSpec,
                    },
                );
            }
            self.enter_build(cx, id);
        } else {
            self.enter(cx, id, Station::Spec);
            self.advance(cx, id);
        }
    }

    /// Whether the worktree and the frozen configuration are there; asks for what is missing. A setup command is part
    /// of the gate set, so it waits for the set's confirmation like the gates (E190).
    fn ready(&mut self, cx: &mut Cx, id: &OrderId) -> bool {
        let Some(order) = self.orders.get(id) else { return false };
        if order.worktree().is_none() && self.rules.setup().is_some() && !self.rules.gates_confirmed() {
            self.wait(cx, id, WaitReason::GateCommandsChanged);
            return false;
        }
        if order.worktree().is_none() {
            let op = order.next_op();
            let action = Action::PrepareWorktree {
                branch: id.branch(),
                base: order.base().commit.clone(),
                location: self.rules.location(),
            };
            let setup = self.rules.setup().map(|command| SandboxRun {
                command: command.to_string(),
                env: self.rules.gate_env().clone(),
                network: self.rules.network().to_vec(),
                sockets: self.rules.sockets().to_vec(),
                deadline_ms: self.rules.limits().setup_ms(),
                output_limit: self.rules.limits().gate_output_bytes(),
            });
            let effect = Effect::PrepareWorktree {
                order: id.clone(),
                op: op.clone(),
                branch: id.branch(),
                base: order.base().commit.clone(),
                location: self.rules.location(),
                copy: self.rules.copy().to_vec(),
                setup,
            };
            self.begin(cx, id, action, Phase::Preparing { op: op.clone() });
            cx.effect(effect);
            return false;
        }
        if order.frozen().is_none() {
            self.set_phase(cx, id, Phase::Freezing);
            cx.effect(Effect::FreezeConfig {
                order: id.clone(),
                paths: AGENT_CONFIG.iter().map(|p| p.to_string()).collect(),
            });
            return false;
        }
        true
    }

    /// Records the start of `action` with its operation id and enters `phase`.
    pub(super) fn begin(&mut self, cx: &mut Cx, id: &OrderId, action: Action, phase: Phase) {
        let Some(op) = self.orders.get(id).map(Order::next_op) else {
            return;
        };
        let (pre, target) = (action.pre(), action.target(&op));
        self.emit(
            cx,
            id,
            Event::Begun {
                op,
                action,
                pre,
                target,
            },
        );
        self.set_phase(cx, id, phase);
    }

    fn spec(&mut self, cx: &mut Cx, id: &OrderId) {
        if !self.ready(cx, id) {
            return;
        }
        let Some(order) = self.orders.get(id) else { return };
        let (rework, role) = (order.findings().is_some(), order.spec_role());
        if order.spec().is_some() && !rework {
            if order.approved() || order.g1_skipped() {
                self.end_session(cx, id, role);
                self.enter_build(cx, id);
            } else {
                self.wait(cx, id, WaitReason::SpecApproval);
            }
            return;
        }
        if let Some(session) = self.ensure_session(cx, id, role) {
            self.prompt(cx, id, role, &session, PromptKind::Plan);
        }
    }

    /// Takes the planner's answer as the specification; G1 follows unless the project and the order skip it.
    pub(super) fn specified(&mut self, cx: &mut Cx, id: &OrderId, answer: &str) {
        let spec = crate::spec::read_spec(answer);
        self.emit(
            cx,
            id,
            Event::Specified {
                markdown: answer.to_string(),
                risk: spec.risk,
            },
        );
        cx.effect(Effect::WriteSpec {
            order: id.clone(),
            markdown: answer.to_string(),
        });
        let Some(order) = self.orders.get(id) else { return };
        let suggested = order.options().issue.is_some();
        if (order.criteria().is_empty() || suggested) && !spec.criteria.is_empty() {
            let criteria = crate::criteria::renumber(order.criteria(), spec.criteria);
            self.emit(cx, id, Event::CriteriaChanged { criteria });
        }
        let Some(order) = self.orders.get(id) else { return };
        if order.options().scope.is_empty() && !spec.scope.is_empty() {
            let change = crate::options::OptionChange::Scope(spec.scope);
            self.emit(cx, id, Event::OptionChanged { change });
        }
        let Some(order) = self.orders.get(id) else { return };
        let skip = order.options().skip_spec_approval
            && order.options().issue.is_none()
            && !order.rule_proposal()
            && self.rules.skip_spec_approval()
            && order.expected_risk() == RiskClass::Low;
        self.set_phase(cx, id, Phase::Idle);
        if skip {
            self.emit(
                cx,
                id,
                Event::Skipped {
                    station: Station::Spec,
                    why: SkipWhy::SpecApprovalSkipped,
                },
            );
        }
        self.advance(cx, id);
    }

    fn build(&mut self, cx: &mut Cx, id: &OrderId) {
        if !self.ready(cx, id) {
            return;
        }
        if !self.rules.has_gates() {
            self.wait(cx, id, WaitReason::NoGatePlan);
            return;
        }
        let Some(order) = self.orders.get(id) else { return };
        if order.visit_turns() >= self.rules.limits().turns_per_station() {
            self.fail(cx, id, FailReason::TurnsSpent);
            return;
        }
        let kind = if self.scratch.get(id).is_some_and(|s| s.handoff) {
            PromptKind::Handoff
        } else if order.findings().is_some() || order.comment().is_some() {
            PromptKind::Rework
        } else if order.provenance().iter().any(|p| p.role == Role::Builder) {
            PromptKind::Resume
        } else {
            PromptKind::Build
        };
        if let Some(session) = self.ensure_session(cx, id, Role::Builder) {
            self.prompt(cx, id, Role::Builder, &session, kind);
        }
    }

    fn acceptance(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        let valid = order.evidence().is_some_and(|e| e.invalid().is_none());
        if !valid {
            self.enter(cx, id, Station::Review);
            self.advance(cx, id);
            return;
        }
        self.wait(cx, id, WaitReason::Acceptance);
    }

    /// After a successful turn of the builder: its word on the criteria where the turn asked for it, then its changes;
    /// an objection lets the order wait for the user instead (E178).
    pub(super) fn built(&mut self, cx: &mut Cx, id: &OrderId, answer: Option<&str>) {
        if self.orders.get(id).is_some_and(Order::contract_due) {
            let reply = crate::contract::read_reply(answer);
            self.emit(cx, id, Event::CriteriaReplied { reply: reply.clone() });
            if let crate::contract::CriteriaReply::Objection(text) = reply {
                self.wait(cx, id, WaitReason::CriteriaObjection { text });
                return;
            }
        }
        self.after_turn(cx, id);
    }

    /// After a successful turn of the builder, or the user's work by hand: scan the changes before committing them
    /// (E163).
    pub(super) fn after_turn(&mut self, cx: &mut Cx, id: &OrderId) {
        self.set_phase(cx, id, Phase::TurnScan);
        cx.effect(Effect::ScanSecrets {
            order: id.clone(),
            range: ScanRange::Pending,
        });
    }

    /// Commits the worktree's changes, the operation named in the commit's trailer (E163, E192).
    pub(super) fn commit(&mut self, cx: &mut Cx, id: &OrderId, message: String) {
        let Some(order) = self.orders.get(id) else { return };
        let parent = order
            .head()
            .map_or_else(|| order.base().commit.clone(), |(c, _)| c.to_string());
        let op = order.next_op();
        let action = Action::Commit {
            branch: id.branch(),
            parent: parent.clone(),
        };
        self.begin(cx, id, action, Phase::Committing { op: op.clone() });
        cx.effect(Effect::Commit {
            order: id.clone(),
            op,
            message,
            parent,
        });
    }

    /// After the commit: the ref watch for turns of the builder (E173), then on to the review.
    pub(super) fn after_commit(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        if order.station() == Station::Build {
            self.set_phase(cx, id, Phase::Refs);
            cx.effect(Effect::CompareRefs { order: id.clone() });
        } else {
            self.leave_build(cx, id);
        }
    }

    /// The builder's work is committed: the evidence of the old state is void, and the review begins.
    pub(super) fn leave_build(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        if order.evidence().is_some_and(|e| e.invalid().is_none()) {
            let why = if order.station() == Station::Acceptance {
                crate::evidence::Invalidation::PartialAcceptance
            } else {
                crate::evidence::Invalidation::Tree
            };
            self.emit(cx, id, Event::Invalidated { why });
        }
        self.end_session(cx, id, Role::Builder);
        self.enter(cx, id, Station::Review);
        self.advance(cx, id);
    }

    /// Picks up order `id` where it stopped, after a pause or a restart: what was in flight is asked for again, an
    /// operation whose result never came is verified first (E167).
    pub(super) fn redo(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        let phase = order.phase().clone();
        let open = |op| order.open_ops().get(op).cloned();
        match phase {
            Phase::Preparing { op }
            | Phase::Committing { op }
            | Phase::Merging { op }
            | Phase::TakingOver { op }
            | Phase::Cleaning { op }
            | Phase::Publishing { op }
            | Phase::Verifying { op } => match open(&op) {
                Some(action) => {
                    let target = action.target(&op);
                    self.set_phase(cx, id, Phase::Verifying { op: op.clone() });
                    cx.effect(Effect::Verify {
                        order: id.clone(),
                        op,
                        action,
                        target,
                    });
                }
                None => {
                    self.set_phase(cx, id, Phase::Idle);
                    self.advance(cx, id);
                }
            },
            Phase::TurnScan => self.after_turn(cx, id),
            Phase::Refs => self.after_commit(cx, id),
            Phase::Freezing
            | Phase::Starting { .. }
            | Phase::Turn { .. }
            | Phase::Retrying
            | Phase::Slot
            | Phase::Idle
            | Phase::Queue => {
                self.set_phase(cx, id, Phase::Idle);
                self.advance(cx, id);
            }
            Phase::Diffing
            | Phase::Scanning
            | Phase::Gate { .. }
            | Phase::ReviewInput
            | Phase::Manifest
            | Phase::Comparing
            | Phase::PublishScan => {
                self.set_phase(cx, id, Phase::Idle);
                self.advance(cx, id);
            }
            Phase::Rejecting => {
                self.set_phase(cx, id, Phase::Idle);
                self.reject_hunks(cx, id);
            }
        }
    }

    /// The criteria text for prompts.
    pub(super) fn criteria_text(criteria: &[Criterion]) -> String {
        crate::prompt::criteria_list(criteria)
    }
}
