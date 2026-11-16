//! Figures from the order logs (E180): first-pass rate, review rounds, cost and runtime per role, agent, model and
//! risk class, and the wait for the user. Orders with interventions, a switch of model, work by hand or a change of
//! agent do not count towards the first-pass rate, which would otherwise measure the user or teach routing the wrong
//! thing.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::budget::Spend;
use crate::evidence::Deviation;
use crate::order::Order;
use crate::routing::Agent;
use crate::station::{RiskClass, Role, Station};

/// How many orders a first-pass rate must rest on before routing's tie-break uses it, and how many orders a folder
/// needs before a scout proposes rules (B47).
pub const MIN_ORDERS: u32 = 5;

/// What the figures are grouped by.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Key {
    role: Role,
    agent: Agent,
    model: Option<String>,
    risk: RiskClass,
}

impl Key {
    /// A key of these parts.
    pub fn new(role: Role, agent: Agent, model: Option<String>, risk: RiskClass) -> Key {
        Key {
            role,
            agent,
            model,
            risk,
        }
    }

    /// The role.
    pub fn role(&self) -> Role {
        self.role
    }

    /// The agent.
    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    /// The model.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// The risk class.
    pub fn risk(&self) -> RiskClass {
        self.risk
    }
}

/// The figures of one key.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Figures {
    orders: u32,
    eligible: u32,
    first_pass: u32,
    rounds: u32,
    spend: Spend,
    runtime_ms: u64,
}

impl Figures {
    /// Orders the key took part in.
    pub fn orders(&self) -> u32 {
        self.orders
    }

    /// The orders its first-pass rate rests on.
    pub fn eligible(&self) -> u32 {
        self.eligible
    }

    /// The share of eligible orders that passed the review in its first round; none without eligible orders.
    pub fn first_pass_rate(&self) -> Option<f64> {
        (self.eligible > 0).then(|| f64::from(self.first_pass) / f64::from(self.eligible))
    }

    /// The review rounds that did not pass, in total.
    pub fn rounds(&self) -> u32 {
        self.rounds
    }

    /// What the key's role spent.
    pub fn spend(&self) -> &Spend {
        &self.spend
    }

    /// How long its turns ran.
    pub fn runtime_ms(&self) -> u64 {
        self.runtime_ms
    }
}

/// A first-pass rate with the number of orders it rests on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rate {
    /// The share from 0 to 1.
    pub rate: f64,
    /// The eligible orders.
    pub orders: u32,
}

impl Rate {
    /// Whether it rests on at least [`MIN_ORDERS`] orders.
    pub fn sound(&self) -> bool {
        self.orders >= MIN_ORDERS
    }
}

/// The figures of a set of orders.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Metrics {
    figures: BTreeMap<Key, Figures>,
    median_wait_ms: Option<u64>,
    orders: u32,
    eligible: u32,
}

/// Whether `order` counts towards the first-pass rate: it reached the acceptance without help or a switch (E180,
/// F77).
pub fn eligible(order: &Order) -> bool {
    order.reached() >= Station::Acceptance
        && order.interventions() == 0
        && !order.switched()
        && !order.by_hand()
        && !order
            .deviations()
            .iter()
            .any(|d| matches!(d, Deviation::ModelSwitched { .. }))
}

impl Metrics {
    /// The figures of `orders` at wall-clock `now`.
    pub fn new<'a>(orders: impl IntoIterator<Item = &'a Order>, now: u64) -> Metrics {
        let mut metrics = Metrics::default();
        let mut waits = Vec::new();
        for order in orders {
            metrics.orders += 1;
            let counts = eligible(order);
            metrics.eligible += u32::from(counts);
            let wait = order.waited_ms(now);
            if wait > 0 {
                waits.push(wait);
            }
            for role in Role::ALL {
                let Some(slot) = order.session(role) else { continue };
                let key = Key::new(
                    role,
                    slot.entry().agent.clone(),
                    slot.entry().model.clone(),
                    order.risk(),
                );
                let f = metrics.figures.entry(key).or_default();
                f.orders += 1;
                f.rounds += order.review_rounds();
                f.spend.add(&order.role_spend(role));
                f.runtime_ms += order.role_runtime(role);
                if role == Role::Builder && counts {
                    f.eligible += 1;
                    f.first_pass += u32::from(order.review_rounds() == 0);
                }
            }
        }
        waits.sort_unstable();
        metrics.median_wait_ms = (!waits.is_empty()).then(|| waits[waits.len() / 2]);
        metrics
    }

    /// Every key with its figures.
    pub fn figures(&self) -> &BTreeMap<Key, Figures> {
        &self.figures
    }

    /// How many orders the figures rest on.
    pub fn orders(&self) -> u32 {
        self.orders
    }

    /// How many of them count towards the first-pass rate.
    pub fn eligible(&self) -> u32 {
        self.eligible
    }

    /// The first-pass rate of `agent` with `model` as builder over all risk classes.
    pub fn first_pass_rate(&self, agent: &Agent, model: Option<&str>) -> Option<f64> {
        self.sum(|k| k.role == Role::Builder && k.agent == *agent && k.model.as_deref() == model)
            .map(|rate| rate.rate)
    }

    /// The first-pass rate of `agent` with `model` in `role` for orders of class `risk`, for routing's tie-break
    /// (E170); none for a role without one, or without eligible orders.
    pub fn rate(&self, role: Role, agent: &Agent, model: Option<&str>, risk: RiskClass) -> Option<Rate> {
        self.sum(|k| k.role == role && k.agent == *agent && k.model.as_deref() == model && k.risk == risk)
    }

    fn sum(&self, keep: impl Fn(&Key) -> bool) -> Option<Rate> {
        let (eligible, passed) = self
            .figures
            .iter()
            .filter(|(k, _)| keep(k))
            .fold((0u32, 0u32), |(e, p), (_, f)| (e + f.eligible, p + f.first_pass));
        (eligible > 0).then(|| Rate {
            rate: f64::from(passed) / f64::from(eligible),
            orders: eligible,
        })
    }

    /// The median wait for the user over the orders.
    pub fn median_wait_ms(&self) -> Option<u64> {
        self.median_wait_ms
    }
}
