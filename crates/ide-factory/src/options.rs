//! The sheet "Neuer Auftrag" (E185): title, goal and criteria, and the folded options risk class, path scope, budget,
//! casting per role and skipping G1, checked before the order starts; and the later changes of those options, each
//! allowed until its station begins.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::budget::{Budget, Spend, Unit};
use crate::criteria::criteria_from_lines;
use crate::gates::PathScope;
use crate::relevance::PatternError;
use crate::routing::{AgentStatus, Capabilities, RankEntry, ReviewerRule, vendors_differ};
use crate::station::{RiskClass, Role, Stage, Station};
use crate::workflow::rules::Rules;

/// The expected risk class as the user chose it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "risk", content = "class")]
pub enum RiskChoice {
    /// From the planner's proposal, else low; the measured class can only raise it (E163).
    #[default]
    Automatic,
    /// This class.
    Fixed(RiskClass),
}

/// What the user asked for in the sheet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewOrder {
    /// The title.
    pub title: String,
    /// The goal, several lines.
    pub goal: String,
    /// The criteria, one per line.
    pub criteria: String,
    /// The expected risk class.
    pub risk: RiskChoice,
    /// The path scope; empty for none.
    pub scope: Vec<String>,
    /// An own budget instead of the default.
    pub budget: Option<Budget>,
    /// A fixed entry per role instead of its ranked list (from stage 2).
    pub casting: BTreeMap<Role, RankEntry>,
    /// Skip G1 (only for class low, only where the project allows it).
    pub skip_spec_approval: bool,
}

/// Why the sheet does not let an order start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "problem")]
pub enum OptionProblem {
    /// The title is empty.
    MissingTitle,
    /// The goal is empty.
    MissingGoal,
    /// There is no criterion, and in stage 1 no planner writes them.
    MissingCriteria,
    /// A pattern of the path scope does not parse.
    BadScope {
        /// The pattern.
        pattern: String,
        /// globset's words.
        message: String,
    },
    /// A fixed entry may not take its role (E173).
    CannotTakeRole {
        /// The role.
        role: Role,
    },
    /// A fixed casting leaves no reviewer of another vendor, and the rule requires one (E162).
    SameVendor,
    /// Skipping G1 needs the risk class low.
    SkipNeedsLowRisk,
    /// The project does not allow skipping G1, or the user has not confirmed it (E190).
    SkipNotAllowed,
    /// The workflow file is invalid (E190).
    InvalidWorkflow,
}

/// What the sheet tells the user without keeping the order from starting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetNote {
    /// The working copy has uncommitted changes, and the order starts at HEAD without them (F51).
    StartsAtHead,
    /// The reviewer will be of a builder's vendor, allowed by the rule "preferred".
    WithoutVendorChange,
}

/// The sheet's verdict.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetCheck {
    /// What keeps the order from starting.
    pub problems: Vec<OptionProblem>,
    /// What the sheet shows anyway.
    pub notes: Vec<SheetNote>,
}

impl SheetCheck {
    /// Whether the order may start.
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Checks `order` under `rules` in `stage`; `dirty` says whether the working copy has uncommitted changes, `agents`
/// what the app knows about each path.
pub fn check_new_order(
    order: &NewOrder,
    rules: &Rules,
    stage: Stage,
    dirty: bool,
    agents: &BTreeMap<crate::routing::Agent, AgentStatus>,
) -> SheetCheck {
    let mut check = SheetCheck::default();
    let problems = &mut check.problems;
    if order.title.trim().is_empty() {
        problems.push(OptionProblem::MissingTitle);
    }
    if order.goal.trim().is_empty() {
        problems.push(OptionProblem::MissingGoal);
    }
    if stage == Stage::One && criteria_from_lines(&order.criteria).is_empty() {
        problems.push(OptionProblem::MissingCriteria);
    }
    if let Err(PatternError { pattern, message }) = PathScope::new(&order.scope) {
        problems.push(OptionProblem::BadScope { pattern, message });
    }
    if !rules.errors().is_empty() {
        problems.push(OptionProblem::InvalidWorkflow);
    }
    if order.skip_spec_approval {
        if order.risk != RiskChoice::Fixed(RiskClass::Low) {
            problems.push(OptionProblem::SkipNeedsLowRisk);
        }
        if !rules.skip_spec_approval() {
            problems.push(OptionProblem::SkipNotAllowed);
        }
    }
    if stage >= Stage::Two {
        check_casting(order, rules, agents, &mut check);
    }
    if dirty {
        check.notes.push(SheetNote::StartsAtHead);
    }
    check
}

fn check_casting(
    order: &NewOrder,
    rules: &Rules,
    agents: &BTreeMap<crate::routing::Agent, AgentStatus>,
    check: &mut SheetCheck,
) {
    for (role, entry) in &order.casting {
        let capabilities = agents
            .get(&entry.agent)
            .map_or_else(|| Capabilities::of(&entry.agent), |status| status.capabilities);
        if !capabilities.takes(*role) {
            check.problems.push(OptionProblem::CannotTakeRole { role: *role });
        }
    }
    let list = |role: Role| -> Vec<RankEntry> {
        order
            .casting
            .get(&role)
            .map_or_else(|| rules.role(role).to_vec(), |entry| vec![entry.clone()])
    };
    if !order.casting.contains_key(&Role::Builder) && !order.casting.contains_key(&Role::Reviewer) {
        return;
    }
    let Some(builder) = list(Role::Builder).first().map(RankEntry::vendor) else {
        return;
    };
    let reviewers = list(Role::Reviewer);
    if reviewers.iter().any(|r| vendors_differ(r.vendor(), builder)) {
        return;
    }
    match rules.reviewer_rule() {
        ReviewerRule::Required => check.problems.push(OptionProblem::SameVendor),
        ReviewerRule::Preferred => check.notes.push(SheetNote::WithoutVendorChange),
    }
}

/// A change of an option after the order was created (E185).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "option", content = "value")]
pub enum OptionChange {
    /// The expected risk class; until the build begins.
    Risk(RiskChoice),
    /// The path scope; until the build begins.
    Scope(Vec<String>),
    /// The budget; at any time, never below what is spent.
    Budget(Option<Budget>),
    /// A fixed entry for a role, or its ranked list again; until the role's station begins.
    Casting(Role, Option<RankEntry>),
    /// Skipping G1; until the specification begins.
    SkipSpecApproval(bool),
}

/// Why an option cannot be changed now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum ChangeRefusal {
    /// Its station has begun.
    StationBegun {
        /// The station.
        station: Station,
    },
    /// The budget would lie below what is spent.
    BelowSpent {
        /// In which unit.
        unit: Unit,
    },
    /// The new value is not allowed, as the sheet would say.
    Problem {
        /// What is wrong.
        problem: OptionProblem,
    },
}

/// The station whose beginning locks `change`.
pub fn locking_station(change: &OptionChange) -> Option<Station> {
    match change {
        OptionChange::Risk(_) | OptionChange::Scope(_) => Some(Station::Build),
        OptionChange::SkipSpecApproval(_) => Some(Station::Spec),
        OptionChange::Budget(_) => None,
        OptionChange::Casting(role, _) => Some(match role {
            Role::Planner | Role::Scout => Station::Spec,
            Role::Builder => Station::Build,
            Role::Reviewer => Station::Review,
        }),
    }
}

/// Whether `change` may be made now: `reached` is the furthest station the order has begun, `spent` what it spent.
pub fn check_change(change: &OptionChange, reached: Station, spent: &Spend) -> Result<(), ChangeRefusal> {
    if let Some(station) = locking_station(change)
        && reached >= station
    {
        return Err(ChangeRefusal::StationBegun { station });
    }
    match change {
        OptionChange::Budget(Some(budget)) => match budget.below(spent) {
            Some(unit) => Err(ChangeRefusal::BelowSpent { unit }),
            None => Ok(()),
        },
        OptionChange::Scope(scope) => PathScope::new(scope).map(|_| ()).map_err(|error| ChangeRefusal::Problem {
            problem: OptionProblem::BadScope {
                pattern: error.pattern,
                message: error.message,
            },
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::Agent;
    use crate::workflow::UserDefaults;
    use crate::workflow::rules::Confirmations;

    fn order() -> NewOrder {
        NewOrder {
            title: "CSV export".into(),
            goal: "Export the table as CSV.".into(),
            criteria: "- exports CSV\n".into(),
            ..NewOrder::default()
        }
    }

    fn rules(rule: ReviewerRule) -> Rules {
        let mut defaults = UserDefaults::default();
        defaults.set_reviewer_rule(rule);
        Rules::new(&defaults, None, &Confirmations::default())
    }

    fn agents() -> BTreeMap<Agent, AgentStatus> {
        [Agent::Claude, Agent::Codex, Agent::OpenCode]
            .into_iter()
            .map(|a| (a.clone(), AgentStatus::ready(&a)))
            .collect()
    }

    fn problems(order: &NewOrder, stage: Stage, rule: ReviewerRule) -> SheetCheck {
        check_new_order(order, &rules(rule), stage, false, &agents())
    }

    #[test]
    fn sheet_table() {
        type Edit = fn(&mut NewOrder);
        let cases: [(&str, Edit, Stage, Vec<OptionProblem>); 8] = [
            ("complete", |_| {}, Stage::One, vec![]),
            ("no title", |o| o.title = " ".into(), Stage::One, vec![OptionProblem::MissingTitle]),
            ("no goal", |o| o.goal.clear(), Stage::One, vec![OptionProblem::MissingGoal]),
            ("no criteria in stage 1", |o| o.criteria = "\n".into(), Stage::One, vec![OptionProblem::MissingCriteria]),
            ("the planner writes them in stage 2", |o| o.criteria.clear(), Stage::Two, vec![]),
            (
                "skip G1 without low risk and without the project's leave",
                |o| o.skip_spec_approval = true,
                Stage::Two,
                vec![OptionProblem::SkipNeedsLowRisk, OptionProblem::SkipNotAllowed],
            ),
            (
                "OpenCode cannot build",
                |o| {
                    o.casting.insert(Role::Builder, RankEntry::new(Agent::OpenCode));
                },
                Stage::Two,
                vec![OptionProblem::CannotTakeRole { role: Role::Builder }],
            ),
            (
                "builder and reviewer of one vendor under the rule required",
                |o| {
                    o.casting.insert(Role::Builder, RankEntry::new(Agent::Claude));
                    o.casting.insert(Role::Reviewer, RankEntry::new(Agent::Claude));
                },
                Stage::Two,
                vec![OptionProblem::SameVendor],
            ),
        ];
        for (name, edit, stage, want) in cases {
            let mut new = order();
            edit(&mut new);
            assert_eq!(problems(&new, stage, ReviewerRule::Required).problems, want, "{name}");
        }
    }

    #[test]
    fn the_rule_preferred_only_notes_a_missing_vendor_change() {
        let mut new = order();
        new.casting.insert(Role::Builder, RankEntry::new(Agent::Codex));
        new.casting.insert(Role::Reviewer, RankEntry::new(Agent::Codex));
        let check = problems(&new, Stage::Two, ReviewerRule::Preferred);
        assert!(check.ok());
        assert_eq!(check.notes, [SheetNote::WithoutVendorChange]);
    }

    #[test]
    fn uncommitted_changes_are_noted() {
        let check = check_new_order(&order(), &rules(ReviewerRule::Required), Stage::One, true, &agents());
        assert_eq!(check.notes, [SheetNote::StartsAtHead]);
    }

    #[test]
    fn options_lock_when_their_station_begins() {
        let spent = Spend::new(3.0, 0, 0.0);
        let risk = OptionChange::Risk(RiskChoice::Fixed(RiskClass::High));
        assert_eq!(check_change(&risk, Station::Spec, &spent), Ok(()));
        assert_eq!(
            check_change(&risk, Station::Build, &spent),
            Err(ChangeRefusal::StationBegun { station: Station::Build })
        );
        let reviewer = OptionChange::Casting(Role::Reviewer, None);
        assert_eq!(check_change(&reviewer, Station::Build, &spent), Ok(()));
        let low = OptionChange::Budget(Some(Budget::new(Some(2.0), None, None)));
        assert_eq!(
            check_change(&low, Station::Acceptance, &spent),
            Err(ChangeRefusal::BelowSpent { unit: Unit::Dollars })
        );
        let enough = OptionChange::Budget(Some(Budget::new(Some(3.5), None, None)));
        assert_eq!(check_change(&enough, Station::Acceptance, &spent), Ok(()));
    }
}
