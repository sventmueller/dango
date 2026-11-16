//! The sheet "Neuer Auftrag" (E185): title, goal and criteria, the options folded away, and what keeps the order from
//! starting, checked by `ide_factory::options` as the user types.

use std::collections::BTreeMap;

use ide_factory::budget::Budget;
use ide_factory::options::{NewOrder, RiskChoice, SheetCheck, check_new_order};
use ide_factory::routing::{Agent, AgentStatus};
use ide_factory::station::{RiskClass, Stage};
use ide_factory::workflow::rules::Rules;
use splat_i18n::t;

use super::words;

/// What the sheet checks against, as the app knows it for the folder.
#[derive(Debug, Clone, Default)]
pub struct SheetContext {
    stage: Stage,
    rules: Rules,
    dirty: bool,
    agents: BTreeMap<Agent, AgentStatus>,
}

impl SheetContext {
    /// The context of stage `stage` under `rules`; `dirty` whether the working copy has uncommitted changes, `agents`
    /// what the app knows about each path.
    pub fn new(stage: Stage, rules: Rules, dirty: bool, agents: BTreeMap<Agent, AgentStatus>) -> SheetContext {
        SheetContext {
            stage,
            rules,
            dirty,
            agents,
        }
    }

    /// Whether the criteria are needed: in stage 1 no planner writes them.
    pub fn criteria_needed(&self) -> bool {
        self.stage == Stage::One
    }

    /// Whether the project allows a specification without approval (E190).
    pub fn skip_allowed(&self) -> bool {
        self.rules.skip_spec_approval()
    }
}

/// The risk choices of the options, in the popup's order.
pub const RISKS: [RiskChoice; 4] = [
    RiskChoice::Automatic,
    RiskChoice::Fixed(RiskClass::Low),
    RiskChoice::Fixed(RiskClass::Medium),
    RiskChoice::Fixed(RiskClass::High),
];

/// The title of a risk choice.
pub fn risk_title(choice: RiskChoice) -> &'static str {
    match choice {
        RiskChoice::Automatic => t("Automatisch"),
        RiskChoice::Fixed(class) => words::risk(class),
    }
}

/// What the user typed and chose in the sheet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SheetForm {
    title: String,
    goal: String,
    criteria: String,
    risk: RiskChoice,
    scope: String,
    budget: String,
    skip: bool,
    options_open: bool,
}

impl SheetForm {
    /// Takes the text fields as they read now.
    pub fn set_texts(&mut self, title: &str, goal: &str, criteria: &str) {
        title.clone_into(&mut self.title);
        goal.clone_into(&mut self.goal);
        criteria.clone_into(&mut self.criteria);
    }

    /// Takes the options as they read now: the risk choice at `risk` of [`RISKS`], the path scope (one pattern per
    /// line or separated by commas), the budget in dollars (empty for the default), and whether G1 is skipped.
    pub fn set_options(&mut self, risk: usize, scope: &str, budget: &str, skip: bool) {
        self.risk = RISKS.get(risk).copied().unwrap_or_default();
        scope.clone_into(&mut self.scope);
        budget.clone_into(&mut self.budget);
        self.skip = skip;
    }

    /// Opens or folds the options.
    pub fn toggle_options(&mut self) {
        self.options_open = !self.options_open;
    }

    /// Whether the options show.
    pub fn options_open(&self) -> bool {
        self.options_open
    }

    /// The order the sheet asks for.
    pub fn order(&self) -> NewOrder {
        NewOrder {
            title: self.title.trim().to_string(),
            goal: self.goal.trim().to_string(),
            criteria: self.criteria.clone(),
            risk: self.risk,
            scope: self
                .scope
                .split(['\n', ','])
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect(),
            budget: self.budget_value(),
            casting: BTreeMap::new(),
            skip_spec_approval: self.skip,
        }
    }

    /// The own budget in dollars; none while the field is empty or no number.
    fn budget_value(&self) -> Option<Budget> {
        let text = self.budget.trim().replace(',', ".");
        let dollars: f64 = text.parse().ok().filter(|d: &f64| d.is_finite() && *d > 0.0)?;
        Some(Budget::new(Some(dollars), None, None))
    }

    /// Whether the budget field holds something that is no amount.
    pub fn budget_unreadable(&self) -> bool {
        !self.budget.trim().is_empty() && self.budget_value().is_none()
    }

    /// The check of the order under `cx`.
    pub fn check(&self, cx: &SheetContext) -> SheetCheck {
        check_new_order(&self.order(), &cx.rules, cx.stage, cx.dirty, &cx.agents)
    }

    /// The lines the sheet shows under its fields: what keeps the order back first, then its notes.
    pub fn messages(&self, cx: &SheetContext) -> Vec<String> {
        let check = self.check(cx);
        let mut lines: Vec<String> = check.problems.iter().map(words::problem).collect();
        if self.budget_unreadable() {
            lines.push(t("Das Budget ist kein Betrag in Dollar.").into());
        }
        lines.extend(check.notes.iter().map(|note| words::note(note).to_string()));
        lines
    }

    /// Whether "Starten" can start the order.
    pub fn can_start(&self, cx: &SheetContext) -> bool {
        self.check(cx).ok() && !self.budget_unreadable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_fields_and_a_criterion_are_enough_in_stage_one() {
        let cx = SheetContext::default();
        let mut form = SheetForm::default();
        assert!(!form.can_start(&cx));
        form.set_texts("CSV export", "Export the table.", "- exports CSV");
        assert!(form.can_start(&cx), "{:?}", form.messages(&cx));
    }

    #[test]
    fn a_budget_that_is_no_amount_keeps_the_order_back() {
        let cx = SheetContext::default();
        let mut form = SheetForm::default();
        form.set_texts("A", "B", "C");
        form.set_options(0, "src/**, docs/*", "zwei", false);
        assert!(!form.can_start(&cx));
        form.set_options(0, "src/**, docs/*", "2,50", false);
        assert!(form.can_start(&cx));
        assert_eq!(form.order().scope, ["src/**", "docs/*"]);
        assert_eq!(form.order().budget.and_then(|b| b.dollars()), Some(2.5));
    }
}
