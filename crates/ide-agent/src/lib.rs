//! The app's own agent (E6): it talks to Anthropic, OpenAI, Gemini, OpenRouter, Ollama, LM Studio and servers that
//! speak OpenAI's or Anthropic's protocol, with keys from the keychain and prices for what it spends.
//!
//! A `Provider` is an endpoint with its key. `Provider::stream` sends one request of the conversation and reads the
//! answer as it streams; `Provider::models` lists what the endpoint offers. Every protocol is turned into the same
//! `Reply` and `Delta`s, so the agent's loop does not know which one it talks to.
//!
//! A `Session` is one thread: it fills the shared thread model (`ide-thread`) with messages, reasoning, tool cards,
//! permission requests, to-dos and usage, and is driven like an ACP session (`send`, `cancel`, `answer`). Its tools
//! read, search, edit and write files, run commands through the supervisor, fetch web pages and read the language
//! servers' diagnostics through the `Diagnostics` trait the app implements. The `Policy` decides what runs, asks or
//! is refused (E27); with the thread's sandbox on, files, web requests and commands stay within the project and the
//! allowed folders (E19). When the context window fills up, older turns are compacted into a summary (E42). What the
//! agent tells the user comes as values the app words (`notes`, E41). `store` keeps a thread's conversation for
//! resuming.

#![forbid(unsafe_code)]

mod anthropic;
pub mod chat;
mod compact;
pub mod conversation;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing;
mod gemini;
pub mod http;
pub mod keys;
pub mod models;
pub mod notes;
mod openai;
pub mod policy;
pub mod prices;
pub mod prompt;
pub mod provider;
mod replay;
pub mod session;
mod sse;
pub mod store;
#[cfg(any(test, feature = "testing"))]
pub mod stub;
mod tools;
mod turn;
pub mod workspace;

pub use chat::{Cancel, Delta, ErrorKind, ProviderError, Reply, Request, Stop, ToolSpec, TurnUsage};
pub use conversation::{Block, Message, Role};
pub use http::Http;
pub use keys::{ApiKey, Keychain, Memory, NotReady, Remembered, SecretError, SecretStore};
pub use models::{ModelChoice, ModelInfo};
pub use notes::{Note, Plain, Problem, ToolNote, Wording};
pub use policy::{Approval, Destructive, Policy, Profile, Reason, Refusal, Rule};
pub use prices::{Price, PriceTable};
pub use provider::{Endpoint, ProviderKind, Wire};
pub use session::{Choice, Config, Event, Session, SessionError};
pub use store::LoadError;
pub use workspace::{Diagnostic, Diagnostics, Severity, Workspace};

/// An endpoint with its key, ready for requests.
#[derive(Clone)]
pub struct Provider {
    endpoint: Endpoint,
    key: Option<ApiKey>,
    http: Http,
}

impl Provider {
    /// `endpoint` reached with `key` over `http`.
    pub fn new(endpoint: Endpoint, key: Option<ApiKey>, http: Http) -> Provider {
        Provider { endpoint, key, http }
    }

    pub(crate) fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// The same endpoint and key over connections of its own, which `close` ends without touching other requests.
    pub(crate) fn separate(&self) -> Provider {
        Provider {
            endpoint: self.endpoint.clone(),
            key: self.key.clone(),
            http: self.http.separate(),
        }
    }

    /// Ends the requests of this provider and its clones at once, also one waiting in silence for the next part.
    pub(crate) fn close(&self) {
        self.http.close();
    }

    /// A service that needs a key and has none; an added endpoint may work without.
    fn missing_key(&self) -> Option<ProviderError> {
        let missing = self.key.is_none() && self.endpoint.kind.takes_key() && self.endpoint.is_first_party();
        missing.then(|| ProviderError::new(ErrorKind::NoKey, self.endpoint.name.clone()))
    }

    /// Sends `request` and reads the answer as it streams, telling `on` about each part; returns early, with the
    /// text so far, once `cancel` is cancelled.
    ///
    /// ```
    /// use ide_agent::stub::{Answer, Stub};
    /// use ide_agent::{Cancel, Delta, Endpoint, Http, Message, Provider, ProviderKind, Request, Stop};
    ///
    /// let stub = Stub::script(vec![Answer::events(concat!(
    ///     r#"data: {"choices": [{"index": 0, "delta": {"content": "Hello."}, "finish_reason": "stop"}]}"#,
    ///     "\n\ndata: [DONE]\n\n",
    /// ))])
    /// .unwrap();
    /// let base = format!("{}/v1", stub.url());
    /// let endpoint = Endpoint::new("local", ProviderKind::OpenAiCompatible, "Local", &base);
    /// let provider = Provider::new(endpoint, None, Http::new());
    /// let messages = [Message::user("Hi")];
    /// let request = Request { model: "m", system: "", messages: &messages, tools: &[], max_tokens: None, effort: None };
    /// let mut deltas = Vec::new();
    /// let reply = provider.stream(&request, &Cancel::new(), &mut |delta| deltas.push(delta)).unwrap();
    /// assert_eq!((reply.stop, reply.message.text()), (Stop::EndTurn, "Hello.".to_string()));
    /// assert_eq!(deltas, [Delta::Text("Hello.".into())]);
    /// ```
    pub fn stream(
        &self,
        request: &Request<'_>,
        cancel: &Cancel,
        on: &mut dyn FnMut(Delta),
    ) -> Result<Reply, ProviderError> {
        if let Some(error) = self.missing_key() {
            return Err(error);
        }
        let key = self.key.as_ref();
        let result = match self.endpoint.kind.wire() {
            Wire::Anthropic => anthropic::stream(&self.http, &self.endpoint, key, request, cancel, on),
            Wire::OpenAiChat => openai::stream(&self.http, &self.endpoint, key, request, cancel, on),
            Wire::Gemini => gemini::stream(&self.http, &self.endpoint, key, request, cancel, on),
        };
        match result {
            Err(_) if cancel.is_cancelled() => Ok(Reply {
                message: Message {
                    role: Role::Assistant,
                    content: Vec::new(),
                },
                usage: TurnUsage::default(),
                stop: Stop::Cancelled,
                model: None,
            }),
            result => result,
        }
    }

    /// The models the endpoint lists for this key.
    pub fn models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        if let Some(error) = self.missing_key() {
            return Err(error);
        }
        let key = self.key.as_ref();
        match self.endpoint.kind.wire() {
            Wire::Anthropic => anthropic::models(&self.http, &self.endpoint, key),
            Wire::OpenAiChat => openai::models(&self.http, &self.endpoint, key),
            Wire::Gemini => gemini::models(&self.http, &self.endpoint, key),
        }
    }
}

/// Whether `model` at an endpoint of `kind` thinks adaptively and takes an effort level.
pub(crate) fn takes_effort(kind: ProviderKind, model: &str) -> bool {
    match kind.wire() {
        Wire::Anthropic => anthropic::adaptive(model),
        Wire::OpenAiChat | Wire::Gemini => true,
    }
}
