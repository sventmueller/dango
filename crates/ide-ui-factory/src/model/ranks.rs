//! A role's ranked list as the settings tab and the project dialog edit it: one entry per agent, moved up and down,
//! each with its model and effort, and the words of an entry with the model its agent takes when it names none.

use ide_factory::models::Catalog;
use ide_factory::routing::{Agent, RankEntry};
use ide_factory::station::Role;
use splat_i18n::{t, tf};

use super::settings::{agent_name, takes};
use super::words;

/// Adds `agent` at the end of `list`, unless it is in it already.
pub fn add(list: &mut Vec<RankEntry>, agent: Agent) {
    if !list.iter().any(|e| e.agent == agent) {
        list.push(RankEntry::new(agent));
    }
}

/// Removes the entry at `index`.
pub fn remove(list: &mut Vec<RankEntry>, index: usize) {
    if index < list.len() {
        list.remove(index);
    }
}

/// Moves the entry at `index` one place up, or down; where it went.
pub fn shift(list: &mut [RankEntry], index: usize, up: bool) -> usize {
    let to = if up { index.checked_sub(1) } else { Some(index + 1) };
    let Some(to) = to.filter(|to| *to < list.len() && index < list.len()) else {
        return index;
    };
    list.swap(index, to);
    to
}

/// Sets the model of the entry at `index`; empty for the agent's default.
pub fn set_model(list: &mut [RankEntry], index: usize, model: &str) {
    if let Some(entry) = list.get_mut(index) {
        entry.model = named(model);
    }
}

/// Sets the reasoning effort of the entry at `index`; empty for the agent's default.
pub fn set_effort(list: &mut [RankEntry], index: usize, effort: &str) {
    if let Some(entry) = list.get_mut(index) {
        entry.effort = named(effort);
    }
}

fn named(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// What a model or effort field shows while the entry names none: the agent's default, by name where its settings
/// give one.
pub fn default_words(default: Option<&str>) -> String {
    match default {
        Some(name) => tf("Vorgabe des Agenten ({})", &[name]),
        None => t("Vorgabe des Agenten").to_string(),
    }
}

/// An entry as its row reads: "Codex · gpt-5-codex · high", the agent's default model by name where `catalog` knows
/// it, and why it cannot take the role.
pub fn entry_words(entry: &RankEntry, role: Role, catalog: &Catalog) -> String {
    let model = entry
        .model
        .clone()
        .unwrap_or_else(|| default_words(catalog.default_model.as_deref()));
    let mut words = format!("{} · {model}", agent_name(&entry.agent));
    if let Some(effort) = &entry.effort {
        words = format!("{words} · {effort}");
    }
    if !takes(entry, role) {
        words = tf("{} – kann nicht {} sein", &[&words, words::role(role)]);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_keeps_one_entry_per_agent_and_moves_within_its_ends() {
        let mut list = vec![RankEntry::new(Agent::Claude)];
        add(&mut list, Agent::Codex);
        add(&mut list, Agent::Claude);
        assert_eq!(list.len(), 2);
        assert_eq!(shift(&mut list, 1, true), 0);
        assert_eq!(list[0].agent, Agent::Codex);
        assert_eq!(shift(&mut list, 1, false), 1);
        set_model(&mut list, 0, " gpt-a ");
        set_effort(&mut list, 0, "");
        assert_eq!(
            (list[0].model.as_deref(), list[0].effort.as_deref()),
            (Some("gpt-a"), None)
        );
        remove(&mut list, 0);
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn an_entry_without_a_model_names_its_agents_default_where_known() {
        let entry = RankEntry::new(Agent::Claude);
        let known = Catalog {
            default_model: Some("opus".into()),
            ..Catalog::default()
        };
        assert!(entry_words(&entry, Role::Builder, &known).contains("opus"));
        let unknown = entry_words(&entry, Role::Builder, &Catalog::default());
        assert!(unknown.ends_with(t("Vorgabe des Agenten")), "{unknown}");
    }
}
