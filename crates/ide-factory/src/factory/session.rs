//! Sessions and turns of an order: routing a role to an entry (E170), prompts, the end of a turn (E189), retries with
//! a growing wait, the stall rule (E168), budgets before each turn and quotas while a station runs (C64, C65).

use std::collections::BTreeMap;

use super::{Cx, Factory};
use crate::budget::{BudgetScope, Spend, UsageReport};
use crate::effect::{Answer, Effect};
use crate::event::{Event, Phase, PromptKind, TurnOutcome};
use crate::evidence::Deviation;
use crate::ids::{OrderId, SessionKey};
use crate::input::{RequestKind, SessionEvent};
use crate::order::Order;
use crate::prompt::{render, untrusted};
use crate::quota::{QuotaObservation, QuotaState};
use crate::routing::{RankEntry, Route, RouteContext, Skip, route, vendor_of};
use crate::station::{Condition, FailReason, PauseReason, Role, Station, WaitReason};
use crate::verdict::VERDICT_SCHEMA;

impl Factory {
    /// The ranked list of `role` for `order`: its fixed entry, else the rule set's list. The scout's list goes on with
    /// the reviewer's entries, so a scout of the reviewer's vendor reads when no cheap or local one is ready (B47).
    fn role_list(&self, order: &Order, role: Role) -> Vec<RankEntry> {
        if let Some(entry) = order.options().casting.get(&role) {
            return vec![entry.clone()];
        }
        let mut list = self.rules.role(role).to_vec();
        if role == Role::Scout {
            for entry in self.rules.role(Role::Reviewer) {
                if !list.contains(entry) {
                    list.push(entry.clone());
                }
            }
        }
        list
    }

    /// The first budget scope reached for `order`, counting the billing service `service`.
    fn budget_reached(&self, order: &Order, service: Option<&str>) -> Option<BudgetScope> {
        let budgets = self.rules.budgets();
        let order_budget = order
            .options()
            .budget
            .clone()
            .unwrap_or_else(|| budgets.order().clone());
        if order_budget.reached(order.spend()).is_some() {
            return Some(BudgetScope::Order);
        }
        if budgets.day().reached(self.day.total()).is_some() {
            return Some(BudgetScope::Day);
        }
        let service = service?;
        let spent = self.day.provider(service).copied().unwrap_or_default();
        budgets
            .provider(service)
            .and_then(|b| b.reached(&spent))
            .map(|_| BudgetScope::Provider(service.to_string()))
    }

    fn route_for(&self, order: &Order, role: Role, list: &[RankEntry], excluded: &[usize]) -> Route {
        let fresh = order.turns_in(order.station()) == 0;
        let start_limit = if fresh {
            self.rules.limits().window_start_share()
        } else {
            1.0
        };
        let reached = |service: Option<&str>| self.budget_reached(order, service).is_some();
        let risk = order.risk();
        let first_pass = |entry: &RankEntry| self.metrics.rate(role, &entry.agent, entry.model.as_deref(), risk);
        let cx = RouteContext {
            agents: &self.agents,
            quotas: &self.quotas,
            now_wall: self.now.wall,
            start_limit,
            budget_reached: &reached,
            excluded,
            builders: order.builders(),
            rule: self.rules.reviewer_rule(),
            first_pass: &first_pass,
        };
        route(role, list, &cx)
    }

    /// Whether routing would find an entry for the role of `order`'s station now, without recording anything.
    pub(super) fn can_route(&self, order: &Order) -> bool {
        let role = station_role(order);
        let list = self.role_list(order, role);
        self.route_for(order, role, &list, order.excluded()).choice.is_some()
    }

    /// The live session of `role`, or `None` after asking for one to start; without an entry the order waits or
    /// pauses with why (E170).
    pub(super) fn ensure_session(&mut self, cx: &mut Cx, id: &OrderId, role: Role) -> Option<SessionKey> {
        let order = self.orders.get(id)?;
        if let Some(slot) = order.session(role).filter(|s| s.alive()) {
            return Some(slot.session().clone());
        }
        let list = self.role_list(order, role);
        let handoff = self.scratch.get(id).is_some_and(|s| s.handoff);
        let resumable = order
            .session(role)
            .filter(|slot| slot.resume().is_some() && !handoff && !order.excluded().contains(&slot.index()));
        if let Some(slot) = resumable {
            let alone = self.route_for(order, role, std::slice::from_ref(slot.entry()), &[]);
            if let Some(mut choice) = alone.choice {
                choice.index = slot.index();
                let resume = slot.resume().map(str::to_string);
                self.start(cx, id, role, choice, false, resume);
                return None;
            }
        }
        let routed = self.route_for(order, role, &list, order.excluded());
        match routed.choice {
            Some(choice) => self.start(cx, id, role, choice, routed.same_vendor, None),
            None => self.no_entry(cx, id, role, &routed),
        }
        None
    }

    fn no_entry(&mut self, cx: &mut Cx, id: &OrderId, role: Role, routed: &Route) {
        let skips = || routed.skipped.iter().map(|(_, skip)| skip);
        if skips().any(|s| matches!(s, Skip::Quota { .. })) {
            self.pause(cx, id, PauseReason::Quota);
        } else if !routed.skipped.is_empty() && skips().all(|s| matches!(s, Skip::Budget)) {
            let scope = self
                .orders
                .get(id)
                .and_then(|order| self.budget_reached(order, None))
                .unwrap_or(BudgetScope::Order);
            self.pause(cx, id, PauseReason::Budget { scope });
        } else if role == Role::Reviewer && skips().any(|s| matches!(s, Skip::SameVendor)) {
            self.wait(cx, id, WaitReason::NoReviewer);
        } else {
            self.wait(cx, id, WaitReason::NoAgent);
        }
    }

    fn start(
        &mut self,
        cx: &mut Cx,
        id: &OrderId,
        role: Role,
        choice: crate::routing::Choice,
        same_vendor: bool,
        resume: Option<String>,
    ) {
        let Some(order) = self.orders.get(id) else { return };
        let capabilities = self.agents.get(&choice.entry.agent).map_or_else(
            || crate::routing::Capabilities::of(&choice.entry.agent),
            |s| s.capabilities,
        );
        let budget_usd = capabilities
            .takes_budget
            .then(|| {
                let budget = order
                    .options()
                    .budget
                    .clone()
                    .unwrap_or_else(|| self.rules.budgets().order().clone());
                budget.dollars_left(order.spend())
            })
            .flatten();
        let effect = Effect::StartSession {
            order: id.clone(),
            role,
            entry: choice.entry.clone(),
            resume,
            worktree: order.worktree().unwrap_or_default().to_string(),
            frozen: order.frozen().unwrap_or_default().to_string(),
            read_only: role.reads_only(),
            subagents: capabilities.reports_subagents,
            network: self.rules.network().to_vec(),
            tools: self.rules.tools(role).to_vec(),
            budget_usd,
            checkpoint_thread: id.checkpoint_thread(role),
        };
        let index = choice.index;
        self.scratch(id).choice = Some((choice, same_vendor));
        self.set_phase(cx, id, Phase::Starting { role, index });
        cx.effect(effect);
    }

    /// A session the factory asked for started.
    pub(super) fn session_started(
        &mut self,
        cx: &mut Cx,
        id: &OrderId,
        role: Role,
        session: SessionKey,
        resume: Option<String>,
    ) {
        let Some(order) = self.orders.get(id) else { return };
        let Phase::Starting { role: wanted, index } = order.phase().clone() else {
            return;
        };
        let Some((choice, same_vendor)) = self.scratch(id).choice.take() else {
            return;
        };
        if wanted != role || choice.index != index {
            return;
        }
        self.emit(
            cx,
            id,
            Event::SessionStarted {
                role,
                entry: choice.entry,
                index,
                vendor: choice.vendor,
                session,
                resume,
                quota_unknown: choice.quota == QuotaState::Unknown,
                same_vendor,
                tie_break: choice.tie_break,
            },
        );
        self.set_phase(cx, id, Phase::Idle);
        self.advance(cx, id);
    }

    /// A session could not start: the entry counts as not ready, and routing passes to the next (E170).
    pub(super) fn session_failed(
        &mut self,
        cx: &mut Cx,
        id: &OrderId,
        role: Role,
        readiness: crate::routing::Readiness,
    ) {
        let Some(order) = self.orders.get(id) else { return };
        let Phase::Starting { role: wanted, index } = order.phase().clone() else {
            return;
        };
        if wanted != role {
            return;
        }
        if let Some((choice, _)) = self.scratch(id).choice.take()
            && let Some(status) = self.agents.get_mut(&choice.entry.agent)
        {
            status.readiness = readiness.clone();
        }
        self.emit(cx, id, Event::SessionFailed { role, index, readiness });
        self.set_phase(cx, id, Phase::Idle);
        self.advance(cx, id);
    }

    /// Ends `role`'s session after its station (E189).
    pub(super) fn end_session(&mut self, cx: &mut Cx, id: &OrderId, role: Role) {
        let Some(slot) = self.orders.get(id).and_then(|o| o.session(role)).filter(|s| s.alive()) else {
            return;
        };
        cx.effect(Effect::EndSession {
            order: id.clone(),
            session: slot.session().clone(),
        });
        self.emit(cx, id, Event::SessionEnded { role });
    }

    /// Sends `role` the prompt of `kind`, after checking the budget and the station's turns (E168, C64).
    pub(super) fn prompt(&mut self, cx: &mut Cx, id: &OrderId, role: Role, session: &SessionKey, kind: PromptKind) {
        let Some(order) = self.orders.get(id) else { return };
        let service = order
            .session(role)
            .and_then(|slot| self.agents.get(&slot.entry().agent))
            .and_then(|status| status.service.clone());
        if let Some(scope) = self.budget_reached(order, service.as_deref()) {
            self.pause(cx, id, PauseReason::Budget { scope });
            return;
        }
        if order.turns_in(order.station()) >= self.rules.limits().turns_per_station() {
            self.fail(cx, id, FailReason::TurnsSpent);
            return;
        }
        let text = self.prompt_text(order, kind);
        let prompt = order.next_prompt();
        let schema = (kind == PromptKind::Review).then(|| VERDICT_SCHEMA.to_string());
        self.emit(cx, id, Event::Prompted { prompt, role, kind });
        self.set_phase(cx, id, Phase::Turn { prompt });
        self.scratch(id).handoff = false;
        cx.effect(Effect::SnapshotRefs { order: id.clone() });
        cx.effect(Effect::SendPrompt {
            order: id.clone(),
            session: session.clone(),
            prompt,
            text,
            schema,
        });
    }

    fn prompt_text(&self, order: &Order, kind: PromptKind) -> String {
        let mut values: BTreeMap<&str, String> = BTreeMap::new();
        values.insert("title", order.title().to_string());
        values.insert("goal", order.goal().to_string());
        values.insert("criteria", Factory::criteria_text(order.criteria()));
        if let Some(origin) = &order.options().issue {
            let source = format!("the text of issue #{}", origin.number);
            values.insert("goal", untrusted(&source, order.goal()));
            if kind == PromptKind::Plan && !order.criteria().is_empty() {
                let suggested = untrusted(
                    "criteria suggested by the issue",
                    &Factory::criteria_text(order.criteria()),
                );
                values.insert(
                    "criteria",
                    format!("Make these suggestions testable criteria:\n\n{suggested}"),
                );
            }
        }
        if let Some(findings) = order.findings() {
            values.insert("findings", findings.to_string());
        }
        if let Some(comment) = order.comment() {
            values.insert("comment", format!("The user's comment:\n{comment}"));
        }
        if !order.options().scope.is_empty() {
            let list: Vec<String> = order.options().scope.iter().map(|p| format!("- {p}")).collect();
            values.insert("scope", format!("## Path scope\n\n{}", list.join("\n")));
        }
        if kind == PromptKind::Handoff {
            values.insert("summary", summary(order));
        }
        if kind == PromptKind::Review {
            values.insert("schema", VERDICT_SCHEMA.to_string());
            let gates: Vec<String> = order
                .gates()
                .iter()
                .map(|g| {
                    let log = g
                        .command
                        .as_ref()
                        .map(|c| untrusted("a gate log", &c.excerpt))
                        .unwrap_or_default();
                    format!(
                        "- {}: {}\n{log}",
                        g.name,
                        if g.failure.is_none() { "passed" } else { "failed" }
                    )
                })
                .collect();
            values.insert("gates", format!("## Gate results\n\n{}", gates.join("\n")));
            if let Some((diff, commits)) = self.scratch.get(order.id()).and_then(|s| s.review_input.clone()) {
                values.insert("diff", untrusted("the diff of the change", &diff));
                values.insert("commits", untrusted("the commit messages", &commits));
            }
        }
        render(self.rules.template(kind), &values)
    }

    /// Handles an event of a session of order `id`.
    pub(super) fn session_event(&mut self, cx: &mut Cx, id: &OrderId, key: &SessionKey, event: SessionEvent) {
        let Some(order) = self.orders.get_mut(id) else { return };
        let Some(role) = Role::ALL
            .into_iter()
            .find(|r| order.session(*r).is_some_and(|s| s.session() == key))
        else {
            return;
        };
        let now = cx.now.mono;
        order.touch(now);
        match event {
            SessionEvent::Activity {
                running,
                tool_since,
                subagents,
                background,
                queued,
            } => {
                order.watch(now, running, tool_since, subagents, background, queued);
                self.settle(cx, id);
            }
            SessionEvent::TurnEnded {
                prompt,
                outcome,
                answer,
            } => {
                let Some(turn) = order.turn() else { return };
                if prompt.is_none() {
                    self.settle(cx, id);
                    return;
                }
                if prompt != Some(turn.prompt()) || turn.role() != role {
                    return;
                }
                let busy = turn.watch().subagents() > 0 || turn.queued() > 0;
                if outcome == TurnOutcome::Succeeded && busy {
                    order.hold_outcome(outcome, answer);
                } else {
                    self.turn_over(cx, id, role, outcome, answer);
                }
            }
            SessionEvent::Request { request, kind } => match kind {
                RequestKind::Plan => cx.effect(Effect::AnswerRequest {
                    order: id.clone(),
                    session: key.clone(),
                    request,
                    answer: Answer::Refuse("Orders do not take plan requests; carry on within the order.".into()),
                }),
                RequestKind::Question | RequestKind::OutsideBounds => {
                    self.emit(
                        cx,
                        id,
                        Event::Asked {
                            request: request.clone(),
                        },
                    );
                    self.wait(cx, id, WaitReason::Question { request });
                }
            },
            SessionEvent::Answered { request } => self.answered(cx, id, request),
            SessionEvent::Usage { report } => self.spent(cx, id, role, key, report),
            SessionEvent::Gap { reconciled } => {
                if !reconciled {
                    self.stop_work(cx, id);
                    self.pause(cx, id, PauseReason::StreamGap);
                }
            }
            SessionEvent::Exited => {
                self.emit(cx, id, Event::SessionEnded { role });
                let running = self
                    .orders
                    .get(id)
                    .and_then(Order::turn)
                    .is_some_and(|t| t.role() == role);
                if running {
                    self.turn_over(cx, id, role, TurnOutcome::ConnectionLost, None);
                }
            }
            SessionEvent::Intervened => self.emit(cx, id, Event::Intervened { role }),
            SessionEvent::ModelSwitched { model } => {
                let Some(agent) = order.session(role).map(|s| s.entry().agent.clone()) else {
                    return;
                };
                let vendor = vendor_of(&agent, Some(&model));
                self.emit(cx, id, Event::ModelSwitched { role, model, vendor });
            }
        }
    }

    /// Ends a turn whose success came while subagents or queued messages still ran, once they are done (E184, E189).
    fn settle(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get_mut(id) else { return };
        let Some(turn) = order.turn() else { return };
        let Some(outcome) = turn.ended().cloned() else { return };
        if turn.watch().subagents() > 0 || turn.queued() > 0 {
            return;
        }
        let role = turn.role();
        let answer = order.take_answer();
        self.turn_over(cx, id, role, outcome, answer);
    }

    fn answered(&mut self, cx: &mut Cx, id: &OrderId, request: crate::ids::RequestId) {
        let Some(order) = self.orders.get(id) else { return };
        if order.request() != Some(&request) {
            return;
        }
        let waiting = matches!(order.condition(), Condition::Waiting(WaitReason::Question { .. }));
        self.emit(cx, id, Event::Answered { request });
        if waiting {
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
    }

    /// A turn ended: success leads on, failure and a lost connection to a retry, a cancel to the user (E189).
    pub(super) fn turn_over(
        &mut self,
        cx: &mut Cx,
        id: &OrderId,
        role: Role,
        outcome: TurnOutcome,
        answer: Option<String>,
    ) {
        let Some(order) = self.orders.get(id) else { return };
        let Some(turn) = order.turn() else { return };
        let (prompt, background) = (turn.prompt(), turn.watch().background());
        let session = order.session(role).map(|s| s.session().clone());
        self.emit(
            cx,
            id,
            Event::TurnEnded {
                prompt,
                outcome: outcome.clone(),
            },
        );
        if outcome == TurnOutcome::ConnectionLost {
            self.emit(cx, id, Event::SessionEnded { role });
        }
        if self.scratch.get(id).is_some_and(|s| s.switch) {
            self.switch(cx, id, role);
        }
        match outcome {
            TurnOutcome::Succeeded => {
                if background > 0
                    && let Some(session) = session
                {
                    cx.effect(Effect::StopBackground {
                        order: id.clone(),
                        session,
                    });
                    self.emit(cx, id, Event::LeftoversStopped { background });
                }
                match role {
                    Role::Planner | Role::Scout => match answer {
                        Some(answer) if !answer.trim().is_empty() => self.specified(cx, id, &answer),
                        _ => self.failure(cx, id),
                    },
                    Role::Builder => self.after_turn(cx, id),
                    Role::Reviewer => self.reviewed(cx, id, answer.as_deref().unwrap_or_default()),
                }
            }
            TurnOutcome::Failed(_) | TurnOutcome::ConnectionLost => {
                if self.scratch.get(id).is_some_and(|s| s.handoff) {
                    self.set_phase(cx, id, Phase::Idle);
                    self.advance(cx, id);
                } else {
                    self.failure(cx, id);
                }
            }
            TurnOutcome::Cancelled => self.wait(cx, id, WaitReason::CancelledTurn),
        }
    }

    /// A failed attempt: a retry after 10 s · 2^(n−1), or the station fails after the last attempt (E168). Failures
    /// within the grace after waking do not count.
    pub(super) fn failure(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        let counted = !self.in_wake_grace(cx.now.mono);
        let failed = order.attempts() + u32::from(counted);
        let limits = self.rules.limits();
        if failed >= limits.attempts() {
            let role = station_role(order);
            self.end_session(cx, id, role);
            self.fail(cx, id, FailReason::AttemptsSpent);
            return;
        }
        let delay_ms = limits.retry_delay_ms(failed.max(1));
        self.emit(
            cx,
            id,
            Event::Retry {
                attempt: failed + 1,
                counted,
                delay_ms,
            },
        );
        self.set_phase(cx, id, Phase::Retrying);
    }

    /// Hands `role`'s station to the next entry of its list (E170).
    fn switch(&mut self, cx: &mut Cx, id: &OrderId, role: Role) {
        self.scratch(id).switch = false;
        let Some(slot) = self.orders.get(id).and_then(|o| o.session(role)).cloned() else {
            return;
        };
        if slot.alive() {
            cx.effect(Effect::EndSession {
                order: id.clone(),
                session: slot.session().clone(),
            });
        }
        self.emit(
            cx,
            id,
            Event::AgentSwitched {
                role,
                from: slot.entry().agent.clone(),
                index: slot.index(),
                quota: true,
            },
        );
        self.scratch(id).handoff = true;
    }

    fn spent(&mut self, cx: &mut Cx, id: &OrderId, role: Role, key: &SessionKey, report: UsageReport) {
        let Some(order) = self.orders.get(id) else { return };
        let status = order
            .session(role)
            .and_then(|s| self.agents.get(&s.entry().agent))
            .cloned();
        let service = status.as_ref().and_then(|s| s.service.clone());
        let has_quota = status
            .as_ref()
            .is_some_and(|s| s.account.is_some() && s.service.is_some());
        let previous = self.scratch(id).usage.insert(key.clone(), report);
        let before = previous.unwrap_or_default();
        let dollars = match (report.dollars, before.dollars) {
            (Some(now), Some(then)) => (now - then).max(0.0),
            (Some(now), None) => now,
            _ => 0.0,
        };
        let tokens = match (report.tokens_reliable, report.tokens, before.tokens) {
            (true, Some(now), Some(then)) => now.saturating_sub(then),
            (true, Some(now), None) => now,
            _ => 0,
        };
        let unmetered = report.dollars.is_none() && !(report.tokens_reliable && report.tokens.is_some()) && !has_quota;
        let spend = Spend::new(dollars, tokens, 0.0);
        let first = previous.is_none();
        if spend.is_zero() && !(unmetered && first) {
            return;
        }
        self.record_spend(cx, id, role, service, spend, unmetered);
    }

    fn record_spend(
        &mut self,
        cx: &mut Cx,
        id: &OrderId,
        role: Role,
        service: Option<String>,
        spend: Spend,
        unmetered: bool,
    ) {
        self.emit(
            cx,
            id,
            Event::Spent {
                role,
                service: service.clone(),
                spend,
                unmetered,
            },
        );
        if !spend.is_zero() {
            let day = self.day.day().to_string();
            self.day.add(&day, service.as_deref(), &spend);
            cx.effect(Effect::AddToDay {
                service: service.clone(),
                spend,
            });
        }
        let Some(order) = self.orders.get(id) else { return };
        let over = self.budget_reached(order, service.as_deref()).is_some();
        if over && order.turn().is_some() && !order.metering().overshoot {
            self.emit(
                cx,
                id,
                Event::Deviated {
                    deviation: Deviation::BudgetOvershoot,
                },
            );
        }
    }

    /// A quota was observed: a station whose quota ran out passes to the next entry at the end of its turn, the
    /// share of the window counts towards the running order, and orders paused for quota try again (E170, C65).
    pub(super) fn quota(&mut self, cx: &mut Cx, observation: QuotaObservation) {
        let key = observation.key.clone();
        let previous = self
            .quotas
            .observations()
            .find(|o| o.key == key)
            .filter(|o| crate::quota::QuotaBook::current(o, observation.observed_at))
            .and_then(|o| o.utilization);
        self.quotas.observe(observation.clone());
        let users: Vec<(OrderId, Role)> = self
            .orders
            .values()
            .filter(|o| *o.condition() == Condition::Running && !o.finished())
            .filter_map(|o| {
                let slot = o.current()?;
                let status = self.agents.get(&slot.entry().agent)?;
                let matches = slot.alive()
                    && status.account.as_deref() == Some(key.account.as_str())
                    && status.service.as_deref() == Some(key.service.as_str());
                matches.then(|| (o.id().clone(), station_role(o)))
            })
            .collect();
        let spent = self.quotas.state(&key.account, &key.service, self.now.wall).spent();
        if let ([(id, role)], Some(before), Some(after)) = (users.as_slice(), previous, observation.utilization) {
            let share = after - before;
            if share > 0.0 {
                let (id, role) = (id.clone(), *role);
                self.record_spend(
                    cx,
                    &id,
                    role,
                    Some(key.service.clone()),
                    Spend::new(0.0, 0, share),
                    false,
                );
            }
        }
        for (id, role) in users {
            if spent {
                let turning = self.orders.get(&id).and_then(Order::turn).is_some();
                if turning {
                    self.scratch(&id).switch = true;
                } else {
                    self.switch(cx, &id, role);
                }
            }
        }
        self.retry_quota(cx);
    }

    /// Resumes orders paused for quota once routing finds an entry again.
    pub(super) fn retry_quota(&mut self, cx: &mut Cx) {
        let ids: Vec<OrderId> = self
            .orders
            .values()
            .filter(|o| *o.condition() == Condition::Paused(PauseReason::Quota) && self.can_route(o))
            .map(|o| o.id().clone())
            .collect();
        for id in ids {
            self.resume(cx, &id);
        }
    }

    /// Time passed: overdue turns are stopped, due retries start, a gate past its deadline is stopped (E168, C67).
    pub(super) fn tick(&mut self, cx: &mut Cx) {
        let now = cx.now.mono;
        let ids: Vec<OrderId> = self.orders.keys().cloned().collect();
        for id in ids {
            let Some(order) = self.orders.get(&id) else { continue };
            if *order.condition() != Condition::Running {
                continue;
            }
            match order.phase().clone() {
                Phase::Turn { prompt } => {
                    let Some(turn) = order.turn() else { continue };
                    let Some(why) = turn.watch().overdue(now, self.rules.limits()) else {
                        continue;
                    };
                    let role = turn.role();
                    if let Some(slot) = order.session(role) {
                        let session = slot.session().clone();
                        cx.effect(Effect::StopTurn {
                            order: id.clone(),
                            session: session.clone(),
                        });
                        cx.effect(Effect::EndSession {
                            order: id.clone(),
                            session,
                        });
                    }
                    self.emit(cx, &id, Event::TurnStopped { prompt, why: Some(why) });
                    self.emit(cx, &id, Event::SessionEnded { role });
                    self.failure(cx, &id);
                }
                Phase::Retrying => {
                    if order.retry_due().is_none_or(|due| due <= now) {
                        self.set_phase(cx, &id, Phase::Idle);
                        self.advance(cx, &id);
                    }
                }
                Phase::Gate { .. } => {
                    let deadline = self.rules.limits().gate_ms() + 60_000;
                    let scratch = self.scratch(&id);
                    let late = scratch.gate_started.is_some_and(|at| now.since(at) > deadline);
                    if late && !scratch.gate_stopping {
                        scratch.gate_stopping = true;
                        cx.effect(Effect::StopGate { order: id.clone() });
                    }
                }
                _ => {}
            }
        }
        self.retry_quota(cx);
    }
}

/// The role whose session works in `order`'s station.
pub(super) fn station_role(order: &Order) -> Role {
    match order.station() {
        Station::Spec => order.spec_role(),
        Station::Review => Role::Reviewer,
        _ => Role::Builder,
    }
}

/// What happened in an order so far, for an entry that takes over its station (E170).
fn summary(order: &Order) -> String {
    let mut lines = Vec::new();
    for visit in order.visits() {
        lines.push(format!("- entered {:?}", visit.station));
    }
    for record in order.gates() {
        let result = record
            .failure
            .as_ref()
            .map_or("passed".to_string(), |f| format!("failed: {f:?}"));
        lines.push(format!("- gate {}: {result}", record.name));
    }
    if let Some(findings) = order.findings() {
        lines.push(format!("- open findings:\n{findings}"));
    }
    lines.push(format!("- turns so far: {}", order.turns_in(Station::Build)));
    lines.join("\n")
}
