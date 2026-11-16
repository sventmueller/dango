//! How each event changes an order (E167): the one place its state changes, for the running factory and a restart
//! alike.

use super::{Order, Slot, Turn, Visit};
use crate::event::{Action, Event, OpResult, PromptKind, TurnOutcome};
use crate::evidence::{Deviation, Provenance, order_deviations};
use crate::ids::{OperationId, PromptId, SessionKey};
use crate::limits::TurnWatch;
use crate::log::LogEntry;
use crate::routing::{RankEntry, Vendor};
use crate::station::{Condition, Role, Station};
use crate::time::Stamp;

impl Order {
    /// Applies one log entry.
    pub(crate) fn apply(&mut self, entry: &LogEntry) {
        let at = entry.at;
        self.updated = at;
        self.seq = entry.seq + 1;
        match &entry.event {
            Event::Created { .. } => {}
            Event::OptionChanged { change } => self.change_option(change),
            Event::CriteriaChanged { criteria } => self.criteria.clone_from(criteria),
            Event::Entered { station } => self.enter(*station, at),
            Event::Skipped {
                station: Station::Spec, ..
            } => self.g1_skipped = true,
            Event::Skipped { .. } | Event::BackToLeitstand | Event::LeftoversStopped { .. } => {}
            Event::Condition { condition } => self.set_condition(condition.clone(), at),
            Event::Phase { phase } => self.phase = phase.clone(),
            Event::Begun { op, action, .. } => {
                self.ops.insert(op.clone(), action.clone());
                self.bump_op(op);
            }
            Event::Finished { op, result } => self.finish(op, result),
            Event::Dropped { op } => {
                self.ops.remove(op);
            }
            Event::Frozen { dir } => self.frozen = Some(dir.clone()),
            Event::Rebased { base } => self.base.commit.clone_from(base),
            Event::Head { commit, tree } => self.head = Some((commit.clone(), tree.clone())),
            Event::Asked { request } => self.request = Some(request.clone()),
            Event::Answered { request } => {
                if self.request.as_ref() == Some(request) {
                    self.request = None;
                }
            }
            Event::Classified { .. }
            | Event::Acknowledged
            | Event::Gate { .. }
            | Event::Reviewed { .. }
            | Event::Findings { .. }
            | Event::Sealed { .. }
            | Event::Invalidated { .. }
            | Event::Deviated { .. }
            | Event::Specified { .. }
            | Event::Approved
            | Event::Commented { .. }
            | Event::DiffOpened
            | Event::Accepted { .. }
            | Event::AcceptanceKept
            | Event::ByHand => self.apply_review(&entry.event),
            Event::BranchKept => self.branch_kept = true,
            Event::CriteriaReplied { reply } => {
                if !matches!(reply, crate::contract::CriteriaReply::Objection(_)) {
                    self.agreed = Some(self.criteria.clone());
                }
            }
            Event::ObjectionOverruled => self.agreed = Some(self.criteria.clone()),
            Event::IssueChanged { body } => {
                let frozen = self.options.issue.as_ref().is_some_and(|o| o.unchanged(body));
                self.issue_update = (!frozen).then(|| body.clone());
            }
            Event::IssueTaken { origin } => {
                self.goal = origin.body.trim().to_string();
                self.options.issue = Some(origin.clone());
                self.issue_update = None;
            }
            Event::SessionStarted { .. }
            | Event::SessionFailed { .. }
            | Event::SessionEnded { .. }
            | Event::Prompted { .. }
            | Event::TurnEnded { .. }
            | Event::TurnStopped { .. }
            | Event::Retry { .. }
            | Event::Spent { .. }
            | Event::Intervened { .. }
            | Event::ModelSwitched { .. }
            | Event::AgentSwitched { .. } => self.apply_session(&entry.event, at),
        }
    }

    fn apply_session(&mut self, event: &Event, at: Stamp) {
        match event {
            Event::SessionStarted {
                role,
                entry,
                index,
                vendor,
                session,
                resume,
                quota_unknown,
                same_vendor,
                tie_break,
            } => {
                self.start_session(*role, entry, *index, *vendor, session, resume.clone());
                if *quota_unknown {
                    self.deviate(Deviation::QuotaUnknown);
                }
                if *same_vendor {
                    self.deviate(Deviation::SameVendorReview);
                }
                if let Some(note) = tie_break {
                    self.deviate(Deviation::TieBreak {
                        role: *role,
                        chosen: entry.clone(),
                        over: note.over.clone(),
                        percent: percent(note.rate),
                        over_percent: percent(note.over_rate),
                    });
                }
            }
            Event::SessionFailed { index, .. } => self.excluded.push(*index),
            Event::SessionEnded { role } => {
                if let Some(slot) = self.sessions.get_mut(role) {
                    slot.alive = false;
                }
            }
            Event::Prompted { prompt, role, kind } => self.prompted(*prompt, *role, *kind, at),
            Event::TurnEnded { prompt, outcome } => {
                self.count_runtime(*prompt, at);
                self.turn_ended(*prompt, outcome);
            }
            Event::TurnStopped { prompt, .. } => {
                self.count_runtime(*prompt, at);
                if self.turn.as_ref().is_some_and(|t| t.prompt == *prompt) {
                    self.turn = None;
                }
            }
            Event::Retry { counted, delay_ms, .. } => {
                if *counted {
                    self.attempts += 1;
                }
                self.retry_due = Some(at.mono.after(*delay_ms));
            }
            Event::Spent {
                role, spend, unmetered, ..
            } => {
                self.spend.add(spend);
                self.role_spend.entry(*role).or_default().add(spend);
                self.costs.entry(self.station).or_default().add(spend);
                self.metering.unmetered |= *unmetered;
                if *unmetered {
                    self.deviate(Deviation::Unmetered);
                }
            }
            Event::Intervened { .. } => {
                self.interventions += 1;
                let count = self.interventions;
                self.deviations
                    .retain(|d| !matches!(d, Deviation::Interventions { .. }));
                self.deviate(Deviation::Interventions { count });
            }
            Event::ModelSwitched { role, model, vendor } => {
                if let Some(slot) = self.sessions.get_mut(role) {
                    slot.entry.model = Some(model.clone());
                    slot.vendor = *vendor;
                }
                if *role == Role::Builder && !self.builders.contains(vendor) {
                    self.builders.push(*vendor);
                }
                self.deviate(Deviation::ModelSwitched {
                    role: *role,
                    model: model.clone(),
                });
            }
            Event::AgentSwitched { role, from, index, .. } => {
                self.switching = Some((*role, from.clone()));
                self.excluded.push(*index);
                if let Some(slot) = self.sessions.get_mut(role) {
                    slot.alive = false;
                    slot.resume = None;
                }
            }
            _ => {}
        }
    }

    fn apply_review(&mut self, event: &Event) {
        match event {
            Event::Classified { classification } => {
                self.classification = Some(classification.clone());
                self.acknowledged = false;
            }
            Event::Acknowledged => self.acknowledged = true,
            Event::Gate { record } => self.gates.push(record.clone()),
            Event::Reviewed { round } => self.review_log.push(round.clone()),
            Event::Findings { text, round } => {
                self.findings = Some(text.clone());
                self.rounds += u32::from(*round);
                self.partial = false;
            }
            Event::Sealed { evidence } => self.evidence = Some((**evidence).clone()),
            Event::Invalidated { why } => {
                if let Some(evidence) = &mut self.evidence {
                    evidence.invalidate(*why);
                }
                if !self.kept {
                    self.accepted = None;
                }
            }
            Event::Deviated { deviation } => {
                if *deviation == Deviation::BudgetOvershoot {
                    self.metering.overshoot = true;
                }
                self.deviate(deviation.clone());
            }
            Event::Specified { markdown, risk } => {
                self.spec = Some(markdown.clone());
                self.spec_risk = *risk;
            }
            Event::Approved => self.approved = true,
            Event::Commented { role, text } => {
                if *role == Role::Builder {
                    self.comment = Some(text.clone());
                } else {
                    self.approved = false;
                    self.findings = Some(text.clone());
                }
            }
            Event::DiffOpened => self.diff_opened = true,
            Event::Accepted { fingerprint, rejected } => {
                self.accepted = Some(fingerprint.clone());
                self.partial = !rejected.is_empty();
                self.rejected.clone_from(rejected);
            }
            Event::AcceptanceKept => {
                self.kept = true;
                self.deviate(Deviation::BaseUpdated);
            }
            Event::ByHand => {
                self.by_hand = true;
                self.deviate(Deviation::ByHand);
            }
            _ => {}
        }
    }

    fn change_option(&mut self, change: &crate::options::OptionChange) {
        use crate::options::OptionChange;
        match change {
            OptionChange::Risk(risk) => self.options.risk = *risk,
            OptionChange::Scope(scope) => self.options.scope.clone_from(scope),
            OptionChange::Budget(budget) => self.options.budget.clone_from(budget),
            OptionChange::Casting(role, entry) => match entry {
                Some(entry) => {
                    self.options.casting.insert(*role, entry.clone());
                }
                None => {
                    self.options.casting.remove(role);
                }
            },
            OptionChange::SkipSpecApproval(on) => self.options.skip_spec_approval = *on,
        }
    }

    fn enter(&mut self, station: Station, at: Stamp) {
        self.station = station;
        self.reached = self.reached.max(station);
        self.attempts = 0;
        self.excluded.clear();
        self.retry_due = None;
        self.visits.push(Visit { station, at });
        if station == Station::Review {
            self.gates.clear();
        }
        if station == Station::Build {
            self.kept = false;
        }
    }

    fn set_condition(&mut self, condition: Condition, at: Stamp) {
        if condition.waits() {
            self.waiting_since.get_or_insert(at);
        } else if let Some(since) = self.waiting_since.take() {
            self.waited_ms += at.wall.saturating_sub(since.wall);
        }
        if condition == Condition::Running {
            self.retry_due = None;
        }
        self.condition = condition;
    }

    fn bump_op(&mut self, op: &OperationId) {
        if let Some(n) = op.0.rsplit('-').next().and_then(|n| n.parse::<u64>().ok()) {
            self.next_op = self.next_op.max(n + 1);
        }
    }

    fn finish(&mut self, op: &OperationId, result: &OpResult) {
        let action = self.ops.remove(op);
        match result {
            OpResult::Prepared { path, tree, copied, .. } => {
                self.worktree = Some(path.clone());
                self.copied.clone_from(copied);
                self.head = Some((self.base.commit.clone(), tree.clone()));
            }
            OpResult::Committed { commit, tree } => {
                self.head = Some((commit.clone(), tree.clone()));
                if let Some(last) = self.provenance.last_mut() {
                    last.commit = Some(commit.clone());
                }
            }
            OpResult::Merged { head, tree, base, .. } => {
                self.head = Some((head.clone(), tree.clone()));
                self.base.commit.clone_from(base);
            }
            OpResult::Removed => match action {
                Some(Action::RemoveWorktree { .. }) => self.worktree = None,
                Some(Action::RemoveCheckpoints { .. }) => self.checkpoints_removed = true,
                _ => {}
            },
            OpResult::TakenOver { .. } => self.taken_over = true,
            _ => {}
        }
    }

    fn start_session(
        &mut self,
        role: Role,
        entry: &RankEntry,
        index: usize,
        vendor: Vendor,
        session: &SessionKey,
        resume: Option<String>,
    ) {
        self.sessions.insert(
            role,
            Slot {
                session: session.clone(),
                entry: entry.clone(),
                index,
                vendor,
                resume,
                alive: true,
            },
        );
        if role == Role::Builder && !self.builders.contains(&vendor) {
            self.builders.push(vendor);
        }
        if let Some((switched, from)) = self.switching.take()
            && switched == role
            && from != entry.agent
        {
            self.switched = true;
            self.deviate(Deviation::AgentSwitched {
                role,
                from,
                to: entry.agent.clone(),
            });
        }
    }

    fn prompted(&mut self, prompt: PromptId, role: Role, kind: PromptKind, at: Stamp) {
        self.turn = Some(Turn {
            prompt,
            role,
            kind,
            watch: TurnWatch::start(at.mono),
            queued: 0,
            ended: None,
            answer: None,
            started_wall: at.wall,
        });
        self.next_prompt = self.next_prompt.max(prompt.0 + 1);
        *self.turns.entry(self.station).or_default() += 1;
        if matches!(kind, PromptKind::Rework | PromptKind::Plan | PromptKind::Handoff) {
            self.findings = None;
            self.comment = None;
        }
    }

    fn count_runtime(&mut self, prompt: PromptId, at: Stamp) {
        if let Some(turn) = self.turn.as_ref().filter(|t| t.prompt == prompt) {
            let ms = at.wall.saturating_sub(turn.started_wall);
            *self.role_runtime.entry(turn.role).or_default() += ms;
        }
    }

    fn turn_ended(&mut self, prompt: PromptId, outcome: &TurnOutcome) {
        let Some(turn) = self.turn.as_ref().filter(|t| t.prompt == prompt) else {
            return;
        };
        let role = turn.role;
        if *outcome == TurnOutcome::Succeeded
            && let Some(slot) = self.sessions.get(&role)
        {
            self.provenance.push(Provenance {
                role,
                agent: slot.entry.agent.clone(),
                model: slot.entry.model.clone(),
                vendor: slot.vendor,
                commit: None,
            });
        }
        self.turn = None;
    }

    fn deviate(&mut self, deviation: Deviation) {
        self.deviations.push(deviation);
        order_deviations(&mut self.deviations);
    }
}

/// A share from 0 to 1 in whole percent.
fn percent(share: f64) -> u32 {
    let rounded = (share.clamp(0.0, 1.0) * 100.0).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=100"
    )]
    let percent = rounded as u32;
    percent
}
