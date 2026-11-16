//! The user's commands (E175, E184, E185) and the routing of effect results to their handlers.

use super::{Cx, Factory, Refused};
use crate::criteria::{changed_criteria, criteria_from_lines};
use crate::effect::{Answer, Effect};
use crate::event::{Event, OrderOptions, Phase};
use crate::evidence::Invalidation;
use crate::ids::{HunkId, OrderId};
use crate::input::InputKind;
use crate::options::{OptionChange, check_change, check_new_order};
use crate::order::Order;
use crate::station::{Condition, RiskClass, Role, Station, WaitReason};

impl Factory {
    pub(super) fn command_or_result(&mut self, cx: &mut Cx, kind: InputKind) {
        match kind {
            InputKind::NewOrder { id, order, base, dirty } => self.new_order(cx, id, order, base, dirty),
            InputKind::Approve(id) => self.approve(cx, &id),
            InputKind::CommentSpec(id, text) => self.comment_spec(cx, &id, text),
            InputKind::DiffOpened(id) => {
                if self.known(cx, &id) {
                    self.emit(cx, &id, Event::DiffOpened);
                }
            }
            InputKind::Accept {
                order,
                fingerprint,
                rejected,
            } => self.accept(cx, &order, fingerprint, rejected),
            InputKind::KeepBranch { order, fingerprint } => self.keep_branch(cx, &order, fingerprint),
            InputKind::BackToBuilder(id, text) => self.back_to_builder(cx, &id, text),
            InputKind::ChangeCriteria(id, text) => self.change_criteria(cx, &id, &text),
            InputKind::ChangeOption(id, change) => self.change_option(cx, &id, change),
            InputKind::ByHand(id) => self.by_hand(cx, &id),
            InputKind::BackToLeitstand(id) => self.back_to_leitstand(cx, &id),
            InputKind::Abort(id) => self.abort(cx, &id),
            InputKind::Resume(id) => self.resume_command(cx, id),
            InputKind::Continue(id) => self.continue_order(cx, &id),
            InputKind::Answer { order, request, text } => self.answer(cx, &order, request, text),
            InputKind::ConfirmGates(hash) => self.confirm_gates(cx, hash),
            InputKind::TakeIssueChange(id) => self.take_issue_change(cx, &id),
            InputKind::IssueRead { order, body } => self.issue_read(cx, &order, body),
            InputKind::Session(id, key, event) => self.session_event(cx, &id, &key, event),
            InputKind::SessionStarted {
                order,
                role,
                session,
                resume,
            } => self.session_started(cx, &order, role, session, resume),
            InputKind::SessionFailed { order, role, readiness } => self.session_failed(cx, &order, role, readiness),
            InputKind::Done { order, op, result } => self.done(cx, &order, op, result),
            InputKind::Verified { order, op, reached } => self.verified(cx, &order, op, reached),
            InputKind::Frozen(id, result) => self.frozen(cx, &id, result),
            InputKind::Scanned(id, scan) => self.scanned(cx, &id, scan),
            InputKind::Refs(id, changes) => self.refs(cx, &id, changes),
            InputKind::Diff(id, changes) => self.diffed(cx, &id, changes),
            InputKind::GateDone { order, index, outcome } => self.gate_done(cx, &order, index, outcome),
            InputKind::ReviewInput { order, diff, commits } => self.review_input(cx, &order, diff, commits),
            InputKind::Manifest(id, manifest) => self.manifest(cx, &id, manifest),
            InputKind::Rejected(id, result) => self.rejected(cx, &id, result),
            InputKind::Compared(id, identical) => self.compared(cx, &id, identical),
            InputKind::Rules(_)
            | InputKind::Agents(_)
            | InputKind::Quota(_)
            | InputKind::Day(_)
            | InputKind::Metrics(_)
            | InputKind::Sleep
            | InputKind::Wake
            | InputKind::Battery(_)
            | InputKind::Windows(_)
            | InputKind::Quit
            | InputKind::Tick
            | InputKind::LogFailed(_) => {}
        }
    }

    /// Whether order `id` exists; refuses the command if not.
    pub(super) fn known(&self, cx: &mut Cx, id: &OrderId) -> bool {
        let known = self.orders.contains_key(id);
        if !known {
            cx.refuse(Some(id), Refused::UnknownOrder);
        }
        known
    }

    /// The order if it exists and `fits` it; refuses the command otherwise.
    pub(super) fn fitting(&self, cx: &mut Cx, id: &OrderId, fits: impl Fn(&Order) -> bool) -> bool {
        if !self.known(cx, id) {
            return false;
        }
        let ok = self.orders.get(id).is_some_and(|o| !o.finished() && fits(o));
        if !ok {
            cx.refuse(Some(id), Refused::NotNow);
        }
        ok
    }

    fn new_order(
        &mut self,
        cx: &mut Cx,
        id: OrderId,
        order: crate::options::NewOrder,
        base: crate::event::BaseRef,
        dirty: bool,
    ) {
        if self.orders.contains_key(&id) {
            cx.refuse(Some(&id), Refused::Exists);
            return;
        }
        let mut rules = self.rules.clone();
        if !self.errors.is_empty() {
            rules =
                crate::workflow::rules::Rules::invalid(&crate::workflow::UserDefaults::default(), self.errors.clone());
        }
        let check = check_new_order(&order, &rules, self.stage, dirty, &self.agents);
        if !check.ok() {
            cx.refuse(Some(&id), Refused::Sheet { check });
            return;
        }
        let event = Event::Created {
            title: order.title.trim().to_string(),
            goal: order.goal.trim().to_string(),
            criteria: criteria_from_lines(&order.criteria),
            options: OrderOptions {
                risk: order.risk,
                scope: order.scope,
                budget: order.budget,
                casting: order.casting,
                skip_spec_approval: order.skip_spec_approval && !order.rule_proposal,
                issue: order.issue,
                rule_proposal: order.rule_proposal,
            },
            base,
            stage: self.stage,
        };
        let Some(mut created) = Order::create(id.clone(), cx.now, &event) else {
            return;
        };
        let entry = crate::log::LogEntry {
            v: crate::log::LOG_VERSION,
            order: id.clone(),
            seq: 0,
            at: cx.now,
            event,
        };
        created.apply(&entry);
        cx.step.log.push(entry);
        self.orders.insert(id.clone(), created);
        self.scratch.insert(id.clone(), super::Scratch::default());
        self.advance(cx, &id);
    }

    fn approve(&mut self, cx: &mut Cx, id: &OrderId) {
        let waits = |o: &Order| {
            matches!(
                o.condition(),
                Condition::Waiting(WaitReason::SpecApproval | WaitReason::LateSpecApproval)
            )
        };
        if !self.fitting(cx, id, waits) {
            return;
        }
        let late = self
            .orders
            .get(id)
            .is_some_and(|o| *o.condition() == Condition::Waiting(WaitReason::LateSpecApproval));
        self.emit(cx, id, if late { Event::Acknowledged } else { Event::Approved });
        self.run(cx, id);
        if late {
            self.review_next(cx, id);
        } else {
            self.advance(cx, id);
        }
    }

    fn comment_spec(&mut self, cx: &mut Cx, id: &OrderId, text: String) {
        if !self.fitting(cx, id, |o| {
            *o.condition() == Condition::Waiting(WaitReason::SpecApproval)
        }) {
            return;
        }
        let role = self.orders.get(id).map_or(Role::Planner, Order::spec_role);
        self.emit(cx, id, Event::Commented { role, text });
        self.run(cx, id);
        self.advance(cx, id);
    }

    /// Whether G2 may be given now for the evidence of `fingerprint`; refuses it if not.
    fn acceptable(&self, cx: &mut Cx, id: &OrderId, fingerprint: &str) -> bool {
        if !self.fitting(cx, id, |o| *o.condition() == Condition::Waiting(WaitReason::Acceptance)) {
            return false;
        }
        let Some(order) = self.orders.get(id) else { return false };
        let current = order
            .evidence()
            .filter(|e| e.invalid().is_none())
            .map(|e| e.fingerprint().to_string());
        if current.as_deref() != Some(fingerprint) {
            cx.refuse(Some(id), Refused::StaleEvidence);
            return false;
        }
        if order.risk() == RiskClass::High && !order.diff_opened() {
            cx.refuse(Some(id), Refused::OpenDiffFirst);
            return false;
        }
        true
    }

    fn accept(&mut self, cx: &mut Cx, id: &OrderId, fingerprint: String, rejected: Vec<HunkId>) {
        if !self.acceptable(cx, id, &fingerprint) {
            return;
        }
        let partial = !rejected.is_empty();
        self.emit(cx, id, Event::Accepted { fingerprint, rejected });
        self.run(cx, id);
        if partial {
            self.reject_hunks(cx, id);
        } else {
            self.enter(cx, id, Station::TakeOver);
            self.advance(cx, id);
        }
    }

    /// G2 with the branch kept: the worktree goes, the branch stays, and the order is done without a take-over.
    fn keep_branch(&mut self, cx: &mut Cx, id: &OrderId, fingerprint: String) {
        if !self.acceptable(cx, id, &fingerprint) {
            return;
        }
        let rejected = Vec::new();
        self.emit(cx, id, Event::Accepted { fingerprint, rejected });
        self.emit(cx, id, Event::BranchKept);
        self.run(cx, id);
        self.enter(cx, id, Station::TakeOver);
        self.advance(cx, id);
    }

    fn back_to_builder(&mut self, cx: &mut Cx, id: &OrderId, text: String) {
        let fits = |o: &Order| o.reached() >= Station::Build && o.station() != Station::TakeOver;
        if !self.fitting(cx, id, fits) {
            return;
        }
        self.stop_work(cx, id);
        self.emit(
            cx,
            id,
            Event::Commented {
                role: Role::Builder,
                text,
            },
        );
        if self
            .orders
            .get(id)
            .and_then(Order::evidence)
            .is_some_and(|e| e.invalid().is_none())
        {
            self.emit(
                cx,
                id,
                Event::Invalidated {
                    why: Invalidation::Tree,
                },
            );
        }
        self.run(cx, id);
        self.enter(cx, id, Station::Build);
        self.advance(cx, id);
    }

    fn change_criteria(&mut self, cx: &mut Cx, id: &OrderId, text: &str) {
        if !self.fitting(cx, id, |o| o.station() != Station::TakeOver) {
            return;
        }
        let Some(order) = self.orders.get(id) else { return };
        let criteria = changed_criteria(order.criteria(), text);
        self.emit(cx, id, Event::CriteriaChanged { criteria });
        let comment = "The acceptance criteria changed; check your work against the new list.".to_string();
        self.rework_criteria(cx, id, comment);
    }

    /// After the criteria changed, an order that has built goes back to the builder with `comment`, its evidence void
    /// (E184).
    pub(super) fn rework_criteria(&mut self, cx: &mut Cx, id: &OrderId, comment: String) {
        if self.orders.get(id).is_none_or(|o| o.reached() < Station::Build) {
            return;
        }
        self.stop_work(cx, id);
        self.emit(
            cx,
            id,
            Event::Invalidated {
                why: Invalidation::Criteria,
            },
        );
        self.emit(
            cx,
            id,
            Event::Commented {
                role: Role::Builder,
                text: comment,
            },
        );
        self.run(cx, id);
        self.enter(cx, id, Station::Build);
        self.advance(cx, id);
    }

    fn change_option(&mut self, cx: &mut Cx, id: &OrderId, change: OptionChange) {
        if !self.known(cx, id) {
            return;
        }
        let Some(order) = self.orders.get(id) else { return };
        if let Err(refusal) = check_change(&change, order.reached(), order.spend()) {
            cx.refuse(Some(id), Refused::Option { refusal });
            return;
        }
        let budget = matches!(change, OptionChange::Budget(_));
        self.emit(cx, id, Event::OptionChanged { change });
        let paused = matches!(
            self.orders.get(id).map(Order::condition),
            Some(Condition::Paused(crate::station::PauseReason::Budget { .. }))
        );
        if budget && paused {
            self.resume(cx, id);
        }
    }

    fn by_hand(&mut self, cx: &mut Cx, id: &OrderId) {
        let fits = |o: &Order| {
            matches!(o.station(), Station::Build | Station::Review)
                && !matches!(o.condition(), Condition::Waiting(WaitReason::ByHand))
        };
        if !self.fitting(cx, id, fits) {
            return;
        }
        self.stop_work(cx, id);
        self.emit(cx, id, Event::ByHand);
        self.wait(cx, id, WaitReason::ByHand);
    }

    /// Hands the station back: mode and sandbox are restored by a new start that resumes the session, the evidence
    /// is void, and the station ends through its gate (E184).
    fn back_to_leitstand(&mut self, cx: &mut Cx, id: &OrderId) {
        if !self.fitting(cx, id, |o| *o.condition() == Condition::Waiting(WaitReason::ByHand)) {
            return;
        }
        self.emit(cx, id, Event::BackToLeitstand);
        for role in Role::ALL {
            self.end_session(cx, id, role);
        }
        if self
            .orders
            .get(id)
            .and_then(Order::evidence)
            .is_some_and(|e| e.invalid().is_none())
        {
            self.emit(
                cx,
                id,
                Event::Invalidated {
                    why: Invalidation::ByHand,
                },
            );
        }
        self.run(cx, id);
        if self.orders.get(id).is_some_and(|o| o.station() != Station::Build) {
            self.enter(cx, id, Station::Build);
        }
        self.after_turn(cx, id);
    }

    fn abort(&mut self, cx: &mut Cx, id: &OrderId) {
        if !self.fitting(cx, id, |_| true) {
            return;
        }
        self.stop_work(cx, id);
        self.set_condition(cx, id, Condition::Aborted);
        self.cleanup_next(cx, id, false);
    }

    fn resume_command(&mut self, cx: &mut Cx, id: Option<OrderId>) {
        let ids: Vec<OrderId> = match id {
            Some(id) => {
                if !self.fitting(cx, &id, |o| matches!(o.condition(), Condition::Paused(_))) {
                    return;
                }
                vec![id]
            }
            None => self
                .orders
                .values()
                .filter(|o| !o.finished() && matches!(o.condition(), Condition::Paused(_)))
                .map(|o| o.id().clone())
                .collect(),
        };
        for id in ids {
            self.resume(cx, &id);
        }
    }

    /// Lets a wait or a failure pass, each where it stopped.
    fn continue_order(&mut self, cx: &mut Cx, id: &OrderId) {
        if !self.known(cx, id) {
            return;
        }
        let Some(order) = self.orders.get(id) else { return };
        let station = order.station();
        match order.condition().clone() {
            Condition::Failed(_) => {
                self.run(cx, id);
                self.enter(cx, id, station);
                self.advance(cx, id);
            }
            Condition::Waiting(reason) => self.continue_wait(cx, id, reason),
            _ => cx.refuse(Some(id), Refused::NotNow),
        }
    }

    fn continue_wait(&mut self, cx: &mut Cx, id: &OrderId, reason: WaitReason) {
        match reason {
            WaitReason::AgentConfigChanged { .. } | WaitReason::LateSpecApproval => {
                self.emit(cx, id, Event::Acknowledged);
                self.run(cx, id);
                self.review_next(cx, id);
            }
            WaitReason::RefsChanged { .. } => {
                self.run(cx, id);
                self.leave_build(cx, id);
            }
            WaitReason::Unclear => {
                self.run(cx, id);
                self.unclear_passed(cx, id);
            }
            WaitReason::ReviewRoundsSpent => {
                self.run(cx, id);
                self.enter(cx, id, Station::Build);
                self.advance(cx, id);
            }
            WaitReason::CriteriaObjection { .. } => {
                self.emit(cx, id, Event::ObjectionOverruled);
                let text = "The user keeps the acceptance criteria as they are; carry out the order against them.";
                self.emit(
                    cx,
                    id,
                    Event::Commented {
                        role: Role::Builder,
                        text: text.to_string(),
                    },
                );
                self.run(cx, id);
                self.set_phase(cx, id, Phase::Idle);
                self.advance(cx, id);
            }
            WaitReason::CancelledTurn
            | WaitReason::NoAgent
            | WaitReason::NoReviewer
            | WaitReason::TakeOverBlocked { .. } => {
                self.run(cx, id);
                self.set_phase(cx, id, Phase::Idle);
                self.advance(cx, id);
            }
            WaitReason::SpecApproval
            | WaitReason::Acceptance
            | WaitReason::Question { .. }
            | WaitReason::GateCommandsChanged
            | WaitReason::NoGatePlan
            | WaitReason::ByHand => cx.refuse(Some(id), Refused::NotNow),
        }
    }

    fn answer(&mut self, cx: &mut Cx, id: &OrderId, request: crate::ids::RequestId, text: String) {
        let fits = |o: &Order| o.request() == Some(&request);
        if !self.fitting(cx, id, fits) {
            return;
        }
        let session = self
            .orders
            .get(id)
            .and_then(Order::turn)
            .and_then(|t| self.orders.get(id).and_then(|o| o.session(t.role())))
            .map(|s| s.session().clone());
        if let Some(session) = session {
            cx.effect(Effect::AnswerRequest {
                order: id.clone(),
                session,
                request: request.clone(),
                answer: Answer::Text(text),
            });
        }
        self.answered_by_user(cx, id, request);
    }

    fn answered_by_user(&mut self, cx: &mut Cx, id: &OrderId, request: crate::ids::RequestId) {
        self.emit(cx, id, Event::Answered { request });
        self.run(cx, id);
        let prompt = self
            .orders
            .get(id)
            .and_then(Order::turn)
            .map(crate::order::Turn::prompt);
        match prompt {
            Some(prompt) => self.set_phase(cx, id, Phase::Turn { prompt }),
            None => self.advance(cx, id),
        }
    }

    fn confirm_gates(&mut self, cx: &mut Cx, hash: String) {
        if hash != self.rules.gate_hash() {
            cx.refuse(None, Refused::StaleConfirmation);
            return;
        }
        self.rules.confirm_gates();
        cx.effect(Effect::Confirm { hash });
        self.retry_waiting(cx, |r| matches!(r, WaitReason::GateCommandsChanged));
    }

    fn frozen(&mut self, cx: &mut Cx, id: &OrderId, result: Result<String, String>) {
        if self.orders.get(id).map(Order::phase) != Some(&Phase::Freezing) {
            return;
        }
        match result {
            Ok(dir) => {
                self.emit(cx, id, Event::Frozen { dir });
                self.set_phase(cx, id, Phase::Idle);
                self.advance(cx, id);
            }
            Err(detail) => self.fail(
                cx,
                id,
                crate::station::FailReason::Effect {
                    effect: "freeze configuration".into(),
                    detail,
                },
            ),
        }
    }

    /// The ref watch after a turn (E173): changes Dango cannot account for let the order wait.
    fn refs(&mut self, cx: &mut Cx, id: &OrderId, changes: Vec<crate::input::RefChange>) {
        let Some(order) = self.orders.get(id) else { return };
        if *order.phase() != Phase::Refs {
            return;
        }
        let own = order.head().map(|(c, _)| c.to_string());
        let branch = format!("refs/heads/{}", id.branch());
        let unexplained: Vec<String> = changes
            .into_iter()
            .filter(|c| !c.by_dango)
            .filter(|c| !(c.name == branch && c.after == own))
            .map(|c| c.name)
            .collect();
        if unexplained.is_empty() {
            self.set_phase(cx, id, Phase::Idle);
            self.leave_build(cx, id);
        } else {
            self.wait(cx, id, WaitReason::RefsChanged { refs: unexplained });
        }
    }
}
