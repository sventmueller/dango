//! The take-over (E164, F49, C56): one order at a time per target branch (E174), the target merged into the order's
//! branch first, the evidence checked against the new base and manifest, then the take-over and the clean-up of
//! checkpoints before the worktree (C70). Also the results of every operation and their verification after a restart
//! (E167).

use super::{Cx, Factory};
use crate::effect::{Effect, Notice};
use crate::event::{Action, Event, OpResult, Phase};
use crate::evidence::Invalidation;
use crate::ids::{OperationId, OrderId};
use crate::order::Order;
use crate::station::{Condition, FailReason, Station, WaitReason};

impl Factory {
    /// Whether another order is ahead of `order` in the take-over queue of its target branch (E174).
    fn queued_behind(&self, order: &Order) -> bool {
        let entered = |o: &Order| o.visits().iter().rev().find(|v| v.station == Station::TakeOver).map(|v| v.at);
        let Some(mine) = entered(order) else { return false };
        self.orders.values().any(|other| {
            other.id() != order.id()
                && other.station() == Station::TakeOver
                && !other.finished()
                && other.base().branch == order.base().branch
                && entered(other).is_some_and(|at| (at.wall, at.mono, other.id()) < (mine.wall, mine.mono, order.id()))
        })
    }

    /// The next step of the take-over.
    pub(super) fn take_over_next(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        if order.taken_over() {
            self.cleanup_next(cx, id, false);
            return;
        }
        if self.queued_behind(order) {
            self.set_phase(cx, id, Phase::Queue);
            return;
        }
        let head = order.head().map_or_else(String::new, |(c, _)| c.to_string());
        let scratch = self.scratch.get(id).cloned().unwrap_or_default();
        if !scratch.merged {
            let action = Action::MergeTarget {
                branch: id.branch(),
                head: head.clone(),
                target: order.base().branch.clone(),
            };
            let op = order.next_op();
            let target = order.base().branch.clone();
            self.scratch(id).before_merge = Some((order.base().commit.clone(), head));
            self.begin(cx, id, action, Phase::Merging { op: op.clone() });
            cx.effect(Effect::MergeTarget {
                order: id.clone(),
                op,
                target,
            });
            return;
        }
        let Some(manifest) = scratch.manifest else {
            self.read_manifest(cx, id);
            return;
        };
        let now = self.bound(order, manifest);
        let valid = order.evidence().is_some_and(|e| e.valid_for(&now)) && order.accepted().is_some();
        if !valid {
            let why = order
                .evidence()
                .and_then(|e| e.bound().changed(&now))
                .unwrap_or(Invalidation::Manifest);
            self.emit(cx, id, Event::Invalidated { why });
            self.enter(cx, id, Station::Review);
            self.advance(cx, id);
            return;
        }
        let expected = order.base().commit.clone();
        let target = order.base().branch.clone();
        let op = order.next_op();
        let action = Action::TakeOver {
            target: target.clone(),
            head: head.clone(),
            expected: expected.clone(),
        };
        self.begin(cx, id, action, Phase::TakingOver { op: op.clone() });
        cx.effect(Effect::TakeOver {
            order: id.clone(),
            op,
            target,
            head,
            expected,
        });
    }

    /// Removes checkpoints, then the worktree (C70); after a take-over the order is done, after an abort it stays
    /// aborted.
    pub(super) fn cleanup_next(&mut self, cx: &mut Cx, id: &OrderId, keep_branch: bool) {
        for role in crate::station::Role::ALL {
            self.end_session(cx, id, role);
        }
        let Some(order) = self.orders.get(id) else { return };
        if !order.checkpoints_removed() {
            let threads = order.checkpoint_threads();
            let op = order.next_op();
            let action = Action::RemoveCheckpoints {
                threads: threads.clone(),
            };
            self.begin(cx, id, action, Phase::Cleaning { op: op.clone() });
            cx.effect(Effect::RemoveCheckpoints {
                order: id.clone(),
                op,
                threads,
            });
            return;
        }
        if let Some(path) = order.worktree().map(str::to_string) {
            let op = order.next_op();
            let action = Action::RemoveWorktree {
                path: path.clone(),
                branch: id.branch(),
            };
            self.begin(cx, id, action, Phase::Cleaning { op: op.clone() });
            cx.effect(Effect::RemoveWorktree {
                order: id.clone(),
                op,
                path,
                branch: id.branch(),
                keep_branch,
            });
            return;
        }
        let finishes = order.condition() != &Condition::Aborted && order.station() == Station::TakeOver;
        self.set_phase(cx, id, Phase::Idle);
        if finishes {
            self.enter(cx, id, Station::Done);
            cx.effect(Effect::Notify {
                order: id.clone(),
                notice: Notice::Done,
            });
            self.wake_queue(cx);
        }
    }

    /// Lets the next order of each take-over queue go on.
    fn wake_queue(&mut self, cx: &mut Cx) {
        let ids: Vec<OrderId> = self
            .orders
            .values()
            .filter(|o| *o.phase() == Phase::Queue && *o.condition() == Condition::Running)
            .map(|o| o.id().clone())
            .collect();
        for id in ids {
            self.set_phase(cx, &id, Phase::Idle);
            self.advance(cx, &id);
        }
    }

    /// An operation finished.
    pub(super) fn done(&mut self, cx: &mut Cx, id: &OrderId, op: OperationId, result: OpResult) {
        let Some(order) = self.orders.get(id) else { return };
        let Some(action) = order.open_ops().get(&op).cloned() else { return };
        self.emit(cx, id, Event::Finished { op, result: result.clone() });
        if let OpResult::Failed { detail } = &result {
            let effect = action_name(&action).to_string();
            let detail = detail.clone();
            self.fail(cx, id, FailReason::Effect { effect, detail });
            return;
        }
        self.set_phase(cx, id, Phase::Idle);
        match (action, result) {
            (Action::PrepareWorktree { .. }, OpResult::Prepared { setup, .. }) => {
                let failed = setup.as_ref().and_then(|s| crate::gates::judge_command(s).err());
                match failed {
                    Some(failure) => self.fail(
                        cx,
                        id,
                        FailReason::Effect {
                            effect: "setup".into(),
                            detail: format!("{failure:?}"),
                        },
                    ),
                    None => self.advance(cx, id),
                }
            }
            (Action::Commit { .. }, OpResult::Committed { .. } | OpResult::NothingToCommit) => self.after_commit(cx, id),
            (Action::MergeTarget { .. }, result) => self.merged(cx, id, result),
            (Action::TakeOver { .. }, OpResult::TakenOver { .. }) => self.cleanup_next(cx, id, false),
            (Action::TakeOver { .. }, OpResult::Blocked { blocker }) => {
                self.wait(cx, id, WaitReason::TakeOverBlocked { blocker });
            }
            (Action::TakeOver { .. }, OpResult::TargetMoved) => {
                self.scratch(id).merged = false;
                self.scratch(id).manifest = None;
                self.take_over_next(cx, id);
            }
            (Action::RemoveCheckpoints { .. } | Action::RemoveWorktree { .. }, _) => {
                let aborted = self.orders.get(id).is_some_and(|o| *o.condition() == Condition::Aborted);
                self.cleanup_next(cx, id, false);
                if aborted {
                    self.set_phase(cx, id, Phase::Idle);
                }
            }
            _ => self.advance(cx, id),
        }
    }

    /// The target branch was merged in, or not: an unmoved target goes on, a moved one sends the order back to the
    /// review (G2 stands only for a byte-identical diff), a conflict back to the builder (C56).
    fn merged(&mut self, cx: &mut Cx, id: &OrderId, result: OpResult) {
        match result {
            OpResult::Merged { moved: false, .. } => {
                self.scratch(id).merged = true;
                self.take_over_next(cx, id);
            }
            OpResult::Merged {
                head, base, moved: true, ..
            } => {
                let Some((old_base, old_head)) = self.scratch(id).before_merge.take() else {
                    self.emit(cx, id, Event::Invalidated { why: Invalidation::Base });
                    self.enter(cx, id, Station::Review);
                    self.advance(cx, id);
                    return;
                };
                self.set_phase(cx, id, Phase::Comparing);
                cx.effect(Effect::CompareDiffs {
                    order: id.clone(),
                    old_base,
                    old_head,
                    new_base: base,
                    new_head: head,
                });
            }
            OpResult::Conflict { paths } => {
                let text = format!(
                    "Merging the target branch into the order's branch conflicts in:\n{}\nMerge it and resolve the \
                     conflicts.",
                    paths.iter().map(|p| format!("- {p}")).collect::<Vec<_>>().join("\n")
                );
                self.emit(cx, id, Event::Invalidated { why: Invalidation::Base });
                self.emit(cx, id, Event::Findings { text, round: false });
                self.enter(cx, id, Station::Build);
                self.advance(cx, id);
            }
            _ => self.take_over_next(cx, id),
        }
    }

    /// Whether the diff against the new base is byte for byte the old: then G2 stands, and either way gates and
    /// reviewer run again (C56).
    pub(super) fn compared(&mut self, cx: &mut Cx, id: &OrderId, identical: bool) {
        let Some(order) = self.orders.get(id) else { return };
        if *order.phase() != Phase::Comparing {
            return;
        }
        if identical && order.accepted().is_some() {
            self.emit(cx, id, Event::AcceptanceKept);
        }
        self.emit(cx, id, Event::Invalidated { why: Invalidation::Base });
        self.enter(cx, id, Station::Review);
        self.advance(cx, id);
    }

    /// Reverses the hunks the user rejected at G2 (C55).
    pub(super) fn reject_hunks(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        let Some(hunks) = self.rejected_hunks(order) else {
            self.set_phase(cx, id, Phase::Idle);
            self.advance(cx, id);
            return;
        };
        self.set_phase(cx, id, Phase::Rejecting);
        cx.effect(Effect::RejectHunks { order: id.clone(), hunks });
    }

    /// The hunks of the last partial acceptance, from the log.
    fn rejected_hunks(&self, order: &Order) -> Option<Vec<crate::ids::HunkId>> {
        order.rejected().filter(|h| !h.is_empty()).map(<[_]>::to_vec)
    }

    /// The rejected hunks are reversed: the new state is committed, its evidence is void, and the review runs again
    /// (C55).
    pub(super) fn rejected(&mut self, cx: &mut Cx, id: &OrderId, result: Result<(), String>) {
        if self.orders.get(id).map(Order::phase) != Some(&Phase::Rejecting) {
            return;
        }
        match result {
            Ok(()) => self.commit(cx, id, "Leave out the hunks rejected at acceptance".into()),
            Err(detail) => self.fail(
                cx,
                id,
                FailReason::Effect {
                    effect: "reject hunks".into(),
                    detail,
                },
            ),
        }
    }

    /// After a restart: an operation's target state holds (it happened) or not (it is done again).
    pub(super) fn verified(&mut self, cx: &mut Cx, id: &OrderId, op: OperationId, reached: Option<OpResult>) {
        let Some(order) = self.orders.get(id) else { return };
        let Some(action) = order.open_ops().get(&op).cloned() else { return };
        let running = *order.condition() == Condition::Running || *order.condition() == Condition::Aborted;
        match reached {
            Some(result) if running => self.done(cx, id, op, result),
            Some(result) => {
                self.emit(cx, id, Event::Finished { op, result });
                let next = if matches!(action, Action::Commit { .. }) { Phase::Refs } else { Phase::Idle };
                self.set_phase(cx, id, next);
            }
            None => {
                self.emit(cx, id, Event::Dropped { op });
                self.set_phase(cx, id, redo_phase(&action));
                if running {
                    self.redo(cx, id);
                }
            }
        }
    }
}

/// Where an order goes on when `action` did not happen: a commit is preceded by its secrets scan, everything else
/// follows from the station.
pub(super) fn redo_phase(action: &Action) -> Phase {
    if matches!(action, Action::Commit { .. }) {
        Phase::TurnScan
    } else {
        Phase::Idle
    }
}

/// The name of an action in a failure reason.
fn action_name(action: &Action) -> &'static str {
    match action {
        Action::PrepareWorktree { .. } => "prepare worktree",
        Action::Commit { .. } => "commit",
        Action::MergeTarget { .. } => "merge target",
        Action::TakeOver { .. } => "take over",
        Action::RemoveCheckpoints { .. } => "remove checkpoints",
        Action::RemoveWorktree { .. } => "remove worktree",
    }
}
