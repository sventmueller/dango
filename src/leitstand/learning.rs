//! What the Leitstand learns from a folder's orders (B47): the figures of every order log (E180), computed on a lane
//! whenever an order reaches its acceptance, ends or fails, and handed to the factory for routing's tie-break (E170);
//! a scout's rule proposal after an order ended escalated or failed, as an order of its own that waits at G1; and the
//! recurring orders of the workflow file, created only while the app runs.

use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use ide_factory::event::Event;
use ide_factory::learning::{proposal_due, proposal_order};
use ide_factory::log::LogEntry;
use ide_factory::metrics::Metrics;
use ide_factory::workflow::recurring::{RecurringOrder, Schedule};
use ide_factory::{Condition, InputKind, Order, OrderId, Station, WaitReason};
use splat_i18n::{t, tf};

use super::{Delivery, Leitstand, store};
use crate::context::AppContext;

/// How often the recurring orders are looked at while the app runs.
const RECURRING_EVERY: Duration = Duration::from_secs(5 * 60);

/// What the Leitstand learned from one folder's orders.
pub(super) struct Learned {
    /// The figures as last computed; none before the first time.
    metrics: Option<Metrics>,
    /// When each recurring order last came.
    schedule: Schedule,
    /// The last rule proposal asked for, which may not have reached the factory yet.
    proposed: Option<OrderId>,
}

impl Learned {
    pub(super) fn new(schedule: Schedule) -> Learned {
        Learned {
            metrics: None,
            schedule,
            proposed: None,
        }
    }
}

/// What the log of one step says ended.
#[derive(Default)]
pub(super) struct Ended {
    /// An order reached its acceptance, ended or failed, so the figures change.
    figures: bool,
    /// The orders that failed or ended escalated.
    escalated: Vec<OrderId>,
}

/// What `log` says ended.
pub(super) fn ended(log: &[LogEntry]) -> Ended {
    let mut ended = Ended::default();
    for entry in log {
        match &entry.event {
            Event::Entered {
                station: Station::Acceptance | Station::Done,
            }
            | Event::Condition {
                condition: Condition::Aborted,
            } => ended.figures = true,
            Event::Condition {
                condition:
                    Condition::Failed(_) | Condition::Waiting(WaitReason::ReviewRoundsSpent | WaitReason::Unclear),
            } => {
                ended.figures = true;
                if !ended.escalated.contains(&entry.order) {
                    ended.escalated.push(entry.order.clone());
                }
            }
            _ => {}
        }
    }
    ended
}

impl Leitstand {
    /// The figures of `root`'s orders as last computed (E180).
    pub(crate) fn metrics(&self, root: &Path) -> Option<Metrics> {
        self.folders.borrow().get(root)?.learned.metrics.clone()
    }

    /// Takes in what a step of `root` ended: new figures, and a rule proposal after an escalation.
    pub(super) fn learn(&self, root: &Path, ended: &Ended) {
        if ended.figures {
            self.refresh_metrics(root);
        }
        for order in &ended.escalated {
            self.propose_rules(root, order);
        }
    }

    /// Computes the figures of every order of `root` on a lane, from the orders its logs made.
    pub(super) fn refresh_metrics(&self, root: &Path) {
        let orders: Option<Vec<Order>> = self
            .folders
            .borrow()
            .get(root)
            .map(|f| f.factory.orders().cloned().collect());
        let Some(orders) = orders else { return };
        let now = self.clock.now().wall;
        let (root, back) = (root.to_path_buf(), self.back());
        let queued = ide_core::lane::WORK.try_run(move || {
            let metrics = Metrics::new(&orders, now);
            back.deliver(Delivery::Metrics(root, Box::new(metrics)));
        });
        if queued.is_err() {
            ide_core::diag::report("computing the figures of orders", "the lane took no more work");
        }
    }

    /// The figures of `root` arrived: kept for the Leitstand tab and handed to the factory for routing.
    pub(super) fn metrics_arrived(&self, cx: &Rc<AppContext>, root: &Path, metrics: Metrics) {
        {
            let mut folders = self.folders.borrow_mut();
            let Some(folder) = folders.get_mut(root) else { return };
            folder.learned.metrics = Some(metrics.clone());
        }
        self.input(cx, root, InputKind::Metrics(Box::new(metrics)));
    }

    /// After `ended` ended escalated or failed, a scout proposes a rule change as an order of its own, if the setting
    /// says so and the folder's findings recur (B47).
    fn propose_rules(&self, root: &Path, ended: &OrderId) {
        let order = {
            let folders = self.folders.borrow();
            let Some(folder) = folders.get(root) else { return };
            let factory = &folder.factory;
            if !factory.rules().rule_proposals() || !factory.errors().is_empty() {
                return;
            }
            let in_flight = folder.learned.proposed.as_ref().is_some_and(|id| {
                factory.order(id).is_none() && !folder.refusals.iter().any(|r| r.order.as_ref() == Some(id))
            });
            if in_flight {
                return;
            }
            let Some(found) = factory.order(ended).and_then(|o| proposal_due(factory.orders(), o)) else {
                return;
            };
            let title = match found.len() {
                1 => t("Regelvorschlag: 1 wiederkehrender Befund").to_string(),
                n => tf("Regelvorschlag: {} wiederkehrende Befunde", &[&n.to_string()]),
            };
            let intro = t(
                "Diese Befunde kamen in mehreren Aufträgen dieses Ordners wieder. Schlage eine Änderung an AGENTS.md oder CLAUDE.md vor, die sie künftig verhindert: den genauen Text, wohin er gehört und warum. Ändere nichts; die Änderung kommt erst nach Freigabe und Abnahme.",
            );
            proposal_order(&title, intro, &found)
        };
        let id = self.new_order(root, order);
        if let Some(folder) = self.folders.borrow_mut().get_mut(root) {
            folder.learned.proposed = Some(id);
        }
    }

    /// Looks for due recurring orders every few minutes from now on, once.
    pub(super) fn keep_recurring(&self) {
        if std::mem::replace(&mut self.shared.borrow_mut().recurring, true) {
            return;
        }
        self.back().deliver_after(RECURRING_EVERY, Delivery::Recurring);
    }

    /// Creates the recurring orders that are due in every folder, then looks again in a few minutes.
    pub(super) fn recurring_due(&self) {
        if self.shared.borrow().quitting {
            return;
        }
        for root in self.roots() {
            self.create_recurring(&root);
        }
        self.back().deliver_after(RECURRING_EVERY, Delivery::Recurring);
    }

    /// Creates the recurring orders of `root` that are due, each through the normal flow, and keeps their dates.
    fn create_recurring(&self, root: &Path) {
        let now = self.clock.now().wall;
        let due: Vec<RecurringOrder> = {
            let mut folders = self.folders.borrow_mut();
            let Some(folder) = folders.get_mut(root) else { return };
            if !folder.factory.errors().is_empty() {
                return;
            }
            let list = folder.factory.rules().recurring().to_vec();
            let (factory, schedule) = (&folder.factory, &mut folder.learned.schedule);
            let before = schedule.clone();
            let open = |id: &OrderId| factory.order(id).is_some_and(|o| !o.finished());
            let due: Vec<RecurringOrder> = schedule.due(&list, now, open).into_iter().cloned().collect();
            if due.is_empty() && *schedule == before {
                return;
            }
            due
        };
        for recurring in &due {
            let id = self.new_order(root, recurring.order());
            if let Some(folder) = self.folders.borrow_mut().get_mut(root) {
                folder.learned.schedule.created(recurring.title(), id);
            }
        }
        let Some((project, schedule)) = self
            .folders
            .borrow()
            .get(root)
            .map(|f| (f.project.clone(), f.learned.schedule.clone()))
        else {
            return;
        };
        let queued = ide_core::lane::DISK.try_run(move || store::save_schedule(&project, &schedule));
        if queued.is_err() {
            ide_core::diag::report(
                "keeping the dates of recurring orders",
                "the disk lane took no more work",
            );
        }
    }
}
