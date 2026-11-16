//! What the panel "Leitstand" and the Leitstand tab show (E175), without AppKit (E98): the panel's list grouped by
//! station with its red dot, the station strip with counts that filters the sortable table, and the fleet line.

use serde::Serialize;

use crate::Factory;
use crate::budget::Spend;
use crate::event::Phase;
use crate::order::Order;
use crate::quota::QuotaBook;
use crate::station::{Condition, Station};
use crate::time::Stamp;

/// Why an order that runs makes no progress for now, which the views show beside its condition (E168, E174, R68).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "hold")]
pub enum Hold {
    /// As many orders run as may run at once.
    Slot {
        /// How many may.
        limit: u32,
    },
    /// An order whose path scope may touch the same files builds, is reviewed or is taken over (E174).
    Overlap {
        /// That order's title.
        title: String,
    },
    /// Another order is taken over into the same branch first (E174).
    Queue {
        /// The target branch.
        branch: String,
    },
}

impl Hold {
    /// Why `order` of `factory` holds, if it runs and does.
    pub fn of(factory: &Factory, order: &Order) -> Option<Hold> {
        if *order.condition() != Condition::Running || order.finished() {
            return None;
        }
        match order.phase() {
            Phase::Queue => Some(Hold::Queue {
                branch: order.base().branch.clone(),
            }),
            Phase::Slot => match factory.overlapping(order) {
                Some(other) => Some(Hold::Overlap {
                    title: other.title().to_string(),
                }),
                None => Some(Hold::Slot {
                    limit: factory.parallel_limit(),
                }),
            },
            _ => None,
        }
    }
}

/// One order as a row of the panel or the table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    id: crate::OrderId,
    title: String,
    station: Station,
    condition: Condition,
    agent: Option<String>,
    model: Option<String>,
    runtime_ms: u64,
    round: u32,
    spend: Spend,
    waiting_since: Option<u64>,
    held: Option<Hold>,
}

impl Row {
    fn of(factory: &Factory, order: &Order, now: Stamp) -> Row {
        let current = order.current();
        Row {
            held: Hold::of(factory, order),
            id: order.id().clone(),
            title: order.title().to_string(),
            station: order.station(),
            condition: order.condition().clone(),
            agent: current.map(|s| s.entry().agent.to_string()),
            model: current.and_then(|s| s.entry().model.clone()),
            runtime_ms: now.wall.saturating_sub(order.created().wall),
            round: order.review_rounds() + u32::from(order.station() >= Station::Review),
            spend: *order.spend(),
            waiting_since: order.waiting_since().map(|s| s.wall),
        }
    }

    /// The order.
    pub fn id(&self) -> &crate::OrderId {
        &self.id
    }

    /// Its title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Its station.
    pub fn station(&self) -> Station {
        self.station
    }

    /// Its condition, shown as shape and text, not only colour (E101).
    pub fn condition(&self) -> &Condition {
        &self.condition
    }

    /// The agent working on it now or last.
    pub fn agent(&self) -> Option<&str> {
        self.agent.as_deref()
    }

    /// The model, when not the agent's default.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// How long it has existed.
    pub fn runtime_ms(&self) -> u64 {
        self.runtime_ms
    }

    /// The review round it is in, from one; zero before the first review.
    pub fn round(&self) -> u32 {
        self.round
    }

    /// What it spent.
    pub fn spend(&self) -> &Spend {
        &self.spend
    }

    /// Since when it waits for the user, wall clock.
    pub fn waiting_since(&self) -> Option<u64> {
        self.waiting_since
    }

    /// Why it makes no progress although it runs.
    pub fn held(&self) -> Option<&Hold> {
        self.held.as_ref()
    }

    /// Whether it waits for the user (red dot, E71).
    pub fn waits(&self) -> bool {
        self.condition.waits()
    }
}

/// The panel's list: orders grouped by station (E175).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Panel {
    groups: Vec<(Station, Vec<Row>)>,
}

impl Panel {
    /// The panel for `factory`'s orders at `now`; finished and aborted orders are left out.
    pub fn new(factory: &Factory, now: Stamp) -> Panel {
        let mut groups: Vec<(Station, Vec<Row>)> = Vec::new();
        for station in Station::ALL {
            let mut rows: Vec<Row> = factory
                .orders()
                .filter(|o| o.station() == station && !o.finished())
                .map(|o| Row::of(factory, o, now))
                .collect();
            rows.sort_by_key(|r| std::cmp::Reverse(r.runtime_ms));
            if !rows.is_empty() {
                groups.push((station, rows));
            }
        }
        Panel { groups }
    }

    /// The groups, in the stations' order.
    pub fn groups(&self) -> &[(Station, Vec<Row>)] {
        &self.groups
    }

    /// How many orders wait for the user, for the symbol's tooltip and VoiceOver text ("1 Auftrag wartet").
    pub fn waiting(&self) -> usize {
        self.groups
            .iter()
            .flat_map(|(_, rows)| rows)
            .filter(|r| r.waits())
            .count()
    }

    /// Whether the red dot shows.
    pub fn dot(&self) -> bool {
        self.waiting() > 0
    }
}

/// A column of the Leitstand table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Column {
    /// Title.
    Title,
    /// Station.
    Station,
    /// Agent.
    Agent,
    /// Review round.
    Round,
    /// Spending, in dollars, then tokens.
    Spend,
    /// Waiting since; ascending puts the longest wait first.
    WaitingSince,
}

/// The Leitstand tab's station strip and table (E175).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Board {
    rows: Vec<Row>,
    filter: Option<Station>,
    sort: Column,
    ascending: bool,
}

impl Board {
    /// The board for `factory`'s orders at `now`, sorted by "waiting since", longest wait first.
    pub fn new(factory: &Factory, now: Stamp) -> Board {
        let mut board = Board {
            rows: factory.orders().map(|o| Row::of(factory, o, now)).collect(),
            filter: None,
            sort: Column::WaitingSince,
            ascending: true,
        };
        board.sort_rows();
        board
    }

    /// The strip: each station with how many orders stand there.
    pub fn strip(&self) -> Vec<(Station, usize)> {
        Station::ALL
            .into_iter()
            .map(|station| (station, self.rows.iter().filter(|r| r.station == station).count()))
            .collect()
    }

    /// Filters the table by `station`, or clears the filter; a click on the active station clears it.
    pub fn filter(&mut self, station: Option<Station>) {
        self.filter = if station == self.filter { None } else { station };
    }

    /// The active filter.
    pub fn active_filter(&self) -> Option<Station> {
        self.filter
    }

    /// Sorts by `column`; sorting by the same column again turns the direction.
    pub fn sort_by(&mut self, column: Column) {
        if self.sort == column {
            self.ascending = !self.ascending;
        } else {
            self.sort = column;
            self.ascending = true;
        }
        self.sort_rows();
    }

    /// The column and direction sorted by.
    pub fn sorting(&self) -> (Column, bool) {
        (self.sort, self.ascending)
    }

    fn sort_rows(&mut self) {
        let column = self.sort;
        self.rows.sort_by(|a, b| {
            let order = match column {
                Column::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                Column::Station => a.station.cmp(&b.station),
                Column::Agent => a.agent.cmp(&b.agent),
                Column::Round => a.round.cmp(&b.round),
                Column::Spend => a
                    .spend
                    .dollars()
                    .total_cmp(&b.spend.dollars())
                    .then(a.spend.tokens().cmp(&b.spend.tokens())),
                Column::WaitingSince => match (a.waiting_since, b.waiting_since) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                },
            };
            order.then_with(|| a.id.cmp(&b.id))
        });
        if !self.ascending {
            self.rows.reverse();
        }
    }

    /// The rows the filter lets through, in sorted order.
    pub fn rows(&self) -> Vec<&Row> {
        self.rows
            .iter()
            .filter(|r| self.filter.is_none_or(|station| r.station == station))
            .collect()
    }
}

/// One quota in the fleet line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QuotaLine {
    account: String,
    service: String,
    window: String,
    utilization: Option<f64>,
    observed_at: u64,
    current: bool,
}

impl QuotaLine {
    /// The account.
    pub fn account(&self) -> &str {
        &self.account
    }

    /// The billing service.
    pub fn service(&self) -> &str {
        &self.service
    }

    /// The window.
    pub fn window(&self) -> &str {
        &self.window
    }

    /// The share used, from 0 to 1.
    pub fn utilization(&self) -> Option<f64> {
        self.utilization
    }

    /// When it was observed, wall clock (C65).
    pub fn observed_at(&self) -> u64 {
        self.observed_at
    }

    /// Whether it is still current; a stale one shows as unknown.
    pub fn current(&self) -> bool {
        self.current
    }
}

/// The fleet line above the table (E175).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fleet {
    running: usize,
    waiting: usize,
    failed: usize,
    parallel: u32,
    quotas: Vec<QuotaLine>,
    day: String,
    day_spent: Spend,
    day_budget: crate::budget::Budget,
    done_today: usize,
    median_wait_ms: Option<u64>,
    worktree_bytes: u64,
}

impl Fleet {
    /// The fleet line of `factory` at `now`; `day_start` is the wall clock of the day's start, `quotas` the app's
    /// book (it lives in the app's state, C65), and `worktree_bytes` the space the order worktrees take (R68).
    pub fn new(factory: &Factory, quotas: &QuotaBook, now: Stamp, day_start: u64, worktree_bytes: u64) -> Fleet {
        let orders: Vec<&Order> = factory.orders().collect();
        let count = |f: &dyn Fn(&Order) -> bool| orders.iter().filter(|o| f(o)).count();
        let running =
            count(&|o| !o.finished() && o.station() != Station::Intake && *o.condition() == Condition::Running);
        let waiting = count(&|o| !o.finished() && o.condition().waits());
        let failed = count(&|o| matches!(o.condition(), Condition::Failed(_)));
        let done_today =
            count(&|o| o.station() == Station::Done && o.visits().last().is_some_and(|v| v.at.wall >= day_start));
        let mut waits: Vec<u64> = orders
            .iter()
            .map(|o| o.waited_ms(now.wall))
            .filter(|ms| *ms > 0)
            .collect();
        waits.sort_unstable();
        let median_wait_ms = (!waits.is_empty()).then(|| waits[waits.len() / 2]);
        let quotas = quotas
            .observations()
            .map(|o| QuotaLine {
                account: o.key.account.clone(),
                service: o.key.service.clone(),
                window: o.key.window.clone(),
                utilization: o.utilization,
                observed_at: o.observed_at,
                current: QuotaBook::current(o, now.wall),
            })
            .collect();
        Fleet {
            running,
            waiting,
            failed,
            parallel: factory.parallel_limit(),
            quotas,
            day: factory.day().day().to_string(),
            day_spent: if factory.day().current(now.wall) {
                *factory.day().total()
            } else {
                Spend::default()
            },
            day_budget: factory.rules().budgets().day().clone(),
            done_today,
            median_wait_ms,
            worktree_bytes,
        }
    }

    /// Orders that run.
    pub fn running(&self) -> usize {
        self.running
    }

    /// Orders that wait for the user.
    pub fn waiting(&self) -> usize {
        self.waiting
    }

    /// Orders whose station failed.
    pub fn failed(&self) -> usize {
        self.failed
    }

    /// How many orders may run at once.
    pub fn parallel(&self) -> u32 {
        self.parallel
    }

    /// Use per account and service against its window.
    pub fn quotas(&self) -> &[QuotaLine] {
        &self.quotas
    }

    /// The local day the daily account counts, such as `2026-11-16`.
    pub fn day(&self) -> &str {
        &self.day
    }

    /// What the day spent across projects.
    pub fn day_spent(&self) -> &Spend {
        &self.day_spent
    }

    /// The day's budget.
    pub fn day_budget(&self) -> &crate::budget::Budget {
        &self.day_budget
    }

    /// Orders done today.
    pub fn done_today(&self) -> usize {
        self.done_today
    }

    /// The median of the orders' waits for the user.
    pub fn median_wait_ms(&self) -> Option<u64> {
        self.median_wait_ms
    }

    /// The space the worktrees take.
    pub fn worktree_bytes(&self) -> u64 {
        self.worktree_bytes
    }
}
