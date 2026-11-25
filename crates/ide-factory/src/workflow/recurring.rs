//! Recurring cleanup orders (B47): the workflow file names them under `[[recurring]]` with title, goal, criteria and
//! an interval. Dango creates them only while it runs, there is no service in the background, each through the
//! normal flow; after a pause at most one missed date is caught up, and the dates keep their rhythm.
//!
//! ```toml
//! [[recurring]]
//! title = "Remove dead code"
//! goal = "Find code nothing calls any more and remove it."
//! criteria = ["make check passes", "no public item is removed"]
//! interval = "weekly"   # "daily", "weekly" or a number of days such as "14d"
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use toml_edit::{Item, TableLike};

use super::WorkflowErrorKind;
use super::read::{Reader, join};
use crate::ids::OrderId;
use crate::options::NewOrder;

/// One day in milliseconds.
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// The longest interval in days.
const MAX_DAYS: u32 = 366;

/// How often a recurring order comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "every", content = "days")]
pub enum Interval {
    /// Every day.
    Daily,
    /// Every seven days.
    Weekly,
    /// Every so many days.
    Days(u32),
}

impl Interval {
    /// The interval `word` names: `daily`, `weekly` or a number of days followed by `d`, from 1 to 366.
    pub fn parse(word: &str) -> Option<Interval> {
        match word.trim() {
            "daily" => Some(Interval::Daily),
            "weekly" => Some(Interval::Weekly),
            other => other
                .strip_suffix('d')
                .and_then(|n| n.trim().parse::<u32>().ok())
                .filter(|n| (1..=MAX_DAYS).contains(n))
                .map(Interval::Days),
        }
    }

    /// The days between two dates.
    pub fn days(self) -> u32 {
        match self {
            Interval::Daily => 1,
            Interval::Weekly => 7,
            Interval::Days(n) => n,
        }
    }

    /// The time between two dates in milliseconds.
    pub fn period_ms(self) -> u64 {
        u64::from(self.days()) * DAY_MS
    }
}

/// A recurring order of the workflow file; its title tells it apart from the others.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecurringOrder {
    title: String,
    goal: String,
    criteria: Vec<String>,
    interval: Interval,
}

impl RecurringOrder {
    /// A recurring order of these parts.
    pub fn new(title: &str, goal: &str, criteria: &[&str], interval: Interval) -> RecurringOrder {
        RecurringOrder {
            title: title.to_string(),
            goal: goal.to_string(),
            criteria: criteria.iter().map(|c| c.to_string()).collect(),
            interval,
        }
    }

    /// The title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The goal.
    pub fn goal(&self) -> &str {
        &self.goal
    }

    /// The criteria.
    pub fn criteria(&self) -> &[String] {
        &self.criteria
    }

    /// How often it comes.
    pub fn interval(&self) -> Interval {
        self.interval
    }

    /// The sheet of the order it creates, which goes through the normal flow and its G1 (E185).
    pub fn order(&self) -> NewOrder {
        NewOrder {
            title: self.title.clone(),
            goal: self.goal.clone(),
            criteria: self.criteria.join("\n"),
            ..NewOrder::default()
        }
    }
}

/// Reads `[[recurring]]`; every error names its key and place (E141).
pub(super) fn read(r: &mut Reader<'_>, top: &dyn TableLike) -> Vec<RecurringOrder> {
    let path = "recurring";
    let mut out: Vec<RecurringOrder> = Vec::new();
    for item in r.tables(top, "", path) {
        let Some(table) = item.as_table_like() else { continue };
        r.known(table, path, &["title", "goal", "criteria", "interval"]);
        let missing = |r: &mut Reader<'_>, key: &str| {
            r.error(&join(path, key), item.span(), WorkflowErrorKind::Missing);
        };
        let title = r.text(table, path, "title").filter(|t| !t.trim().is_empty());
        let goal = r.text(table, path, "goal").filter(|g| !g.trim().is_empty());
        let criteria: Vec<String> = r
            .texts(table, path, "criteria")
            .into_iter()
            .filter(|c| !c.trim().is_empty())
            .collect();
        let word = r.text(table, path, "interval");
        let interval = word.as_deref().and_then(Interval::parse);
        if title.is_none() {
            missing(r, "title");
        }
        if goal.is_none() {
            missing(r, "goal");
        }
        if criteria.is_empty() && table.get("criteria").is_none_or(|c| c.as_array().is_some()) {
            missing(r, "criteria");
        }
        match (&word, interval) {
            (None, _) if table.get("interval").is_none() => missing(r, "interval"),
            (Some(word), None) => {
                let span = table.get("interval").and_then(Item::span);
                r.error(
                    &join(path, "interval"),
                    span,
                    WorkflowErrorKind::BadInterval { value: word.clone() },
                );
            }
            _ => {}
        }
        let (Some(title), Some(goal), Some(interval)) = (title, goal, interval) else {
            continue;
        };
        if criteria.is_empty() {
            continue;
        }
        if out.iter().any(|o| o.title == title) {
            r.error(
                &join(path, "title"),
                item.span(),
                WorkflowErrorKind::DuplicateRecurring { title },
            );
            continue;
        }
        out.push(RecurringOrder {
            title,
            goal,
            criteria,
            interval,
        });
    }
    out
}

/// When each recurring order of a folder last came and with which order, kept by the app on this Mac.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    slots: BTreeMap<String, Slot>,
}

/// The last date of one recurring order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Slot {
    /// The date, wall clock in milliseconds; the next is one interval later.
    due: u64,
    /// The order created for it.
    order: Option<OrderId>,
}

impl Schedule {
    /// The orders of `list` due at wall-clock `now`, each once: one never seen before is due at once; after a pause
    /// that missed several dates only the latest of them comes, and the date moves on by whole intervals. A date
    /// whose previous order is still `open` passes without a new one. Titles no longer in `list` are forgotten.
    pub fn due<'a>(
        &mut self,
        list: &'a [RecurringOrder],
        now: u64,
        open: impl Fn(&OrderId) -> bool,
    ) -> Vec<&'a RecurringOrder> {
        self.slots.retain(|title, _| list.iter().any(|r| r.title == *title));
        let mut due = Vec::new();
        for recurring in list {
            let Some(slot) = self.slots.get_mut(&recurring.title) else {
                self.slots
                    .insert(recurring.title.clone(), Slot { due: now, order: None });
                due.push(recurring);
                continue;
            };
            let period = recurring.interval.period_ms();
            let Some(passed) = now.checked_sub(slot.due).map(|ms| ms / period).filter(|n| *n > 0) else {
                continue;
            };
            slot.due += passed * period;
            if slot.order.as_ref().is_some_and(&open) {
                continue;
            }
            slot.order = None;
            due.push(recurring);
        }
        due
    }

    /// Notes that `order` came for the recurring order titled `title`.
    pub fn created(&mut self, title: &str, order: OrderId) {
        if let Some(slot) = self.slots.get_mut(title) {
            slot.order = Some(order);
        }
    }

    /// When the next of `list` is due, wall clock in milliseconds; `now` for one never seen.
    pub fn next_due(&self, list: &[RecurringOrder], now: u64) -> Option<u64> {
        list.iter()
            .map(|r| {
                self.slots
                    .get(&r.title)
                    .map_or(now, |slot| slot.due + r.interval.period_ms())
            })
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: u64 = 1_700_000_000_000;

    fn weekly() -> RecurringOrder {
        RecurringOrder::new(
            "Dead code",
            "Remove dead code.",
            &["make check passes"],
            Interval::Weekly,
        )
    }

    fn titles(due: &[&RecurringOrder]) -> Vec<String> {
        due.iter().map(|r| r.title().to_string()).collect()
    }

    #[test]
    fn intervals_read_as_words_or_days() {
        assert_eq!(Interval::parse("daily"), Some(Interval::Daily));
        assert_eq!(Interval::parse("weekly"), Some(Interval::Weekly));
        assert_eq!(Interval::parse("14d"), Some(Interval::Days(14)));
        for bad in ["0d", "367d", "fortnightly", "d", "7"] {
            assert_eq!(Interval::parse(bad), None, "{bad}");
        }
        assert_eq!(Interval::Weekly.period_ms(), 7 * DAY_MS);
    }

    #[test]
    fn a_new_recurring_order_comes_at_once_and_then_once_per_interval() {
        let list = [weekly()];
        let mut schedule = Schedule::default();
        assert_eq!(titles(&schedule.due(&list, START, |_| false)), ["Dead code"]);
        schedule.created("Dead code", OrderId("o-1".into()));
        assert!(schedule.due(&list, START + DAY_MS, |_| false).is_empty());
        assert!(schedule.due(&list, START + 7 * DAY_MS - 1, |_| false).is_empty());
        assert_eq!(schedule.due(&list, START + 7 * DAY_MS, |_| false).len(), 1);
        assert!(schedule.due(&list, START + 7 * DAY_MS + 1, |_| false).is_empty());
        assert_eq!(schedule.next_due(&list, START), Some(START + 14 * DAY_MS));
    }

    #[test]
    fn after_a_pause_only_one_missed_date_is_caught_up_and_the_rhythm_stays() {
        let list = [weekly()];
        let mut schedule = Schedule::default();
        schedule.due(&list, START, |_| false);
        let back = START + 23 * DAY_MS;
        assert_eq!(
            schedule.due(&list, back, |_| false).len(),
            1,
            "three dates missed, one comes"
        );
        assert!(schedule.due(&list, back + DAY_MS, |_| false).is_empty());
        assert_eq!(
            schedule.next_due(&list, back),
            Some(START + 28 * DAY_MS),
            "the next date keeps the rhythm of the first"
        );
    }

    #[test]
    fn a_date_whose_last_order_still_runs_passes_without_a_new_one() {
        let list = [weekly()];
        let mut schedule = Schedule::default();
        schedule.due(&list, START, |_| false);
        schedule.created("Dead code", OrderId("o-1".into()));
        let open = |id: &OrderId| id.0 == "o-1";
        assert!(schedule.due(&list, START + 7 * DAY_MS, open).is_empty());
        assert_eq!(schedule.due(&list, START + 14 * DAY_MS, |_| false).len(), 1);
    }

    #[test]
    fn a_clock_set_back_and_a_removed_title_create_nothing() {
        let list = [weekly()];
        let mut schedule = Schedule::default();
        schedule.due(&list, START, |_| false);
        assert!(schedule.due(&list, START - DAY_MS, |_| false).is_empty());
        assert!(schedule.due(&[], START + 30 * DAY_MS, |_| false).is_empty());
        assert_eq!(schedule, Schedule::default());
    }
}
