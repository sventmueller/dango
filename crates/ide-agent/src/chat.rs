//! One request to a model and what comes back: the parts that stream while it answers, the finished message, the
//! tokens it used, why it stopped, and why it failed.

use serde_json::Value;

use crate::conversation::Message;

/// A tool as the model is told about it.
#[derive(Debug, Clone, PartialEq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct ToolSpec {
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
    pub(crate) fn prompt(&self) -> u64 {
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

pub use ide_net::api::{ErrorKind, ProviderError};

pub use ide_core::Cancel;
