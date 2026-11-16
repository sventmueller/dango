//! Restoring the factory from its order logs after a restart (E167): every order comes back as its events say, a
//! running one paused with "app ended", and every operation begun without its end is verified before anything is
//! done again (C66).

use super::{Cx, Factory, Scratch, Step};
use crate::effect::Effect;
use crate::event::Phase;
use crate::ids::OrderId;
use crate::log::read;
use crate::order::Order;
use crate::station::{Condition, PauseReason, Stage};
use crate::time::Stamp;
use crate::workflow::rules::Rules;

/// What went wrong with one log while restoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreProblem {
    /// The log's index in the list handed in.
    pub log: usize,
    /// The order, when its first entry could be read.
    pub order: Option<OrderId>,
    /// The last line was cut and dropped.
    pub cut: bool,
    /// Reading stopped at this line, from one.
    pub stopped_at: Option<usize>,
    /// The log does not begin with the order's creation, so nothing of it could be restored.
    pub unreadable: bool,
}

/// The factory restored, the step its reconciliation leads to, and what went wrong.
#[derive(Debug, Clone)]
pub struct Restored {
    /// The factory.
    pub factory: Factory,
    /// Log entries and effects: "app ended" for orders that ran, and checks of open operations.
    pub step: Step,
    /// Problems per log.
    pub problems: Vec<RestoreProblem>,
}

impl Factory {
    /// Restores a factory from the text of each order log at `now`.
    pub fn restore(stage: Stage, rules: Rules, logs: &[String], now: Stamp) -> Restored {
        let mut factory = Factory::new(stage, rules);
        factory.now = now;
        let mut problems = Vec::new();
        for (index, text) in logs.iter().enumerate() {
            let read = read(text);
            let mut entries = read.entries.into_iter();
            let order = entries
                .next()
                .and_then(|first| Order::create(first.order.clone(), first.at, &first.event).map(|o| (o, first)));
            let id = order.as_ref().map(|(o, _)| o.id().clone());
            if read.cut || read.stopped_at.is_some() || order.is_none() {
                problems.push(RestoreProblem {
                    log: index,
                    order: id.clone(),
                    cut: read.cut,
                    stopped_at: read.stopped_at,
                    unreadable: order.is_none(),
                });
            }
            let Some((mut order, first)) = order else { continue };
            order.apply(&first);
            for entry in entries.filter(|e| e.order == first.order) {
                order.apply(&entry);
            }
            order.forget_processes();
            factory.scratch.insert(order.id().clone(), Scratch::default());
            factory.orders.insert(order.id().clone(), order);
        }
        let mut cx = Cx {
            now,
            step: Step::default(),
        };
        factory.reconcile(&mut cx);
        factory.update_lock(&mut cx);
        Restored {
            factory,
            step: cx.step,
            problems,
        }
    }

    /// Pauses what ran, verifies open operations, and resumes by itself only where the user's setting says so.
    fn reconcile(&mut self, cx: &mut Cx) {
        let ids: Vec<OrderId> = self.orders.keys().cloned().collect();
        for id in &ids {
            let Some(order) = self.orders.get(id) else { continue };
            if order.finished() && order.open_ops().is_empty() && order.worktree().is_none() {
                continue;
            }
            if *order.condition() == Condition::Running && !order.finished() {
                self.pause(cx, id, PauseReason::AppEnded);
            }
            let Some(order) = self.orders.get(id) else { continue };
            let resumes = self.rules.resume_after_restart() && *order.condition() == Condition::Paused(PauseReason::AppEnded);
            if resumes {
                continue;
            }
            let open: Vec<_> = order.open_ops().iter().map(|(op, a)| (op.clone(), a.clone())).collect();
            for (op, action) in open {
                let target = action.target(&op);
                self.set_phase(cx, id, Phase::Verifying { op: op.clone() });
                cx.effect(Effect::Verify {
                    order: id.clone(),
                    op,
                    action,
                    target,
                });
            }
            let aborted_leftover = self
                .orders
                .get(id)
                .is_some_and(|o| *o.condition() == Condition::Aborted && o.open_ops().is_empty() && o.worktree().is_some());
            if aborted_leftover {
                self.cleanup_next(cx, id, false);
            }
        }
        if self.rules.resume_after_restart() {
            for id in ids {
                let paused = self
                    .orders
                    .get(&id)
                    .is_some_and(|o| *o.condition() == Condition::Paused(PauseReason::AppEnded));
                if paused {
                    self.resume(cx, &id);
                }
            }
        }
    }
}
