//! What the user picked last for each agent: model, reasoning effort and permission mode, which the agent's next
//! sessions start with, also those that come back after a restart.

use ide_core::settings::{AgentChoice, State, remembered};

/// What the user picked last for the agent with the id `agent`.
pub(crate) fn of(agent: &str) -> AgentChoice {
    State::load_user()
        .get(&remembered::AGENT_CHOICES)
        .remove(agent)
        .unwrap_or_default()
}

/// Keeps what `change` sets for the agent with the id `agent`.
pub(crate) fn remember(agent: &str, change: impl FnOnce(&mut AgentChoice)) {
    let mut state = State::load_user();
    let mut choices = state.get(&remembered::AGENT_CHOICES);
    change(choices.entry(agent.to_string()).or_default());
    if let Err(error) = state.set(&remembered::AGENT_CHOICES, &choices) {
        ide_ui_common::diag::report("remembering an agent's model and mode", &error);
    }
}
