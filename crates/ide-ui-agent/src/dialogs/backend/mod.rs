//! Where the overlays get their data and send their changes: one backend per kind of agent (E146). A backend maps its
//! protocol onto the neutral model and has no views; an overlay never asks which agent it serves, only what the
//! backend can do. What a backend leaves out answers that this agent cannot do it.

mod acp;
pub(crate) mod acp_mcp;
mod claude;
mod claude_map;
mod claude_plugins;
mod codex;
mod opencode;

pub use claude::remove_claude_rule;

use ide_core::explain::Explained;

use super::model::{
    Account, Aside, Behavior, Blocked, Exported, FeedbackKind, Hook, HookDraft, Hooks, MarketChange, McpCaps, McpDraft,
    McpServer, Memory, PermissionCaps, PermissionRule, Permissions, PluginChange, PluginStep, Plugins, Scope, Settings,
    Skill, StatusSection,
};
use crate::acp::Agent;
use crate::tab::AgentTab;

/// How an answer comes back: on a worker thread, with what went wrong in the user's words.
pub(crate) type Done<T> = Box<dyn FnOnce(Result<T, Explained>) + Send>;

/// What an agent's overlays can ask.
pub(crate) trait Backend {
    /// Whether its session runs, which the overlays that ask the session need.
    fn live(&self) -> bool;

    /// The rows of `/status`.
    fn status(&self, done: Done<Vec<StatusSection>>) {
        done(Err(unavailable()));
    }

    /// The conversation as text.
    fn export(&self, done: Done<Exported>) {
        done(Err(unavailable()));
    }

    /// The plugins and marketplaces; `available` also the plugins the marketplaces offer.
    fn plugins(&self, done: Done<Plugins>) {
        done(Err(unavailable()));
    }

    /// Installs the plugin `id` in `scope`, or updates it where `update`; `accept` is the hash of the marketplace's
    /// command the user saw and accepted.
    fn plugin_install(&self, _id: &str, _scope: Scope, _update: bool, _accept: Option<String>, done: Done<PluginStep>) {
        done(Err(unavailable()));
    }

    /// Changes the plugin `id` installed in `scope`.
    fn plugin_change(&self, _id: &str, _scope: Scope, _change: PluginChange, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Changes the marketplace `name`, or adds one from the source `name`.
    fn market_change(&self, _name: &str, _change: MarketChange, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Who is signed in.
    fn account(&self, done: Done<Account>) {
        done(Err(unavailable()));
    }

    /// Signs out of the agent.
    fn sign_out(&self, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// What a report sends and to whom, which the user confirms first; none where the agent takes no reports.
    fn feedback_sends(&self) -> Option<&'static str> {
        None
    }

    /// Sends a report; what became of it, in the user's words.
    fn send_feedback(&self, _kind: FeedbackKind, _title: &str, _description: &str, done: Done<String>) {
        done(Err(unavailable()));
    }

    /// Sets the setting `key` to the value `value`, one of its choices.
    fn set_setting(&self, _key: &str, _value: &str, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Asks `question` beside the conversation, after the questions `before`.
    fn aside(&self, _question: &str, _before: &[Aside], done: Done<Aside>) {
        done(Err(unavailable()));
    }

    /// What `/mcp` can do.
    fn mcp_caps(&self) -> McpCaps {
        let no = cannot();
        McpCaps {
            live: no,
            toggle: no,
            reconnect: no,
            sign_in: no,
            add: no,
            remove: no,
            transports: Vec::new(),
            scopes: Vec::new(),
        }
    }

    /// The MCP servers.
    fn mcp_servers(&self, done: Done<Vec<McpServer>>) {
        done(Err(unavailable()));
    }

    /// Switches the server `name` on or off.
    fn mcp_toggle(&self, _name: &str, _enabled: bool, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Connects the server `name` again.
    fn mcp_reconnect(&self, _name: &str, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Starts the sign-in to the server `name`; the address to open in the browser, if any.
    fn mcp_sign_in(&self, _name: &str, done: Done<Option<String>>) {
        done(Err(unavailable()));
    }

    /// Hands over the address the browser landed on after a sign-in to the server `name`.
    fn mcp_sign_in_answer(&self, _name: &str, _url: &str, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Forgets the sign-in to the server `name`.
    fn mcp_sign_out(&self, _name: &str, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Adds the server `draft` describes.
    fn mcp_add(&self, _draft: McpDraft, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Removes the server `name` from `scope`.
    fn mcp_remove(&self, _name: &str, _scope: Scope, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// What `/permissions` can do.
    fn permission_caps(&self) -> PermissionCaps {
        PermissionCaps {
            add: cannot(),
            remove: cannot(),
            scopes: Vec::new(),
        }
    }

    /// The permission rules and extra folders.
    fn permissions(&self, done: Done<Permissions>) {
        done(Err(unavailable()));
    }

    /// Adds `rule` with `behavior` in `scope`.
    fn rule_add(&self, _rule: &str, _behavior: Behavior, _scope: Scope, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Removes `rule`.
    fn rule_remove(&self, _rule: &PermissionRule, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Why hooks cannot be added or changed here; none when they can.
    fn hooks_blocked(&self) -> Blocked {
        cannot()
    }

    /// The hooks.
    fn hooks(&self, done: Done<Hooks>) {
        done(Err(unavailable()));
    }

    /// Adds the hook `draft` describes.
    fn hook_add(&self, _draft: HookDraft, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Puts the hook `draft` describes in the place of `old`.
    fn hook_replace(&self, _old: &Hook, _draft: HookDraft, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// Removes `hook`.
    fn hook_remove(&self, _hook: &Hook, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// The instruction files and memory.
    fn memory(&self, done: Done<Memory>) {
        done(Err(unavailable()));
    }

    /// Switches automatic memory on or off.
    fn set_auto_memory(&self, _on: bool, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// The settings in effect.
    fn settings(&self, done: Done<Settings>) {
        done(Err(unavailable()));
    }

    /// Sets the output style.
    fn set_style(&self, _style: &str, done: Done<()>) {
        done(Err(unavailable()));
    }

    /// The skills.
    fn skills(&self, done: Done<Vec<Skill>>) {
        done(Err(unavailable()));
    }

    /// Switches `skill` on or off.
    fn set_skill(&self, _skill: &Skill, _on: bool, done: Done<()>) {
        done(Err(unavailable()));
    }
}

/// The reason of an action the agent cannot do.
pub(crate) fn cannot() -> Blocked {
    Some(splat_i18n::t("Dieser Agent kann das hier nicht."))
}

/// The backend of `tab`'s agent.
pub(crate) fn of(tab: &AgentTab) -> Box<dyn Backend> {
    match tab.agent {
        Agent::Claude => Box::new(claude::Claude::new(tab)),
        Agent::Acp(ref agent) => {
            let (codex, opencode) = {
                let state = tab.state.borrow();
                (state.source().codex().cloned(), state.source().opencode().cloned())
            };
            match (codex, opencode) {
                (Some(session), _) => Box::new(codex::Codex::new(tab, session)),
                (None, Some(session)) => Box::new(opencode::OpenCode::new(tab, session)),
                (None, None) => Box::new(acp::Acp::new(tab, &agent.name)),
            }
        }
        Agent::Own => Box::new(Unavailable),
    }
}

/// An agent whose overlays are not built; its commands are not offered, so this answers only a stray call.
struct Unavailable;

impl Backend for Unavailable {
    fn live(&self) -> bool {
        false
    }
}

pub(crate) fn unavailable() -> Explained {
    Explained(splat_i18n::t("Dieser Agent kann das hier nicht.").to_string())
}

/// Runs `work` on the work lane and answers through `done`, also when the lane is full or `work` panics.
pub(super) fn on_work<T: Send + 'static>(work: impl FnOnce() -> Result<T, Explained> + Send + 'static, done: Done<T>) {
    let slot = std::sync::Arc::new(std::sync::Mutex::new(Some(done)));
    let answering = slot.clone();
    let asked = ide_core::lane::WORK.ask(work, move |answer| {
        if let Some(done) = ide_core::LockIgnoringPoison::lock_ignoring_poison(&*answering).take() {
            done(answer.unwrap_or_else(|| Err(unavailable())));
        }
    });
    if asked.is_err()
        && let Some(done) = ide_core::LockIgnoringPoison::lock_ignoring_poison(&*slot).take()
    {
        done(Err(Explained(
            splat_i18n::t("Die App ist gerade ausgelastet. Versuch es gleich noch einmal.").to_string(),
        )));
    }
}
