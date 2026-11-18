//! What the self-test reads from and does with the parts of `acp.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// The register the self-test uses instead of the one last fetched; process-wide, since lookups run on any thread.
pub(super) static TEST_REGISTER: std::sync::Mutex<Option<Register>> = std::sync::Mutex::new(None);

/// Uses `register` as the one last fetched, so the self-test needs no network.
pub fn set_test_register(register: Register) {
    *TEST_REGISTER.lock_ignoring_poison() = Some(register);
}

/// The launch every ACP tab gets in the self-test; process-wide, since agents are looked up on other threads.
pub(super) static TEST_LAUNCH: std::sync::Mutex<Option<Option<Launch>>> = std::sync::Mutex::new(None);

/// Starts every ACP tab with `launch`, as the self-test runs its test agent; with `None` no agent is found at all,
/// so the self-test never starts an agent installed on the Mac.
pub fn set_test_launch(launch: Option<Launch>) {
    *TEST_LAUNCH.lock_ignoring_poison() = Some(launch);
}

impl AgentTab {
    /// What the ACP agent offers, as the tab last learned it.
    pub fn acp_capabilities(&self) -> Option<ide_acp::Capabilities> {
        self.state.borrow().acp_caps().cloned()
    }

    /// The titles of the model and mode pills and the sandbox switch's explanation.
    pub fn pills(&self) -> (String, String, String) {
        (
            self.composer.model.title().to_string(),
            self.composer.mode.title().to_string(),
            self.composer
                .sandbox
                .toolTip()
                .map(|t| t.to_string())
                .unwrap_or_default(),
        )
    }

    /// What keeps the session from running, by name.
    pub fn problem_name(&self) -> Option<&'static str> {
        self.state.borrow().problem().map(|p| match p {
            Problem::Untrusted => "untrusted",
            Problem::NotFound => "not found",
            Problem::SignIn(_) | Problem::AcpSignIn => "sign in",
            Problem::Start(_) => "start",
            Problem::Missing(_) => "missing",
            Problem::Installing(_) => "installing",
            Problem::NoModel => "no model",
            Problem::Exited(_) => "exited",
        })
    }

    /// Sets the agent's option `option` to `value`, as picking it in the model menu does.
    pub fn choose_option(&self, option: &str, value: &str) {
        self.acp_set_option(option, value);
    }

    /// Switches the agent to mode `id`, as picking it in the mode menu does.
    pub fn choose_mode(&self, id: &str) {
        self.acp_set_mode(id.to_string());
    }

    /// The agent's name, as the tab shows it.
    pub fn agent_name(&self) -> String {
        self.agent.name().to_string()
    }

    /// How many checkpoints before prompts belong to turns that have ended.
    pub fn finished_checkpoints(&self) -> usize {
        self.state
            .borrow()
            .checkpoints()
            .values()
            .filter(|c| c.finished)
            .count()
    }

    /// What the rewind menu of user message `id` offers, by title.
    pub fn rewind_titles(&self, id: u64) -> Vec<String> {
        let Some(entry) = self.entry(id) else { return Vec::new() };
        let EntryKind::UserMessage(message) = &entry.kind else {
            return Vec::new();
        };
        let Some(uuid) = message.message_id.clone() else {
            return Vec::new();
        };
        self.rewind_choices(&entry, &uuid)
            .into_iter()
            .map(|(title, _, _)| title.to_string())
            .collect()
    }
}
