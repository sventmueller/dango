//! The services the own agent talks to and how each is reached: the protocol it speaks, its address and whether it
//! wants a key.

use serde::{Deserialize, Serialize};

/// A language model service, or a kind of endpoint the user adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Anthropic's Claude API.
    Anthropic,
    /// OpenAI's API.
    OpenAi,
    /// Google's Gemini API.
    Gemini,
    /// Ollama on this Mac or another one.
    Ollama,
    /// OpenRouter, which routes to many providers.
    OpenRouter,
    /// LM Studio's local server.
    LmStudio,
    /// Any server that speaks OpenAI's chat completions.
    OpenAiCompatible,
    /// Any server that speaks Anthropic's Messages API.
    AnthropicCompatible,
}

/// The protocol of an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wire {
    /// Anthropic's Messages API with server-sent events.
    Anthropic,
    /// OpenAI's chat completions with server-sent events.
    OpenAiChat,
    /// Gemini's `streamGenerateContent` with server-sent events.
    Gemini,
}

impl ProviderKind {
    /// Every kind, in the order the settings list them.
    pub const ALL: [ProviderKind; 8] = [
        ProviderKind::Anthropic,
        ProviderKind::OpenAi,
        ProviderKind::Gemini,
        ProviderKind::OpenRouter,
        ProviderKind::Ollama,
        ProviderKind::LmStudio,
        ProviderKind::OpenAiCompatible,
        ProviderKind::AnthropicCompatible,
    ];

    /// Its stable name in settings and keychain accounts.
    pub fn id(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::OpenAi => "openai",
            ProviderKind::Gemini => "gemini",
            ProviderKind::Ollama => "ollama",
            ProviderKind::OpenRouter => "openrouter",
            ProviderKind::LmStudio => "lmstudio",
            ProviderKind::OpenAiCompatible => "openai-compatible",
            ProviderKind::AnthropicCompatible => "anthropic-compatible",
        }
    }

    /// The service's own name, for the services that have one.
    pub fn brand(self) -> Option<&'static str> {
        match self {
            ProviderKind::Anthropic => Some("Anthropic"),
            ProviderKind::OpenAi => Some("OpenAI"),
            ProviderKind::Gemini => Some("Google Gemini"),
            ProviderKind::Ollama => Some("Ollama"),
            ProviderKind::OpenRouter => Some("OpenRouter"),
            ProviderKind::LmStudio => Some("LM Studio"),
            ProviderKind::OpenAiCompatible | ProviderKind::AnthropicCompatible => None,
        }
    }

    /// The protocol it speaks.
    pub fn wire(self) -> Wire {
        match self {
            ProviderKind::Anthropic | ProviderKind::AnthropicCompatible => Wire::Anthropic,
            ProviderKind::Gemini => Wire::Gemini,
            ProviderKind::OpenAi
            | ProviderKind::Ollama
            | ProviderKind::OpenRouter
            | ProviderKind::LmStudio
            | ProviderKind::OpenAiCompatible => Wire::OpenAiChat,
        }
    }

    /// Where the service answers unless the user says otherwise; none for endpoints the user adds.
    pub fn default_base(self) -> Option<&'static str> {
        match self {
            ProviderKind::Anthropic => Some("https://api.anthropic.com"),
            ProviderKind::OpenAi => Some("https://api.openai.com/v1"),
            ProviderKind::Gemini => Some("https://generativelanguage.googleapis.com/v1beta"),
            ProviderKind::Ollama => Some("http://localhost:11434/v1"),
            ProviderKind::OpenRouter => Some("https://openrouter.ai/api/v1"),
            ProviderKind::LmStudio => Some("http://localhost:1234/v1"),
            ProviderKind::OpenAiCompatible | ProviderKind::AnthropicCompatible => None,
        }
    }

    /// Whether requests carry a key; a server on this Mac takes none, an added endpoint may or may not.
    pub fn takes_key(self) -> bool {
        !matches!(self, ProviderKind::Ollama | ProviderKind::LmStudio)
    }

    /// Whether its models run on hardware of the user's, which costs nothing per token.
    pub fn is_local(self) -> bool {
        matches!(self, ProviderKind::Ollama | ProviderKind::LmStudio)
    }

    /// The model a new thread starts with, where the service has an obvious one; others pick from the model list.
    pub fn default_model(self) -> Option<&'static str> {
        match self {
            ProviderKind::Anthropic => Some("claude-opus-5-5"),
            _ => None,
        }
    }

    /// The kind with this id.
    pub fn from_id(id: &str) -> Option<ProviderKind> {
        ProviderKind::ALL.into_iter().find(|kind| kind.id() == id)
    }
}

/// One place the agent sends requests to: a service at its address, or an endpoint the user added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Endpoint {
    /// Names it in settings and in the keychain: the kind's id for a service, a name of the user's for an added one.
    pub id: String,
    pub kind: ProviderKind,
    /// What the user calls it.
    pub name: String,
    /// The API's base URL, without a trailing slash.
    pub base_url: String,
}

impl Endpoint {
    /// The service of `kind` at its usual address; none for the kinds the user adds.
    pub fn preset(kind: ProviderKind) -> Option<Endpoint> {
        Some(Endpoint {
            id: kind.id().to_string(),
            kind,
            name: kind.brand()?.to_string(),
            base_url: kind.default_base()?.to_string(),
        })
    }

    /// An endpoint at `base_url`.
    pub fn new(id: &str, kind: ProviderKind, name: &str, base_url: &str) -> Endpoint {
        Endpoint {
            id: id.to_string(),
            kind,
            name: name.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// The URL of `path` under the base URL.
    pub(crate) fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Whether this is the service itself rather than something that speaks its protocol, which may not know its
    /// newer request fields.
    pub fn is_first_party(&self) -> bool {
        self.kind.default_base().is_some_and(|base| self.base_url == base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_service_has_an_address_and_only_added_endpoints_lack_one() {
        for kind in ProviderKind::ALL {
            let added = matches!(kind, ProviderKind::OpenAiCompatible | ProviderKind::AnthropicCompatible);
            assert_eq!(Endpoint::preset(kind).is_none(), added, "{kind:?}");
            assert_eq!(ProviderKind::from_id(kind.id()), Some(kind));
        }
    }

    #[test]
    fn urls_join_with_one_slash_and_only_the_usual_address_counts_as_the_service() {
        let endpoint = Endpoint::new("x", ProviderKind::OpenAiCompatible, "X", "http://host:8000/v1/");
        assert_eq!(
            endpoint.url("/chat/completions"),
            "http://host:8000/v1/chat/completions"
        );
        assert!(!endpoint.is_first_party());
        let anthropic = Endpoint::preset(ProviderKind::Anthropic).unwrap();
        assert!(anthropic.is_first_party());
        let proxy = Endpoint::new("anthropic", ProviderKind::Anthropic, "A", "http://127.0.0.1:9");
        assert!(!proxy.is_first_party());
    }
}
