//! The preparations of order worktrees in flight, with what stops their setup command (E188, C67): an abort, a pause
//! (but the Mac's sleep, which suspends the command with everything else), the folder's last window closing and
//! quitting stop it. A stopped preparation reaches its factory as an operation whose target was not reached, so
//! resuming prepares the worktree again and runs the setup command anew.

use std::path::Path;
use std::rc::Rc;

use ide_core::Cancel;
use ide_factory::event::OpResult;
use ide_factory::{Condition, InputKind, OperationId, Order, OrderId, PauseReason};

use super::Leitstand;
use crate::context::AppContext;

/// What a preparation that ended tells its factory: what it achieved, or nothing when it was stopped.
pub(super) fn ended(order: OrderId, op: OperationId, result: OpResult, cancel: &Cancel) -> InputKind {
    if cancel.is_cancelled() {
        InputKind::Verified {
            order,
            op,
            reached: None,
        }
    } else {
        InputKind::Done { order, op, result }
    }
}

/// Whether the preparation of `order` stops, as the order no longer runs.
fn stops(order: Option<&Order>) -> bool {
    order.is_none_or(|o| o.finished() || !keeps_running(o.condition()))
}

/// Whether a preparation goes on under `condition`.
fn keeps_running(condition: &Condition) -> bool {
    matches!(condition, Condition::Running | Condition::Paused(PauseReason::Sleep))
}

impl Leitstand {
    /// Notes the preparation `op` of `order` as in flight, with what stops it.
    pub(super) fn setup_started(&self, root: &Path, order: &OrderId, op: &OperationId) -> Cancel {
        let cancel = Cancel::new();
        if let Some(folder) = self.folders.borrow_mut().get_mut(root) {
            folder.setups.insert(order.clone(), (op.clone(), cancel.clone()));
        }
        cancel
    }

    /// Forgets the preparation of `order`, which never started.
    pub(super) fn setup_gone(&self, root: &Path, order: &OrderId) {
        if let Some(folder) = self.folders.borrow_mut().get_mut(root) {
            folder.setups.remove(order);
        }
    }

    /// Whether the preparation `op` of `order` is still in flight; its own end answers for it then.
    pub(super) fn setup_runs(&self, root: &Path, order: &OrderId, op: &OperationId) -> bool {
        self.folders
            .borrow()
            .get(root)
            .and_then(|f| f.setups.get(order))
            .is_some_and(|(running, _)| running == op)
    }

    /// Stops the preparations of the orders of `root` that no longer run.
    pub(super) fn stop_setups(&self, root: &Path) {
        let folders = self.folders.borrow();
        let Some(folder) = folders.get(root) else { return };
        for (order, (_, cancel)) in &folder.setups {
            if stops(folder.factory.order(order)) {
                cancel.cancel();
            }
        }
    }

    /// The preparation `op` of `order` ended: it is no longer in flight, and its factory hears of it.
    pub(super) fn setup_ended(
        &self,
        cx: &Rc<AppContext>,
        root: &Path,
        order: &OrderId,
        op: &OperationId,
        input: InputKind,
    ) {
        if self.setup_runs(root, order, op) {
            self.setup_gone(root, order);
        }
        self.input(cx, root, input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stopped_preparation_reaches_nothing_and_one_that_ran_its_result() {
        let (order, op) = (OrderId("o-1".into()), OperationId("op-1".into()));
        let cancel = Cancel::new();
        let ran = ended(order.clone(), op.clone(), OpResult::Removed, &cancel);
        assert!(matches!(
            ran,
            InputKind::Done {
                result: OpResult::Removed,
                ..
            }
        ));
        cancel.cancel();
        let stopped = ended(order, op, OpResult::Removed, &cancel);
        assert!(matches!(stopped, InputKind::Verified { reached: None, .. }));
    }

    #[test]
    fn a_preparation_goes_on_while_its_order_runs_or_the_mac_sleeps() {
        assert!(keeps_running(&Condition::Running));
        assert!(keeps_running(&Condition::Paused(PauseReason::Sleep)));
        for stopped in [
            Condition::Aborted,
            Condition::Paused(PauseReason::NoWindow),
            Condition::Paused(PauseReason::AppEnded),
            Condition::Paused(PauseReason::Quota),
        ] {
            assert!(!keeps_running(&stopped), "{stopped:?}");
        }
        assert!(stops(None));
    }
}
