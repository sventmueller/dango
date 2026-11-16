//! The Leitstand's settings (E183): the user's defaults for all projects in "Einstellungen ▸ Aufträge", and the
//! project's workflow file in "Leitstand für <Ordner>", each value with where it comes from and a way back to the
//! default. Stage 1 offers the builder, limits, budgets, gates and copying (E183's stages).

use std::collections::BTreeMap;

use ide_core::explain::Explained;
use ide_factory::budget::{Budget, Budgets};
use ide_factory::limits::Limit;
use ide_factory::routing::{Agent, Capabilities, RankEntry};
use ide_factory::station::Role;
use ide_factory::workflow::edit::{self, EditError, EditValue};
use ide_factory::workflow::{UserDefaults, Workflow, WorkflowError, parse};
use splat_i18n::{t, tf};

use super::words;

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

/// An entry of a ranked list as its row reads: "Codex · gpt-5-codex · high", with why it cannot take the role.
pub fn entry_words(entry: &RankEntry, role: Role) -> String {
    let model = entry.model.as_deref().unwrap_or(t("Vorgabe des Agenten"));
    let mut words = format!("{} · {model}", agent_name(&entry.agent));
    if let Some(effort) = &entry.effort {
        words = format!("{words} · {effort}");
    }
    if !Capabilities::of(&entry.agent).takes(role) {
        words = tf("{} – kann nicht {} sein", &[&words, words::role(role)]);
    }
    words
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

    /// Adds `agent` at the end of `role`'s list, unless it is in it already.
    pub fn add(&mut self, role: Role, agent: Agent) {
        let mut list = self.list(role).to_vec();
        if !list.iter().any(|e| e.agent == agent) {
            list.push(RankEntry::new(agent));
        }
        self.defaults.set_role(role, list);
    }

    /// Removes the entry at `index` of `role`'s list.
    pub fn remove(&mut self, role: Role, index: usize) {
        let mut list = self.list(role).to_vec();
        if index < list.len() {
            list.remove(index);
        }
        self.defaults.set_role(role, list);
    }

    /// Moves the entry at `index` of `role`'s list one place up, or down; where it went.
    pub fn shift(&mut self, role: Role, index: usize, up: bool) -> usize {
        let mut list = self.list(role).to_vec();
        let to = if up { index.checked_sub(1) } else { Some(index + 1) };
        let Some(to) = to.filter(|to| *to < list.len() && index < list.len()) else {
            return index;
        };
        list.swap(index, to);
        self.defaults.set_role(role, list);
        to
    }

    /// Sets the model of the entry at `index`; empty for the agent's default.
    pub fn set_model(&mut self, role: Role, index: usize, model: &str) {
        let mut list = self.list(role).to_vec();
        if let Some(entry) = list.get_mut(index) {
            let model = model.trim();
            entry.model = (!model.is_empty()).then(|| model.to_string());
        }
        self.defaults.set_role(role, list);
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

/// The project's workflow file as the dialog "Leitstand für <Ordner>" edits it (E183).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectFile {
    text: String,
    exists: bool,
    ignored: bool,
    defaults: UserDefaults,
}

impl ProjectFile {
    /// The file with `text`, `None` while there is none; `ignored` whether git ignores it, `defaults` the user's.
    pub fn new(text: Option<String>, ignored: bool, defaults: UserDefaults) -> ProjectFile {
        ProjectFile {
            exists: text.is_some(),
            text: text.unwrap_or_default(),
            ignored,
            defaults,
        }
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

    /// The value of `limit` and where it comes from.
    pub fn limit(&self, limit: Limit) -> (u64, Origin) {
        let own = self
            .read()
            .ok()
            .and_then(|w| w.limits().iter().find(|(l, _)| *l == limit).map(|(_, v)| *v));
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

    /// The paths copied into each worktree (E188).
    pub fn copy(&self) -> (Vec<String>, Origin) {
        match self.read() {
            Ok(w) if !w.copy().is_empty() => (w.copy().to_vec(), Origin::File),
            _ => (Vec::new(), Origin::Default),
        }
    }

    /// The setup command (E188).
    pub fn setup(&self) -> (String, Origin) {
        match self.read().ok().and_then(|w| w.setup().map(str::to_string)) {
            Some(setup) => (setup, Origin::File),
            None => (String::new(), Origin::Default),
        }
    }

    /// The budget of one order in dollars and where it comes from.
    pub fn order_budget(&self) -> (Option<f64>, Origin) {
        match self.read().ok().and_then(|w| w.budgets().order().dollars()) {
            Some(dollars) => (Some(dollars), Origin::File),
            None => (self.defaults.budgets().order().dollars(), Origin::Default),
        }
    }

    /// The builder's ranked list and where it comes from.
    pub fn builders(&self) -> (Vec<String>, Origin) {
        let own = self
            .read()
            .ok()
            .and_then(|w| w.role(Role::Builder).map(<[RankEntry]>::to_vec));
        let (list, origin) = match own {
            Some(list) => (list, Origin::File),
            None => (self.defaults.role(Role::Builder).to_vec(), Origin::Default),
        };
        (list.iter().map(|e| entry_words(e, Role::Builder)).collect(), origin)
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
        let result = edit::remove(&self.text, &["limits", limit.key()]);
        self.edit(result)
    }

    /// Writes the paths to copy, one per line or separated by commas; none takes the key out.
    pub fn set_copy(&mut self, text: &str) -> Result<(), Explained> {
        let paths: Vec<String> = text
            .split(['\n', ','])
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        let result = if paths.is_empty() {
            edit::remove(&self.text, &["worktree", "copy"])
        } else {
            edit::set(&self.text, &["worktree", "copy"], EditValue::Texts(paths))
        };
        self.edit(result)
    }

    /// Writes the setup command; an empty one takes the key out.
    pub fn set_setup(&mut self, command: &str) -> Result<(), Explained> {
        let command = command.trim();
        let result = if command.is_empty() {
            edit::remove(&self.text, &["worktree", "setup"])
        } else {
            edit::set(&self.text, &["worktree", "setup"], EditValue::Text(command.to_string()))
        };
        self.edit(result)
    }

    /// Writes the order budget in dollars; `None` takes it out.
    pub fn set_order_budget(&mut self, dollars: Option<f64>) -> Result<(), Explained> {
        let key = ["budget", "order", "dollars"];
        let result = match dollars {
            Some(dollars) => edit::set(&self.text, &key, EditValue::Number(dollars)),
            None => edit::remove(&self.text, &key),
        };
        self.edit(result)
    }
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
    fn dollars_read_with_a_comma_or_a_point() {
        assert_eq!(read_dollars("2,5"), Ok(Some(2.5)));
        assert_eq!(read_dollars(" "), Ok(None));
        assert_eq!(read_dollars("x"), Err(NotAnAmount));
    }
}
