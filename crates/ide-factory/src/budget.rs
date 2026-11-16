//! Budgets in the three units of E168 (F59): dollars where a path reports or prices them, tokens where the count is
//! reliable, and the share of a subscription window. A path that reports usage only after a turn can overshoot by
//! that one turn (C64), which the evidence names.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What a budget limits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope", content = "provider")]
pub enum BudgetScope {
    /// One order.
    Order,
    /// One day across all projects.
    Day,
    /// One billing service per day, such as `anthropic-api`.
    Provider(String),
}

/// A unit of spending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// US dollars billed or priced (E37, E57).
    Dollars,
    /// Tokens, where the path counts them reliably.
    Tokens,
    /// Percent of a subscription window.
    WindowShare,
}

/// A budget: a ceiling per unit, none where the unit is not limited.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Budget {
    dollars: Option<f64>,
    tokens: Option<u64>,
    window_percent: Option<f64>,
}

fn stricter<T: PartialOrd + Copy>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if b < a { b } else { a }),
        (a, None) => a,
        (None, b) => b,
    }
}

impl Budget {
    /// A budget of these ceilings.
    pub fn new(dollars: Option<f64>, tokens: Option<u64>, window_percent: Option<f64>) -> Budget {
        Budget {
            dollars,
            tokens,
            window_percent,
        }
    }

    /// The ceiling in dollars.
    pub fn dollars(&self) -> Option<f64> {
        self.dollars
    }

    /// The ceiling in tokens.
    pub fn tokens(&self) -> Option<u64> {
        self.tokens
    }

    /// The ceiling in percent of a window.
    pub fn window_percent(&self) -> Option<f64> {
        self.window_percent
    }

    /// Whether no unit is limited.
    pub fn is_unlimited(&self) -> bool {
        self.dollars.is_none() && self.tokens.is_none() && self.window_percent.is_none()
    }

    /// The stricter ceiling of both per unit (E190).
    pub fn tighten(&self, other: &Budget) -> Budget {
        Budget {
            dollars: stricter(self.dollars, other.dollars),
            tokens: stricter(self.tokens, other.tokens),
            window_percent: stricter(self.window_percent, other.window_percent),
        }
    }

    /// The first unit `spend` reached the ceiling of.
    pub fn reached(&self, spend: &Spend) -> Option<Unit> {
        if self.dollars.is_some_and(|max| spend.dollars >= max) {
            return Some(Unit::Dollars);
        }
        if self.tokens.is_some_and(|max| spend.tokens >= max) {
            return Some(Unit::Tokens);
        }
        self.window_percent
            .is_some_and(|max| spend.window_share * 100.0 >= max)
            .then_some(Unit::WindowShare)
    }

    /// The first unit of this budget that lies below `spend`, for refusing a budget lower than what is spent (E185).
    pub fn below(&self, spend: &Spend) -> Option<Unit> {
        if self.dollars.is_some_and(|max| max < spend.dollars) {
            return Some(Unit::Dollars);
        }
        if self.tokens.is_some_and(|max| max < spend.tokens) {
            return Some(Unit::Tokens);
        }
        self.window_percent
            .is_some_and(|max| max < spend.window_share * 100.0)
            .then_some(Unit::WindowShare)
    }

    /// The dollars left after `spend`, for paths that take a ceiling at their start (C64).
    pub fn dollars_left(&self, spend: &Spend) -> Option<f64> {
        self.dollars.map(|max| (max - spend.dollars).max(0.0))
    }
}

/// The budgets of E183: per order, per day and per billing service and day.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Budgets {
    order: Budget,
    day: Budget,
    providers: BTreeMap<String, Budget>,
}

impl Budgets {
    /// Budgets of these ceilings.
    pub fn new(order: Budget, day: Budget, providers: BTreeMap<String, Budget>) -> Budgets {
        Budgets { order, day, providers }
    }

    /// The budget of one order.
    pub fn order(&self) -> &Budget {
        &self.order
    }

    /// The budget of one day.
    pub fn day(&self) -> &Budget {
        &self.day
    }

    /// The budget of a billing service per day.
    pub fn provider(&self, service: &str) -> Option<&Budget> {
        self.providers.get(service)
    }

    /// The stricter of both per scope and unit (E190).
    pub fn tighten(&self, other: &Budgets) -> Budgets {
        let mut providers = self.providers.clone();
        for (service, budget) in &other.providers {
            let mine = providers.remove(service).unwrap_or_default();
            providers.insert(service.clone(), mine.tighten(budget));
        }
        Budgets {
            order: self.order.tighten(&other.order),
            day: self.day.tighten(&other.day),
            providers,
        }
    }

    /// The first scope whose budget is reached by `order` (this order's spending) or `daily`, with the unit.
    pub fn reached(&self, order: &Spend, daily: &DailyAccount) -> Option<(BudgetScope, Unit)> {
        if let Some(unit) = self.order.reached(order) {
            return Some((BudgetScope::Order, unit));
        }
        if let Some(unit) = self.day.reached(daily.total()) {
            return Some((BudgetScope::Day, unit));
        }
        self.providers.iter().find_map(|(service, budget)| {
            let spent = daily.provider(service)?;
            budget
                .reached(spent)
                .map(|unit| (BudgetScope::Provider(service.clone()), unit))
        })
    }
}

/// What was spent, in every unit; units a path could not count stay zero and are named by [`Metering`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Spend {
    dollars: f64,
    tokens: u64,
    window_share: f64,
}

impl Spend {
    /// A spending of these amounts; the window share runs from 0 to 1.
    pub fn new(dollars: f64, tokens: u64, window_share: f64) -> Spend {
        Spend {
            dollars,
            tokens,
            window_share,
        }
    }

    /// Dollars.
    pub fn dollars(&self) -> f64 {
        self.dollars
    }

    /// Tokens.
    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    /// Share of a window, from 0 to 1.
    pub fn window_share(&self) -> f64 {
        self.window_share
    }

    /// Adds `other`.
    pub fn add(&mut self, other: &Spend) {
        self.dollars += other.dollars;
        self.tokens = self.tokens.saturating_add(other.tokens);
        self.window_share += other.window_share;
    }

    /// Whether nothing was spent.
    pub fn is_zero(&self) -> bool {
        self.dollars == 0.0 && self.tokens == 0 && self.window_share == 0.0
    }
}

/// What a session reported it used so far, as the app maps it from the thread's usage (E57, E37).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageReport {
    /// What the session cost the user, billed or priced; `None` on a subscription or without a price.
    pub dollars: Option<f64>,
    /// The tokens used so far.
    pub tokens: Option<u64>,
    /// Whether the path counts tokens reliably; ACP agents do not.
    pub tokens_reliable: bool,
}

/// Which units an order's paths could count, for the evidence (E168).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metering {
    /// Some session reported no reliable figure at all, so only runtime and turns limited it.
    pub unmetered: bool,
    /// A usage report after a turn pushed a budget past its ceiling (C64).
    pub overshoot: bool,
}

/// The spending of one day across all projects, which the app keeps in its state and hands to every folder's
/// factory (E168).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DailyAccount {
    day: String,
    total: Spend,
    providers: BTreeMap<String, Spend>,
    #[serde(default, skip_serializing_if = "is_zero")]
    ends: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

impl DailyAccount {
    /// An empty account for `day`, such as `2026-11-16` in the user's time zone.
    pub fn new(day: impl Into<String>) -> DailyAccount {
        DailyAccount {
            day: day.into(),
            ..DailyAccount::default()
        }
    }

    /// The account with its day ending at the wall clock `ends`, the start of the next local day.
    pub fn until(mut self, ends: u64) -> DailyAccount {
        self.ends = ends;
        self
    }

    /// When its day ends on the wall clock; zero when the app did not say.
    pub fn ends(&self) -> u64 {
        self.ends
    }

    /// Whether its day still runs at the wall clock `wall`; an account without an end never runs out by itself.
    pub fn current(&self, wall: u64) -> bool {
        self.ends == 0 || wall < self.ends
    }

    /// The day.
    pub fn day(&self) -> &str {
        &self.day
    }

    /// The day's spending.
    pub fn total(&self) -> &Spend {
        &self.total
    }

    /// A billing service's spending of the day.
    pub fn provider(&self, service: &str) -> Option<&Spend> {
        self.providers.get(service)
    }

    /// Adds `spend` on `service`, starting over first when `day` is a new day.
    pub fn add(&mut self, day: &str, service: Option<&str>, spend: &Spend) {
        if self.day != day {
            *self = DailyAccount::new(day);
        }
        self.total.add(spend);
        if let Some(service) = service {
            self.providers.entry(service.to_string()).or_default().add(spend);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stricter_budget_wins_per_unit() {
        let user = Budget::new(Some(10.0), None, Some(50.0));
        let project = Budget::new(Some(20.0), Some(1000), None);
        let both = user.tighten(&project);
        assert_eq!(both, Budget::new(Some(10.0), Some(1000), Some(50.0)));
    }

    #[test]
    fn a_budget_is_reached_at_its_ceiling_in_any_unit() {
        let budget = Budget::new(Some(5.0), Some(100), Some(20.0));
        assert_eq!(budget.reached(&Spend::new(4.9, 99, 0.19)), None);
        assert_eq!(budget.reached(&Spend::new(5.0, 0, 0.0)), Some(Unit::Dollars));
        assert_eq!(budget.reached(&Spend::new(0.0, 100, 0.0)), Some(Unit::Tokens));
        assert_eq!(budget.reached(&Spend::new(0.0, 0, 0.2)), Some(Unit::WindowShare));
        assert_eq!(Budget::default().reached(&Spend::new(1e9, u64::MAX, 1.0)), None);
    }

    #[test]
    fn the_daily_account_starts_over_on_a_new_day_and_counts_per_service() {
        let mut account = DailyAccount::new("2026-11-16");
        account.add("2026-11-16", Some("anthropic-api"), &Spend::new(2.0, 10, 0.0));
        account.add("2026-11-16", None, &Spend::new(1.0, 0, 0.0));
        assert_eq!(account.total().dollars(), 3.0);
        assert_eq!(account.provider("anthropic-api").map(Spend::dollars), Some(2.0));
        let budgets = Budgets::new(
            Budget::default(),
            Budget::default(),
            BTreeMap::from([("anthropic-api".to_string(), Budget::new(Some(2.0), None, None))]),
        );
        assert_eq!(
            budgets.reached(&Spend::default(), &account),
            Some((BudgetScope::Provider("anthropic-api".into()), Unit::Dollars))
        );
        account.add("2026-11-17", None, &Spend::new(0.5, 0, 0.0));
        assert_eq!((account.day(), account.total().dollars()), ("2026-11-17", 0.5));
        assert_eq!(budgets.reached(&Spend::default(), &account), None);
    }

    #[test]
    fn an_account_with_an_end_runs_out_at_it_and_one_without_never() {
        let account = DailyAccount::new("2026-11-16").until(5_000);
        assert!(account.current(4_999) && !account.current(5_000));
        assert!(DailyAccount::new("2026-11-16").current(u64::MAX));
        let text = serde_json::to_string(&account).unwrap();
        assert_eq!(serde_json::from_str::<DailyAccount>(&text).unwrap(), account);
        let mut old = DailyAccount::new("2026-11-16");
        old.add("2026-11-16", None, &Spend::new(1.0, 0, 0.0));
        let text = serde_json::to_string(&old).unwrap();
        assert!(!text.contains("ends"), "{text}");
        assert_eq!(serde_json::from_str::<DailyAccount>(&text).unwrap().ends(), 0);
    }

    #[test]
    fn a_budget_below_the_spending_is_refused() {
        let spent = Spend::new(3.0, 0, 0.0);
        assert_eq!(Budget::new(Some(2.0), None, None).below(&spent), Some(Unit::Dollars));
        assert_eq!(Budget::new(Some(3.0), None, None).below(&spent), None);
    }
}
