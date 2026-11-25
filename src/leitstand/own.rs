//! The own agent in orders (E171, E173): ready when an endpoint with a model exists, as a new thread of the AI tab
//! would start with it; its vendor that of the endpoint's provider, or of its model where the endpoint serves many;
//! and what a session of an order starts with in the order's worktree. Reading the settings and the keychain blocks,
//! so all of it runs off the main thread.

use std::path::Path;
use std::sync::Arc;

use ide_agent::{Config, ModelChoice, ProviderKind};
use ide_core::explain::Explained;
use ide_factory::routing::{Agent, AgentStatus, RankEntry, Readiness, Vendor, vendor_of_model};
use ide_thread::ThreadId;
use ide_ui_agent::own_agent;

/// An order's worktree has no language server of its own, so its own agent sees no diagnostics.
struct NoDiagnostics;

impl ide_agent::Diagnostics for NoDiagnostics {
    fn diagnostics(&self, _: Option<&Path>) -> Option<Vec<ide_agent::Diagnostic>> {
        None
    }
}

/// The vendor behind `model` on an endpoint of `kind`: the provider's own for its first-party API, else the model's.
pub(super) fn vendor(kind: ProviderKind, model: &str) -> Vendor {
    match kind {
        ProviderKind::Anthropic => Vendor::Anthropic,
        ProviderKind::OpenAi => Vendor::OpenAi,
        ProviderKind::Gemini => Vendor::Google,
        _ => vendor_of_model(model),
    }
}

/// The model a new thread starts with, when its endpoint can be asked.
fn choice() -> Option<(ModelChoice, ide_agent::Endpoint)> {
    let choice = own_agent::default_choice()?;
    let endpoint = own_agent::endpoint(&choice.endpoint)?;
    own_agent::usable(&endpoint).then_some((choice, endpoint))
}

/// What the Leitstand knows of the own agent now.
pub(super) fn status() -> AgentStatus {
    let mut status = AgentStatus::ready(&Agent::Own);
    match choice() {
        Some((choice, endpoint)) => status.vendor = Some(vendor(endpoint.kind, &choice.model)),
        None => {
            status.readiness = Readiness::StartFailed(
                splat_i18n::t("Für den eigenen Agenten ist kein Modell mit Endpunkt eingerichtet.").into(),
            );
        }
    }
    status
}

/// A session of the own agent for an order.
pub(super) struct Start {
    pub(super) config: Box<Config>,
    /// The thread whose stored conversation resumes it later (A15).
    pub(super) thread: String,
    /// Whether a stored conversation of that thread is there to resume now.
    pub(super) resumes: bool,
}

/// What the own agent starts with in `worktree` for `entry`, resuming the thread `resume` names where its
/// conversation is kept; why not, in the user's words.
pub(super) fn start(worktree: &Path, entry: &RankEntry, resume: Option<&str>) -> Result<Start, Explained> {
    let Some((choice, _)) = choice() else {
        return Err(Explained(
            splat_i18n::t("Für den eigenen Agenten ist kein Modell mit Endpunkt eingerichtet.").into(),
        ));
    };
    let choice = ModelChoice {
        model: entry.model.clone().unwrap_or(choice.model),
        endpoint: choice.endpoint,
    };
    let thread = resume.map_or_else(ThreadId::new, ThreadId::from);
    let config = own_agent::config(thread.clone(), worktree, &choice, true, Arc::new(NoDiagnostics))?;
    let resumes = resume.is_some() && config.store.as_ref().is_some_and(|store| store.exists());
    Ok(Start {
        config: Box::new(config),
        thread: thread.as_str().to_string(),
        resumes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_party_endpoint_names_its_vendor_and_any_other_the_model_does() {
        assert_eq!(vendor(ProviderKind::Gemini, "my-tune"), Vendor::Google);
        assert_eq!(vendor(ProviderKind::Anthropic, "anything"), Vendor::Anthropic);
        assert_eq!(vendor(ProviderKind::OpenRouter, "qwen/qwen3-coder"), Vendor::Alibaba);
        assert_eq!(vendor(ProviderKind::Ollama, "llama3.3:70b"), Vendor::Meta);
        assert_eq!(vendor(ProviderKind::OpenAiCompatible, "my-finetune"), Vendor::Unknown);
    }
}
