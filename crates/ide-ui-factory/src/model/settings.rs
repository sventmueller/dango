//! The Leitstand's settings (E183): the user's defaults for all projects in "Einstellungen ▸ Software Factory", and the
//! project's workflow file in "Software Factory für <Ordner>", each value with where it comes from and a way back to the
//! default: every role with model and effort, the reviewer rule, limits, budgets per order, day and provider, gates,
//! copying and setup, risk patterns and the files that count for the review.

use std::collections::BTreeMap;

use ide_core::explain::Explained;
use ide_factory::budget::{Budget, Budgets};
use ide_factory::limits::Limit;
use ide_factory::models::Catalog;
use ide_factory::routing::{Agent, Capabilities, RankEntry, ReviewerRule};
use ide_factory::station::Role;
use ide_factory::workflow::edit::{self, EditError, EditValue};
use ide_factory::workflow::{UserDefaults, Workflow, WorkflowError, parse};
use splat_i18n::{t, tf};
use toml_edit::{DocumentMut, Item};

use super::{ranks, words};

/// The limits the settings offer, in their order, with their unit: minutes, a count or percent.
pub const LIMITS: [Limit; 10] = [
    Limit::ParallelOrders,
    Limit::TurnMinutes,
    Limit::TurnsPerStation,
    Limit::StallMinutes,
    Limit::ToolCallMinutes,
    Limit::GateMinutes,
    Limit::SetupMinutes,
    Limit::Attempts,
    Limit::ReviewRounds,
    Limit::WindowStartPercent,
];

/// The title of `limit`'s row.
pub fn limit_title(limit: Limit) -> &'static str {
    match limit {
        Limit::StallMinutes => t("Hängt nach (min):"),
        Limit::ToolCallMinutes => t("Werkzeugaufruf höchstens (min):"),
        Limit::WakeGraceMinutes => t("Nach dem Aufwachen (min):"),
        Limit::TurnMinutes => t("Zug höchstens (min):"),
        Limit::TurnsPerStation => t("Züge je Station:"),
        Limit::ReviewRounds => t("Prüfrunden:"),
        Limit::GateMinutes => t("Prüfbefehl höchstens (min):"),
        Limit::SetupMinutes => t("Einrichtung höchstens (min):"),
        Limit::GateOutputMib => t("Ausgabe höchstens (MiB):"),
        Limit::RetryBaseSeconds => t("Erste Wiederholung nach (s):"),
        Limit::RetryCapSeconds => t("Wiederholung höchstens nach (s):"),
        Limit::Attempts => t("Versuche je Station:"),
        Limit::WindowStartPercent => t("Kein Start über (% Fenster):"),
        Limit::ParallelOrders => t("Aufträge gleichzeitig:"),
    }
}

/// The agents a ranked list offers, in the menu's order.
pub const AGENTS: [Agent; 5] = [
    Agent::Claude,
    Agent::Codex,
    Agent::Own,
    Agent::OpenCode,
    Agent::Antigravity,
];

/// The paths whose API spending a budget per day can hold (E168), each with its billing service as the app names it.
pub const PROVIDERS: [Agent; 3] = [Agent::Claude, Agent::Codex, Agent::Own];

/// The billing service of `agent`'s API, as the runtime counts its spending.
pub fn api_service(agent: &Agent) -> String {
    format!("{agent}-api")
}

/// The title of the row of `agent`'s budget per day.
pub fn provider_title(agent: &Agent) -> String {
    tf("{} über API je Tag ($):", &[&agent_name(agent)])
}

/// The name of `agent` in the lists.
pub fn agent_name(agent: &Agent) -> String {
    match agent {
        Agent::Claude => "Claude".into(),
        Agent::Codex => "Codex".into(),
        Agent::OpenCode => "OpenCode".into(),
        Agent::Antigravity => "Antigravity".into(),
        Agent::Own => t("Eigener Agent").into(),
        Agent::Acp(name) => name.clone(),
    }
}

/// Whether `entry` can take `role` at all (E173); one that cannot shows greyed out with its reason.
pub fn takes(entry: &RankEntry, role: Role) -> bool {
    Capabilities::of(&entry.agent).takes(role)
}

/// The user's defaults as the settings tab edits them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DefaultsForm {
    defaults: UserDefaults,
}

impl DefaultsForm {
    /// The form over `defaults`.
    pub fn new(defaults: UserDefaults) -> DefaultsForm {
        DefaultsForm { defaults }
    }

    /// The defaults as edited.
    pub fn defaults(&self) -> &UserDefaults {
        &self.defaults
    }

    /// The ranked list of `role`.
    pub fn list(&self, role: Role) -> &[RankEntry] {
        self.defaults.role(role)
    }

    /// Changes `role`'s list with `edit`; what `edit` returns.
    pub fn edit_list<R>(&mut self, role: Role, edit: impl FnOnce(&mut Vec<RankEntry>) -> R) -> R {
        let mut list = self.list(role).to_vec();
        let result = edit(&mut list);
        self.defaults.set_role(role, list);
        result
    }

    /// Adds `agent` at the end of `role`'s list, unless it is in it already.
    pub fn add(&mut self, role: Role, agent: Agent) {
        self.edit_list(role, |list| ranks::add(list, agent));
    }

    /// Moves the entry at `index` of `role`'s list one place up, or down; where it went.
    pub fn shift(&mut self, role: Role, index: usize, up: bool) -> usize {
        self.edit_list(role, |list| ranks::shift(list, index, up))
    }

    /// Sets the reasoning effort of the entry at `index`; empty for the agent's default.
    pub fn set_effort(&mut self, role: Role, index: usize, effort: &str) {
        self.edit_list(role, |list| ranks::set_effort(list, index, effort));
    }

    /// Whether a reviewer of the builders' vendor may review when no other is ready (E162).
    pub fn reviewer_preferred(&self) -> bool {
        self.defaults.reviewer_rule() == ReviewerRule::Preferred
    }

    /// Sets the reviewer rule: another vendor preferred, or required.
    pub fn set_reviewer_preferred(&mut self, preferred: bool) {
        let rule = if preferred {
            ReviewerRule::Preferred
        } else {
            ReviewerRule::Required
        };
        self.defaults.set_reviewer_rule(rule);
    }

    /// The budget in dollars of the billing `service` per day; none for no limit.
    pub fn provider_budget(&self, service: &str) -> Option<f64> {
        self.defaults.budgets().provider(service).and_then(Budget::dollars)
    }

    /// Sets the dollars of `service`'s budget per day, keeping its other units and every other budget.
    pub fn set_provider_budget(&mut self, service: &str, dollars: Option<f64>) {
        let old = self.defaults.budgets();
        let mut providers = BTreeMap::new();
        for name in services(old) {
            if let Some(budget) = old.provider(&name) {
                providers.insert(name, budget.clone());
            }
        }
        let kept = providers.remove(service).unwrap_or_default();
        let budget = Budget::new(dollars, kept.tokens(), kept.window_percent());
        if !budget.is_unlimited() {
            providers.insert(service.to_string(), budget);
        }
        let budgets = Budgets::new(old.order().clone(), old.day().clone(), providers);
        self.defaults.set_budgets(budgets);
    }

    /// The value of `limit`.
    pub fn limit(&self, limit: Limit) -> u64 {
        self.defaults.limits().get(limit)
    }

    /// Sets `limit` to `value`, which the limits keep at one at least.
    pub fn set_limit(&mut self, limit: Limit, value: u64) {
        let limits = self.defaults.limits().clone().with(limit, value);
        self.defaults.set_limits(limits);
    }

    /// The budget in dollars of one order and of one day; none for no limit.
    pub fn budgets(&self) -> (Option<f64>, Option<f64>) {
        let budgets = self.defaults.budgets();
        (budgets.order().dollars(), budgets.day().dollars())
    }

    /// Sets the dollars of one order's and one day's budget, keeping the other units and the services' budgets.
    pub fn set_budgets(&mut self, order: Option<f64>, day: Option<f64>) {
        let old = self.defaults.budgets();
        let keep =
            |budget: &Budget, dollars: Option<f64>| Budget::new(dollars, budget.tokens(), budget.window_percent());
        let providers = BTreeMap::new();
        let budgets = Budgets::new(keep(old.order(), order), keep(old.day(), day), providers)
            .tighten(&without_order_and_day(old));
        self.defaults.set_budgets(budgets);
    }

    /// Whether paused orders go on by themselves after a restart (E167).
    pub fn resume_after_restart(&self) -> bool {
        self.defaults.resume_after_restart()
    }

    /// Sets whether paused orders go on by themselves after a restart.
    pub fn set_resume_after_restart(&mut self, on: bool) {
        self.defaults.set_resume_after_restart(on);
    }

    /// Whether the Mac stays awake for orders on battery (E169).
    pub fn awake_on_battery(&self) -> bool {
        self.defaults.awake_on_battery()
    }

    /// Sets whether the Mac stays awake for orders on battery.
    pub fn set_awake_on_battery(&mut self, on: bool) {
        self.defaults.set_awake_on_battery(on);
    }

    /// Whether a scout proposes rule changes after an order ended escalated or failed (B47).
    pub fn rule_proposals(&self) -> bool {
        self.defaults.rule_proposals()
    }

    /// Sets whether a scout proposes rule changes.
    pub fn set_rule_proposals(&mut self, on: bool) {
        self.defaults.set_rule_proposals(on);
    }
}

/// `budgets` with only its services' budgets, so tightening with it keeps them and changes nothing else.
fn without_order_and_day(budgets: &Budgets) -> Budgets {
    let mut providers = BTreeMap::new();
    for service in services(budgets) {
        if let Some(budget) = budgets.provider(&service) {
            providers.insert(service, budget.clone());
        }
    }
    Budgets::new(Budget::default(), Budget::default(), providers)
}

/// The services `budgets` has a budget for; `Budgets` hands them out only by name, so they are read from its JSON.
fn services(budgets: &Budgets) -> Vec<String> {
    serde_json::to_value(budgets)
        .ok()
        .and_then(|value| value.get("providers")?.as_object().map(|o| o.keys().cloned().collect()))
        .unwrap_or_default()
}

/// Where a value of the project dialog comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The user's default.
    Default,
    /// `.dango/factory.toml`.
    File,
}

impl Origin {
    /// Its words beside the value.
    pub fn words(self) -> &'static str {
        match self {
            Origin::Default => t("Vorgabe"),
            Origin::File => "factory.toml",
        }
    }
}

/// A value of the dialog "Software Factory für <Ordner>" that shows where it comes from and goes back to the default (E183).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectValue {
    /// A role's ranked list.
    Role(Role),
    /// Whether the reviewer must come from another vendor.
    ReviewerRule,
    /// A limit (E168).
    Limit(Limit),
    /// The budget of one order in dollars.
    OrderBudget,
    /// The budget of one day in dollars.
    DayBudget,
    /// The paths copied into each worktree (E188).
    Copy,
    /// The setup command of a new worktree (E188).
    Setup,
    /// The path patterns of risk class high (E163).
    RiskHigh,
    /// The path patterns of risk class medium.
    RiskMedium,
    /// More test paths whose change counts as relevant (E172).
    Tests,
    /// More lint, CI and build configuration whose change counts as relevant.
    Config,
}

impl ProjectValue {
    /// Every value, in the dialog's order.
    pub fn all() -> Vec<ProjectValue> {
        let mut all: Vec<ProjectValue> = Role::ALL.into_iter().map(ProjectValue::Role).collect();
        all.push(ProjectValue::ReviewerRule);
        all.extend(LIMITS.into_iter().map(ProjectValue::Limit));
        all.extend([
            ProjectValue::OrderBudget,
            ProjectValue::DayBudget,
            ProjectValue::Copy,
            ProjectValue::Setup,
            ProjectValue::RiskHigh,
            ProjectValue::RiskMedium,
            ProjectValue::Tests,
            ProjectValue::Config,
        ]);
        all
    }

    /// Its place in [`ProjectValue::all`], the tag of its controls.
    pub fn tag(self) -> isize {
        let index = ProjectValue::all().iter().position(|v| *v == self).unwrap_or(0);
        isize::try_from(index).unwrap_or(0)
    }

    /// The value of `tag`.
    pub fn of_tag(tag: isize) -> Option<ProjectValue> {
        ProjectValue::all().get(usize::try_from(tag).ok()?).copied()
    }

    /// Its key in the workflow file.
    pub fn key(self) -> Vec<&'static str> {
        match self {
            ProjectValue::Role(role) => vec!["roles", role.key()],
            ProjectValue::ReviewerRule => vec!["review", "rule"],
            ProjectValue::Limit(limit) => vec!["limits", limit.key()],
            ProjectValue::OrderBudget => vec!["budget", "order", "dollars"],
            ProjectValue::DayBudget => vec!["budget", "day", "dollars"],
            ProjectValue::Copy => vec!["worktree", "copy"],
            ProjectValue::Setup => vec!["worktree", "setup"],
            ProjectValue::RiskHigh => vec!["risk", "high"],
            ProjectValue::RiskMedium => vec!["risk", "medium"],
            ProjectValue::Tests => vec!["relevant", "tests"],
            ProjectValue::Config => vec!["relevant", "config"],
        }
    }

    /// The title of its row.
    pub fn title(self) -> String {
        match self {
            ProjectValue::Role(role) => role_title(role).into(),
            ProjectValue::ReviewerRule => t("Prüfer-Regel:").into(),
            ProjectValue::Limit(limit) => limit_title(limit).into(),
            ProjectValue::OrderBudget => t("Budget je Auftrag ($):").into(),
            ProjectValue::DayBudget => t("Budget je Tag ($):").into(),
            ProjectValue::Copy => t("Mitkopieren:").into(),
            ProjectValue::Setup => t("Einrichtung:").into(),
            ProjectValue::RiskHigh => t("Risikoklasse hoch:").into(),
            ProjectValue::RiskMedium => t("Risikoklasse mittel:").into(),
            ProjectValue::Tests => t("Tests:").into(),
            ProjectValue::Config => t("Konfiguration:").into(),
        }
    }

    /// Whether the dialog edits it as a list of paths or patterns, one per line or separated by commas.
    pub fn is_list(self) -> bool {
        matches!(
            self,
            ProjectValue::Copy
                | ProjectValue::RiskHigh
                | ProjectValue::RiskMedium
                | ProjectValue::Tests
                | ProjectValue::Config
        )
    }
}

/// The title of the row of `role`'s ranked list.
pub fn role_title(role: Role) -> &'static str {
    match role {
        Role::Planner => t("Planer:"),
        Role::Builder => t("Builder:"),
        Role::Reviewer => t("Prüfer:"),
        Role::Scout => t("Späher:"),
    }
}

/// The reviewer rule's choices, in their order: another vendor required, or preferred.
pub fn reviewer_rules() -> [&'static str; 2] {
    [t("Anderer Hersteller Pflicht"), t("Anderer Hersteller bevorzugt")]
}

/// The project's workflow file as the dialog "Software Factory für <Ordner>" edits it (E183).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectFile {
    text: String,
    exists: bool,
    ignored: bool,
    defaults: UserDefaults,
    /// What each agent offers for an entry, by agent.
    catalogs: Vec<(Agent, Catalog)>,
}

impl ProjectFile {
    /// The file with `text`, `None` while there is none; `ignored` whether git ignores it, `defaults` the user's.
    pub fn new(text: Option<String>, ignored: bool, defaults: UserDefaults) -> ProjectFile {
        ProjectFile {
            exists: text.is_some(),
            text: text.unwrap_or_default(),
            ignored,
            defaults,
            catalogs: Vec::new(),
        }
    }

    /// The file with `catalogs`, what each agent offers for an entry of a role.
    #[must_use]
    pub fn with_catalogs(mut self, catalogs: Vec<(Agent, Catalog)>) -> ProjectFile {
        self.catalogs = catalogs;
        self
    }

    /// What `agent` offers for an entry; nothing for an agent the file was not given.
    pub fn catalog(&self, agent: &Agent) -> Catalog {
        self.catalogs
            .iter()
            .find(|(a, _)| a == agent)
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }

    /// The text as edited.
    pub fn text(&self) -> &str {
        &self.text
    }

    fn read(&self) -> Result<Workflow, Vec<WorkflowError>> {
        parse(&self.text).map(|parsed| parsed.workflow)
    }

    /// What is wrong with the file, in the user's words; no new station starts while there is something (E190).
    pub fn errors(&self) -> Vec<String> {
        match self.read() {
            Ok(_) => Vec::new(),
            Err(errors) => errors.iter().map(words::workflow_error).collect(),
        }
    }

    /// What the dialog says about the file beside its errors: that it is new, or that git ignores it, so the team
    /// does not share it.
    pub fn notes(&self) -> Vec<&'static str> {
        let mut notes = Vec::new();
        if !self.exists {
            notes.push(t(
                "Das Projekt hat noch keine .dango/factory.toml; Sichern legt sie an.",
            ));
        }
        if self.ignored {
            notes.push(t(
                "Git ignoriert .dango/factory.toml; das Team bekommt die Datei so nicht.",
            ));
        }
        notes
    }

    /// Where `value` comes from: the file when it sets the value's key, else the user's default.
    pub fn origin(&self, value: ProjectValue) -> Origin {
        if edit::has(&self.text, &value.key()) {
            Origin::File
        } else {
            Origin::Default
        }
    }

    /// Takes `value` out of the file, so the default applies again; comments and order of the rest stay.
    pub fn reset(&mut self, value: ProjectValue) -> Result<(), Explained> {
        let result = edit::remove(&self.text, &value.key());
        self.edit(result)
    }

    /// What the file sets at `key`: from the parsed file, or, while the file does not parse, from the raw TOML value
    /// there, so a single error elsewhere neither hides the value nor lets Save write it away.
    fn own<T>(
        &self,
        key: &[&str],
        parsed: impl FnOnce(&Workflow) -> Option<T>,
        raw: impl FnOnce(&Item) -> Option<T>,
    ) -> Option<T> {
        match self.read() {
            Ok(workflow) => parsed(&workflow),
            Err(_) => self.raw(key).as_ref().and_then(raw),
        }
    }

    /// The TOML item at `key`, read without checking the rest of the file.
    fn raw(&self, key: &[&str]) -> Option<Item> {
        let doc = self.text.parse::<DocumentMut>().ok()?;
        let mut item = doc.as_item();
        for part in key {
            item = item.as_table_like()?.get(part)?;
        }
        Some(item.clone())
    }

    /// The value of `limit` and where it comes from.
    pub fn limit(&self, limit: Limit) -> (u64, Origin) {
        let own = self.own(
            &ProjectValue::Limit(limit).key(),
            |w| w.limits().iter().find(|(l, _)| *l == limit).map(|(_, v)| *v),
            |item| item.as_integer().and_then(|n| u64::try_from(n).ok()),
        );
        match own {
            Some(value) => (value, Origin::File),
            None => (self.defaults.limits().get(limit), Origin::Default),
        }
    }

    /// The gate commands as "name: command", which the file holds as a list of tables the dialog does not edit.
    pub fn gates(&self) -> Vec<String> {
        self.read()
            .map(|w| w.gates().iter().map(|g| format!("{}: {}", g.name, g.run)).collect())
            .unwrap_or_default()
    }

    /// The paths or patterns of the list `value` as the file gives them; none by default, where only the built-in
    /// patterns of each test tool apply.
    pub fn list(&self, value: ProjectValue) -> (Vec<String>, Origin) {
        let parsed = |workflow: &Workflow| {
            let relevance = workflow.relevance();
            Some(match value {
                ProjectValue::Copy => workflow.copy().to_vec(),
                ProjectValue::RiskHigh => relevance.high.clone(),
                ProjectValue::RiskMedium => relevance.medium.clone(),
                ProjectValue::Tests => relevance.tests.clone(),
                ProjectValue::Config => relevance.config.clone(),
                _ => Vec::new(),
            })
        };
        let raw = |item: &Item| {
            item.as_array()?
                .iter()
                .map(|entry| entry.as_str().map(str::to_string))
                .collect()
        };
        let list = self.own(&value.key(), parsed, raw).unwrap_or_default();
        (list, self.origin(value))
    }

    /// The paths copied into each worktree (E188).
    pub fn copy(&self) -> (Vec<String>, Origin) {
        self.list(ProjectValue::Copy)
    }

    /// The setup command (E188).
    pub fn setup(&self) -> (String, Origin) {
        let key = ProjectValue::Setup.key();
        let setup = self.own(
            &key,
            |w| w.setup().map(str::to_string),
            |item| item.as_str().map(str::to_string),
        );
        (setup.unwrap_or_default(), self.origin(ProjectValue::Setup))
    }

    /// The budget of one order in dollars and where it comes from.
    pub fn order_budget(&self) -> (Option<f64>, Origin) {
        let own = self.own(
            &ProjectValue::OrderBudget.key(),
            |w| w.budgets().order().dollars(),
            raw_dollars,
        );
        match own {
            Some(dollars) => (Some(dollars), Origin::File),
            None => (self.defaults.budgets().order().dollars(), Origin::Default),
        }
    }

    /// The budget of one day in dollars and where it comes from.
    pub fn day_budget(&self) -> (Option<f64>, Origin) {
        let own = self.own(
            &ProjectValue::DayBudget.key(),
            |w| w.budgets().day().dollars(),
            raw_dollars,
        );
        match own {
            Some(dollars) => (Some(dollars), Origin::File),
            None => (self.defaults.budgets().day().dollars(), Origin::Default),
        }
    }

    fn budget(&self, value: ProjectValue) -> (Option<f64>, Origin) {
        if value == ProjectValue::OrderBudget {
            self.order_budget()
        } else {
            self.day_budget()
        }
    }

    /// The text of `value`'s field. A budget the project does not set stays empty, with the default in the
    /// placeholder, so the field never carries the default into the file.
    pub fn field_text(&self, value: ProjectValue) -> String {
        match value {
            ProjectValue::Role(role) => self.role(role).0.join(", "),
            ProjectValue::ReviewerRule => reviewer_rules()[usize::from(self.reviewer_preferred().0)].to_string(),
            ProjectValue::Limit(limit) => self.limit(limit).0.to_string(),
            ProjectValue::OrderBudget | ProjectValue::DayBudget => match self.budget(value) {
                (Some(dollars), Origin::File) => dollars.to_string(),
                _ => String::new(),
            },
            ProjectValue::Setup => self.setup().0,
            list => self.list(list).0.join(", "),
        }
    }

    /// The placeholder of `value`'s field: for a budget what applies while the field is empty, else an example.
    pub fn placeholder(&self, value: ProjectValue) -> Option<String> {
        let default = match value {
            ProjectValue::OrderBudget => self.defaults.budgets().order().dollars(),
            ProjectValue::DayBudget => self.defaults.budgets().day().dollars(),
            ProjectValue::Copy => return Some(t("Mitkopieren, etwa node_modules, .env.local").into()),
            ProjectValue::Setup => return Some(t("Einrichtungsbefehl, etwa npm ci").into()),
            _ => return None,
        };
        Some(match default {
            Some(dollars) => tf("Vorgabe: {}", &[&words::dollars(dollars)]),
            None => t("Vorgabe: ohne Grenze").into(),
        })
    }

    /// Takes `typed` from the field of `value`, which showed `shown`; a field the user left as it was writes nothing.
    pub fn take(&mut self, value: ProjectValue, shown: &str, typed: &str) -> Result<(), Explained> {
        if typed == shown {
            return Ok(());
        }
        match value {
            ProjectValue::Limit(limit) => {
                let Ok(number) = typed.trim().parse::<u64>() else {
                    return Err(Explained(tf("„{}“ ist keine ganze Zahl.", &[typed])));
                };
                self.set_limit(limit, number)
            }
            ProjectValue::OrderBudget | ProjectValue::DayBudget => {
                let amount =
                    read_dollars(typed).map_err(|_| Explained(tf("„{}“ ist kein Betrag in Dollar.", &[typed])))?;
                self.set_dollars(value, amount)
            }
            ProjectValue::Setup => self.set_setup(typed),
            list if list.is_list() => self.set_list(list, typed),
            _ => Ok(()),
        }
    }

    /// The ranked list of `role` and where it comes from.
    pub fn ranks(&self, role: Role) -> (Vec<RankEntry>, Origin) {
        let own = self.read().ok().and_then(|w| w.role(role).map(<[RankEntry]>::to_vec));
        match own {
            Some(list) => (list, Origin::File),
            None => (self.defaults.role(role).to_vec(), Origin::Default),
        }
    }

    /// The ranked list of `role` in words, each entry with its agent's default model by name, and where it comes
    /// from.
    pub fn role(&self, role: Role) -> (Vec<String>, Origin) {
        let (list, origin) = self.ranks(role);
        let words = list
            .iter()
            .map(|e| ranks::entry_words(e, role, &self.catalog(&e.agent)))
            .collect();
        (words, origin)
    }

    /// Writes `list` as the ranked list of `role`; an empty one takes it out, so the default applies again.
    pub fn set_ranks(&mut self, role: Role, list: &[RankEntry]) -> Result<(), Explained> {
        if list.is_empty() {
            return self.reset(ProjectValue::Role(role));
        }
        let tables = list
            .iter()
            .map(|entry| {
                let mut pairs = vec![("agent".to_string(), entry.agent.to_string())];
                pairs.extend(entry.model.clone().map(|m| ("model".to_string(), m)));
                pairs.extend(entry.effort.clone().map(|e| ("effort".to_string(), e)));
                pairs
            })
            .collect();
        let result = edit::set(&self.text, &ProjectValue::Role(role).key(), EditValue::Tables(tables));
        self.edit(result)
    }

    /// The builder's ranked list and where it comes from.
    pub fn builders(&self) -> (Vec<String>, Origin) {
        self.role(Role::Builder)
    }

    /// Whether the rule lets a reviewer of the builders' vendor review when no other is ready, and where it comes
    /// from; "preferred" in the file applies only once the user confirmed it (E190).
    pub fn reviewer_preferred(&self) -> (bool, Origin) {
        let own = self.own(
            &ProjectValue::ReviewerRule.key(),
            Workflow::reviewer_preferred,
            |item| match item.as_str()? {
                "preferred" => Some(true),
                "required" => Some(false),
                _ => None,
            },
        );
        match own {
            Some(preferred) => (preferred, Origin::File),
            None => (
                self.defaults.reviewer_rule() == ReviewerRule::Preferred,
                Origin::Default,
            ),
        }
    }

    fn edit(&mut self, result: Result<String, EditError>) -> Result<(), Explained> {
        match result {
            Ok(text) => {
                self.text = text;
                Ok(())
            }
            Err(EditError::Invalid(errors)) => Err(Explained(
                errors.iter().map(words::workflow_error).collect::<Vec<_>>().join(" "),
            )),
            Err(EditError::NotATable { key }) => Err(Explained(tf("„{}“ ist in der Datei keine Tabelle.", &[&key]))),
            Err(EditError::EmptyKey) => Err(Explained(String::new())),
        }
    }

    /// Writes `value` for `limit` into the file.
    pub fn set_limit(&mut self, limit: Limit, value: u64) -> Result<(), Explained> {
        let result = edit::set(&self.text, &["limits", limit.key()], EditValue::Count(value));
        self.edit(result)
    }

    /// Takes `limit` out of the file, so the default applies again.
    pub fn reset_limit(&mut self, limit: Limit) -> Result<(), Explained> {
        self.reset(ProjectValue::Limit(limit))
    }

    /// Writes the list `value` from `text`, one entry per line or separated by commas; none takes the key out.
    pub fn set_list(&mut self, value: ProjectValue, text: &str) -> Result<(), Explained> {
        let entries: Vec<String> = text
            .split(['\n', ','])
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        if entries.is_empty() {
            return self.reset(value);
        }
        if entries == self.list(value).0 {
            return Ok(());
        }
        let result = edit::set(&self.text, &value.key(), EditValue::Texts(entries));
        self.edit(result)
    }

    /// Writes the paths to copy, one per line or separated by commas; none takes the key out.
    pub fn set_copy(&mut self, text: &str) -> Result<(), Explained> {
        self.set_list(ProjectValue::Copy, text)
    }

    /// Writes the setup command; an empty one takes the key out.
    pub fn set_setup(&mut self, command: &str) -> Result<(), Explained> {
        let command = command.trim();
        if command.is_empty() {
            return self.reset(ProjectValue::Setup);
        }
        if command == self.setup().0 {
            return Ok(());
        }
        let result = edit::set(&self.text, &["worktree", "setup"], EditValue::Text(command.to_string()));
        self.edit(result)
    }

    /// Writes the order budget in dollars; `None` takes it out.
    pub fn set_order_budget(&mut self, dollars: Option<f64>) -> Result<(), Explained> {
        self.set_dollars(ProjectValue::OrderBudget, dollars)
    }

    /// Writes the day budget in dollars; `None` takes it out.
    pub fn set_day_budget(&mut self, dollars: Option<f64>) -> Result<(), Explained> {
        self.set_dollars(ProjectValue::DayBudget, dollars)
    }

    fn set_dollars(&mut self, value: ProjectValue, dollars: Option<f64>) -> Result<(), Explained> {
        let result = match dollars {
            Some(dollars) => edit::set(&self.text, &value.key(), EditValue::Number(dollars)),
            None => edit::remove(&self.text, &value.key()),
        };
        self.edit(result)
    }

    /// Writes the reviewer rule: another vendor preferred, or required.
    pub fn set_reviewer_preferred(&mut self, preferred: bool) -> Result<(), Explained> {
        let word = if preferred { "preferred" } else { "required" };
        let result = edit::set(&self.text, &["review", "rule"], EditValue::Text(word.into()));
        self.edit(result)
    }
}

/// The dollars of a budget as the raw TOML value gives them.
#[expect(clippy::cast_precision_loss, reason = "budgets are far below 2^52")]
fn raw_dollars(item: &Item) -> Option<f64> {
    item.as_float()
        .or_else(|| item.as_integer().map(|n| n as f64))
        .filter(|n| *n >= 0.0)
}

/// A field that holds no amount of dollars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAnAmount;

/// A number of dollars as typed, with a comma or a point; `None` for an empty field.
pub fn read_dollars(text: &str) -> Result<Option<f64>, NotAnAmount> {
    let text = text.trim().replace(',', ".");
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<f64>() {
        Ok(value) if value.is_finite() && value > 0.0 => Ok(Some(value)),
        _ => Err(NotAnAmount),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_limit_in_the_file_shows_its_origin_and_resets_to_the_default() {
        let mut file = ProjectFile::new(
            Some("[limits]\nturn_minutes = 30\n".into()),
            false,
            UserDefaults::default(),
        );
        assert_eq!(file.limit(Limit::TurnMinutes), (30, Origin::File));
        file.reset_limit(Limit::TurnMinutes).unwrap();
        assert_eq!(file.limit(Limit::TurnMinutes), (60, Origin::Default));
        file.set_limit(Limit::Attempts, 2).unwrap();
        assert_eq!(file.limit(Limit::Attempts), (2, Origin::File));
    }

    const FILE: &str = "\
[[roles.reviewer]]
agent = \"claude\"

# Our rules.
[review]
rule = \"preferred\" # the team agreed

[risk]
# Payment code.
high = [\"src/pay/**\"]
";

    #[test]
    fn every_value_shows_its_origin_and_a_reset_keeps_the_rest_of_the_file() {
        let mut file = ProjectFile::new(Some(FILE.into()), false, UserDefaults::default());
        assert_eq!(file.origin(ProjectValue::Role(Role::Reviewer)), Origin::File);
        assert_eq!(file.origin(ProjectValue::Role(Role::Builder)), Origin::Default);
        assert_eq!(file.reviewer_preferred(), (true, Origin::File));
        assert_eq!(
            file.list(ProjectValue::RiskHigh),
            (vec!["src/pay/**".to_string()], Origin::File)
        );
        assert_eq!(file.list(ProjectValue::Tests), (Vec::new(), Origin::Default));
        file.reset(ProjectValue::Role(Role::Reviewer)).unwrap();
        file.reset(ProjectValue::ReviewerRule).unwrap();
        assert_eq!(file.origin(ProjectValue::Role(Role::Reviewer)), Origin::Default);
        assert_eq!(file.reviewer_preferred(), (false, Origin::Default));
        assert!(file.text().contains("# Payment code.") && file.text().contains("# Our rules."));
        file.set_list(ProjectValue::Tests, "spec/**, e2e/**").unwrap();
        assert_eq!(file.origin(ProjectValue::Tests), Origin::File);
        file.set_list(ProjectValue::RiskHigh, "").unwrap();
        assert_eq!(file.origin(ProjectValue::RiskHigh), Origin::Default);
        assert!(file.errors().is_empty(), "{:?}", file.errors());
    }

    /// Shows every field of `file`, lets `edit` change what the user types, and takes every field back as Save does.
    fn save(file: &mut ProjectFile, edit: impl Fn(ProjectValue, &str) -> String) {
        let shown: Vec<(ProjectValue, String)> = ProjectValue::all()
            .into_iter()
            .map(|v| (v, file.field_text(v)))
            .collect();
        for (value, text) in shown {
            file.take(value, &text, &edit(value, &text)).unwrap();
        }
    }

    #[test]
    fn saving_a_file_with_an_error_keeps_the_values_the_user_did_not_touch() {
        let text = "[limits]\nbogus = 1\nattempts = 2\n\n[risk]\n# Payment code.\nhigh = [\"src/pay/**\"]\n\n[worktree]\nsetup = \"npm ci\"\n";
        let mut file = ProjectFile::new(Some(text.into()), false, UserDefaults::default());
        assert!(!file.errors().is_empty(), "the unknown key is an error of the file");
        assert_eq!(file.field_text(ProjectValue::RiskHigh), "src/pay/**");
        assert_eq!(file.field_text(ProjectValue::Setup), "npm ci");
        assert_eq!(file.limit(Limit::Attempts), (2, Origin::File));
        save(&mut file, |value, text| match value {
            ProjectValue::Limit(Limit::TurnMinutes) => "30".into(),
            _ => text.to_string(),
        });
        assert!(file.text().contains("high = [\"src/pay/**\"]"), "{}", file.text());
        assert!(file.text().contains("# Payment code.") && file.text().contains("setup = \"npm ci\""));
        assert!(file.text().contains("bogus = 1") && file.text().contains("attempts = 2"));
        assert_eq!(file.limit(Limit::TurnMinutes), (30, Origin::File));
    }

    #[test]
    fn an_untouched_budget_writes_nothing_and_an_emptied_one_leaves_the_default() {
        let mut form = DefaultsForm::default();
        form.set_budgets(Some(2.345), None);
        let defaults = form.defaults().clone();
        let mut file = ProjectFile::new(Some("[budget.day]\ndollars = 7.125\n".into()), false, defaults);
        assert_eq!(file.field_text(ProjectValue::OrderBudget), "");
        assert_eq!(file.order_budget(), (Some(2.345), Origin::Default));
        assert!(file.placeholder(ProjectValue::OrderBudget).is_some());
        assert_eq!(file.field_text(ProjectValue::DayBudget), "7.125");
        let before = file.text().to_string();
        save(&mut file, |_, text| text.to_string());
        assert_eq!(file.text(), before, "nothing the user left alone is written");
        save(&mut file, |value, text| match value {
            ProjectValue::DayBudget => String::new(),
            _ => text.to_string(),
        });
        assert_eq!(file.day_budget(), (None, Origin::Default));
        assert!(!file.text().contains("dollars"), "{}", file.text());
    }

    #[test]
    fn a_bad_pattern_shows_as_an_error_of_the_file() {
        let mut file = ProjectFile::new(None, false, UserDefaults::default());
        file.set_list(ProjectValue::Config, "ci/[").unwrap();
        assert_eq!(file.errors().len(), 1);
    }

    #[test]
    fn values_come_back_from_their_tags() {
        for value in ProjectValue::all() {
            assert_eq!(ProjectValue::of_tag(value.tag()), Some(value));
        }
    }

    #[test]
    fn effort_reviewer_rule_and_provider_budgets_change_the_defaults() {
        let mut form = DefaultsForm::default();
        form.set_effort(Role::Reviewer, 0, " high ");
        assert_eq!(form.list(Role::Reviewer)[0].effort.as_deref(), Some("high"));
        form.set_reviewer_preferred(true);
        assert!(form.reviewer_preferred());
        let service = api_service(&Agent::Claude);
        form.set_provider_budget(&service, Some(5.0));
        form.set_budgets(Some(2.0), None);
        assert_eq!(form.provider_budget(&service), Some(5.0));
        form.set_provider_budget(&service, None);
        assert_eq!(form.provider_budget(&service), None);
        assert_eq!(form.budgets(), (Some(2.0), None));
    }

    #[test]
    fn the_ranked_list_moves_and_keeps_one_entry_per_agent() {
        let mut form = DefaultsForm::default();
        let first = form.list(Role::Builder)[0].agent.clone();
        assert_eq!(form.shift(Role::Builder, 0, false), 1);
        assert_eq!(form.list(Role::Builder)[1].agent, first);
        let len = form.list(Role::Builder).len();
        form.add(Role::Builder, first);
        assert_eq!(form.list(Role::Builder).len(), len);
    }

    #[test]
    fn a_ranked_list_edited_in_the_dialog_goes_into_the_file_and_an_empty_one_leaves_the_default() {
        let mut file = ProjectFile::new(Some(FILE.into()), false, UserDefaults::default());
        let (mut list, origin) = file.ranks(Role::Planner);
        assert_eq!(origin, Origin::Default);
        ranks::add(&mut list, Agent::Codex);
        let codex = list.iter().position(|e| e.agent == Agent::Codex).unwrap();
        ranks::set_model(&mut list, codex, "gpt-a");
        ranks::set_effort(&mut list, codex, "high");
        file.set_ranks(Role::Planner, &list).unwrap();
        assert_eq!(file.ranks(Role::Planner), (list, Origin::File));
        assert!(file.text().contains("# Payment code."), "{}", file.text());
        assert!(file.errors().is_empty(), "{:?}", file.errors());
        file.set_ranks(Role::Planner, &[]).unwrap();
        assert_eq!(file.origin(ProjectValue::Role(Role::Planner)), Origin::Default);
    }

    #[test]
    fn dollars_read_with_a_comma_or_a_point() {
        assert_eq!(read_dollars("2,5"), Ok(Some(2.5)));
        assert_eq!(read_dollars(" "), Ok(None));
        assert_eq!(read_dollars("x"), Err(NotAnAmount));
    }
}
