//! The user's defaults for all projects (E183, Einstellungen ▸ Software Factory): ranked lists per role, the reviewer rule,
//! limits and budgets, and how orders behave after a restart and on battery. The app stores them per Mac (E113).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::WorktreeLocation;
use crate::budget::Budgets;
use crate::limits::Limits;
use crate::routing::{Agent, RankEntry, ReviewerRule};
use crate::station::Role;

/// The user's defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserDefaults {
    roles: BTreeMap<Role, Vec<RankEntry>>,
    reviewer_rule: ReviewerRule,
    limits: Limits,
    budgets: Budgets,
    resume_after_restart: bool,
    awake_on_battery: bool,
    location: WorktreeLocation,
    rule_proposals: bool,
}

impl Default for UserDefaults {
    fn default() -> Self {
        let list = |agents: &[Agent]| agents.iter().cloned().map(RankEntry::new).collect::<Vec<_>>();
        UserDefaults {
            roles: BTreeMap::from([
                (Role::Planner, list(&[Agent::Claude, Agent::Codex])),
                (Role::Builder, list(&[Agent::Claude, Agent::Codex])),
                (Role::Reviewer, list(&[Agent::Codex, Agent::Claude])),
                (Role::Scout, list(&[Agent::Own, Agent::Claude])),
            ]),
            reviewer_rule: ReviewerRule::Required,
            limits: Limits::default(),
            budgets: Budgets::default(),
            resume_after_restart: false,
            awake_on_battery: false,
            location: WorktreeLocation::DataFolder,
            rule_proposals: true,
        }
    }
}

impl UserDefaults {
    /// The ranked list of `role`.
    pub fn role(&self, role: Role) -> &[RankEntry] {
        self.roles.get(&role).map_or(&[], Vec::as_slice)
    }

    /// Sets the ranked list of `role`.
    pub fn set_role(&mut self, role: Role, entries: Vec<RankEntry>) {
        self.roles.insert(role, entries);
    }

    /// The reviewer rule.
    pub fn reviewer_rule(&self) -> ReviewerRule {
        self.reviewer_rule
    }

    /// Sets the reviewer rule.
    pub fn set_reviewer_rule(&mut self, rule: ReviewerRule) {
        self.reviewer_rule = rule;
    }

    /// The limits.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Sets the limits.
    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// The budgets.
    pub fn budgets(&self) -> &Budgets {
        &self.budgets
    }

    /// Sets the budgets.
    pub fn set_budgets(&mut self, budgets: Budgets) {
        self.budgets = budgets;
    }

    /// Whether orders resume by themselves after a restart; off by default (E167).
    pub fn resume_after_restart(&self) -> bool {
        self.resume_after_restart
    }

    /// Sets whether orders resume by themselves after a restart.
    pub fn set_resume_after_restart(&mut self, on: bool) {
        self.resume_after_restart = on;
    }

    /// Whether running orders keep the Mac awake on battery too (E169).
    pub fn awake_on_battery(&self) -> bool {
        self.awake_on_battery
    }

    /// Sets whether running orders keep the Mac awake on battery.
    pub fn set_awake_on_battery(&mut self, on: bool) {
        self.awake_on_battery = on;
    }

    /// Where worktrees lie unless the project says otherwise.
    pub fn location(&self) -> WorktreeLocation {
        self.location
    }

    /// Sets where worktrees lie.
    pub fn set_location(&mut self, location: WorktreeLocation) {
        self.location = location;
    }

    /// Whether a scout proposes a change to the agents' instructions after an order ended escalated or failed; on
    /// by default (B47).
    pub fn rule_proposals(&self) -> bool {
        self.rule_proposals
    }

    /// Sets whether a scout proposes rule changes.
    pub fn set_rule_proposals(&mut self, on: bool) {
        self.rule_proposals = on;
    }
}
