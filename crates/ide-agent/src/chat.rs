//! One request to a model and what comes back: the parts that stream while it answers, the finished message, the
//! tokens it used, why it stopped, and why it failed.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::conversation::Message;

/// A tool as the model is told about it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    /// Its name.
    pub name: String,
    /// What it does and when to use it.
    pub description: String,
    /// The JSON schema of its input.
    pub schema: Value,
}

/// What one request asks.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    /// The model.
    pub model: &'a str,
    /// The instructions before the conversation.
    pub system: &'a str,
    /// The conversation so far, ending with the user's turn.
    pub messages: &'a [Message],
    /// The tools it may call.
    pub tools: &'a [ToolSpec],
    /// The most tokens the answer may take; the provider's default when not set.
    pub max_tokens: Option<u64>,
    /// How hard the model thinks, such as `low` or `high`, where the provider takes it.
    pub effort: Option<&'a str>,
}

/// Something that arrived while the model answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    /// Text of the answer.
    Text(String),
    /// Text of the reasoning the provider shows.
    Thinking(String),
    /// A tool call began.
    ToolStarted {
        /// Its id.
        id: String,
        /// The tool.
        name: String,
    },
    /// Part of a tool call's input, as JSON text.
    ToolInput {
        /// The call.
        id: String,
        /// The fragment.
        json: String,
    },
    /// The provider handed the answer to another model after the first declined it; what arrived before stays text
    /// only.
    Fallback {
        /// The model that declined.
        from: String,
        /// The model that continues.
        to: String,
    },
}

/// The tokens one request used.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TurnUsage {
    /// Input tokens read neither from nor into the prompt cache.
    pub input: u64,
    /// Output tokens, reasoning included.
    pub output: u64,
    /// Input tokens read from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to the prompt cache.
    pub cache_write: u64,
    /// What the provider says the request cost, in US dollars, where it says.
    pub cost_usd: Option<f64>,
}

impl TurnUsage {
    /// The whole prompt: what the context window held when the request was made.
    pub fn prompt(&self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }
}

/// Why the model stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// It finished its turn.
    EndTurn,
    /// It waits for the results of its tool calls.
    ToolUse,
    /// The answer hit the token limit.
    MaxTokens,
    /// The provider declined to answer; its category, where it names one.
    Refusal(Option<String>),
    /// The caller cancelled; the message holds the text that had arrived.
    Cancelled,
    /// Any other reason, as the provider names it.
    Other(String),
}

/// A finished answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    /// The model's message.
    pub message: Message,
    /// The tokens it used.
    pub usage: TurnUsage,
    /// Why it stopped.
    pub stop: Stop,
    /// The model that answered, as the provider names it.
    pub model: Option<String>,
}

/// What kind of failure a request met.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The endpoint needs a key and has none; the message is the endpoint's name.
    NoKey,
    /// The key is wrong or expired.
    Auth,
    /// The key may not use this model or feature.
    Permission,
    /// The account has no credit or no plan for this.
    Billing,
    /// The model or address does not exist.
    NotFound,
    /// The request is too large.
    TooLarge,
    /// The conversation no longer fits the model's context window.
    ContextLength,
    /// Too many requests or tokens; try again later.
    RateLimit,
    /// The service is overloaded; try again later.
    Overloaded,
    /// The service failed.
    Server,
    /// The service refused the request as malformed.
    BadRequest,
    /// The service could not be reached or the connection broke.
    Network,
    /// The answer does not have the shape the protocol defines.
    Malformed,
    /// The answer ended before it was complete.
    Incomplete,
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderError {
    /// What kind of failure it is.
    pub kind: ErrorKind,
    /// The HTTP status, when there was one.
    pub status: Option<u16>,
    /// What the provider said, word for word, or the library's words for a failure on the way; empty where the
    /// kind says it all.
    pub message: String,
    /// How long the provider asks to wait before the next try.
    pub retry_after: Option<Duration>,
}

impl ProviderError {
    /// A failure without a status.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> ProviderError {
        ProviderError {
            kind,
            status: None,
            message: message.into(),
            retry_after: None,
        }
    }

    /// Whether trying again later can succeed.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::RateLimit
                | ErrorKind::Overloaded
                | ErrorKind::Server
                | ErrorKind::Network
                | ErrorKind::Incomplete
        )
    }

    /// The failure of a status and what the provider said with it; a request too large for the context window is
    /// told by its words, as no status marks it.
    pub fn of_status(status: u16, message: String) -> ProviderError {
        let kind = match ProviderError::kind_of(status) {
            ErrorKind::BadRequest | ErrorKind::TooLarge if too_long(&message) => ErrorKind::ContextLength,
            kind => kind,
        };
        ProviderError {
            kind,
            status: Some(status),
            message,
            retry_after: None,
        }
    }

    /// The kind of failure an HTTP status stands for.
    pub fn kind_of(status: u16) -> ErrorKind {
        match status {
            401 => ErrorKind::Auth,
            402 => ErrorKind::Billing,
            403 => ErrorKind::Permission,
            404 => ErrorKind::NotFound,
            413 => ErrorKind::TooLarge,
            429 => ErrorKind::RateLimit,
            503 | 529 => ErrorKind::Overloaded,
            500..=599 => ErrorKind::Server,
            _ => ErrorKind::BadRequest,
        }
    }
}

/// Whether a provider's message says the prompt exceeds the context window, in the words Anthropic, OpenAI, Gemini,
/// OpenRouter, Ollama and LM Studio use.
pub(crate) fn too_long(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "prompt is too long",
        "context_length_exceeded",
        "maximum context length",
        "context length",
        "context window",
        "exceeds the maximum number of tokens",
        "too many tokens",
    ]
    .iter()
    .any(|words| message.contains(words))
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(status) => write!(f, "{status}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ProviderError {}

pub use ide_core::Cancel;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_map_to_kinds_and_only_passing_trouble_is_retried() {
        let retried: Vec<u16> = [400, 401, 402, 403, 404, 413, 429, 500, 502, 503, 529]
            .into_iter()
            .filter(|s| ProviderError::new(ProviderError::kind_of(*s), "").is_retryable())
            .collect();
        assert_eq!(retried, [429, 500, 502, 503, 529]);
        assert_eq!(ProviderError::kind_of(529), ErrorKind::Overloaded);
    }

    #[test]
    fn a_prompt_too_long_for_the_window_is_told_by_the_providers_words() {
        for message in [
            "prompt is too long: 210000 tokens > 200000 maximum",
            "This model's maximum context length is 128000 tokens.",
            "The input token count (1200000) exceeds the maximum number of tokens allowed (1048576).",
        ] {
            assert_eq!(
                ProviderError::of_status(400, message.into()).kind,
                ErrorKind::ContextLength
            );
        }
        assert_eq!(ProviderError::of_status(400, "bad".into()).kind, ErrorKind::BadRequest);
        assert_eq!(
            ProviderError::of_status(401, "context window".into()).kind,
            ErrorKind::Auth
        );
    }
}
