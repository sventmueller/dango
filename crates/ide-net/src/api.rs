//! HTTPS to the providers of language models, checked against the system's certificates, and why a request to one
//! failed: what the provider said, the kind of failure, and how long it asks to wait. The app's own agent
//! (`ide-agent`) and the local edit predictions (`ide-predict`) share it. A request that starts on HTTPS stays on it
//! through its redirects; plain HTTP is for the local servers, such as Ollama, that the user names so.

use std::io::{BufRead, BufReader};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::net::{Client, client, guarded};

pub mod anthropic;

/// The most of an error or JSON answer that is read.
const BODY_LIMIT: u64 = 8 * 1024 * 1024;

/// The client for every provider request.
#[derive(Clone)]
pub struct Http {
    agent: ureq::Agent,
}

impl Http {
    /// A client that gives up connecting after 20 seconds, and on a stream that is silent for five minutes, which a
    /// local server loading a large model can be.
    pub fn new() -> Http {
        let agent = client(Client {
            purpose: "agent",
            connect: Duration::from_secs(20),
            read: Duration::from_secs(300),
            status_as_error: false,
            https_only: false,
        });
        Http { agent }
    }

    /// Posts `body` and returns the response's body to read as a stream, or the failure.
    pub fn post_stream(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &Value,
    ) -> Result<impl BufRead + Send + 'static, ProviderError> {
        let mut call = guarded(self.agent.post(url), url);
        for (name, value) in headers {
            call = call.header(*name, value);
        }
        let text = serde_json::to_vec(body).map_err(|e| ProviderError::new(ErrorKind::Malformed, e.to_string()))?;
        let response = call
            .header("content-type", "application/json")
            .send(&text[..])
            .map_err(network)?;
        if !response.status().is_success() {
            return Err(failure(response));
        }
        Ok(BufReader::new(response.into_body().into_reader()))
    }

    /// Posts `body` and reads the JSON answer.
    pub fn post_json(&self, url: &str, body: &Value) -> Result<Value, ProviderError> {
        let text = serde_json::to_vec(body).map_err(|e| ProviderError::new(ErrorKind::Malformed, e.to_string()))?;
        let mut response = guarded(self.agent.post(url), url)
            .header("content-type", "application/json")
            .send(&text[..])
            .map_err(network)?;
        if !response.status().is_success() {
            return Err(failure(response));
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(BODY_LIMIT)
            .read_to_vec()
            .map_err(network)?;
        serde_json::from_slice(&bytes).map_err(|e| ProviderError::new(ErrorKind::Malformed, e.to_string()))
    }

    /// A client with this one's settings and connections of its own, so that closing it ends its requests only.
    pub fn separate(&self) -> Http {
        Http {
            agent: self.agent.config().new_agent(),
        }
    }

    /// Ends every request of this client and its clones at once, also one waiting for an answer, by closing their
    /// connections; the server sees the client go. The client takes no further requests.
    pub fn close(&self) {
        self.agent.clone().shutdown();
    }

    /// Gets `url` and reads its JSON.
    pub fn get_json(&self, url: &str, headers: &[(&str, String)]) -> Result<Value, ProviderError> {
        let mut call = guarded(self.agent.get(url), url);
        for (name, value) in headers {
            call = call.header(*name, value);
        }
        let mut response = call.call().map_err(network)?;
        if !response.status().is_success() {
            return Err(failure(response));
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(BODY_LIMIT)
            .read_to_vec()
            .map_err(network)?;
        serde_json::from_slice(&bytes).map_err(|e| ProviderError::new(ErrorKind::Malformed, e.to_string()))
    }
}

impl Http {
    /// Gets `url` as text, up to `limit` bytes, with the type its server gives it.
    pub fn get_text(&self, url: &str, limit: u64) -> Result<(String, String), ProviderError> {
        use std::io::Read;
        let mut response = guarded(self.agent.get(url), url).call().map_err(network)?;
        if !response.status().is_success() {
            return Err(failure(response));
        }
        let kind = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(|e| ProviderError::new(ErrorKind::Network, e.to_string()))?;
        Ok((kind, String::from_utf8_lossy(&bytes).into_owned()))
    }
}

impl Default for Http {
    fn default() -> Http {
        Http::new()
    }
}

fn network(error: ureq::Error) -> ProviderError {
    ProviderError::new(ErrorKind::Network, error.to_string())
}

/// The failure an answer with an error status stands for.
fn failure(mut response: ureq::http::Response<ureq::Body>) -> ProviderError {
    let status = response.status().as_u16();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let retry_after = retry_after(header("retry-after-ms").as_deref(), header("retry-after").as_deref());
    let body = response
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .lossy_utf8(true)
        .read_to_string()
        .unwrap_or_default();
    let mut error = ProviderError::of_status(
        status,
        error_message(&body).unwrap_or_else(|| body.trim().chars().take(500).collect()),
    );
    if body.contains("context_length_exceeded") {
        error.kind = ErrorKind::ContextLength;
    }
    error.retry_after = retry_after;
    error
}

/// How long to wait, from `retry-after-ms` or `retry-after` in seconds; a date is not read.
pub(crate) fn retry_after(millis: Option<&str>, seconds: Option<&str>) -> Option<Duration> {
    if let Some(ms) = millis
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
    {
        return Some(Duration::from_secs_f64(ms / 1000.0));
    }
    seconds
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(Duration::from_secs_f64)
}

/// The message in a provider's error body: `{"error": {"message": …}}`, also inside a list as Gemini sends it.
pub(crate) fn error_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let value = match value {
        Value::Array(items) => items.into_iter().next()?,
        value => value,
    };
    let error = value.get("error")?;
    match error {
        Value::String(text) => Some(text.clone()),
        error => error.get("message").and_then(Value::as_str).map(str::to_string),
    }
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
    pub(crate) fn of_status(status: u16, message: String) -> ProviderError {
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
pub fn too_long(message: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_comes_from_milliseconds_or_seconds() {
        assert_eq!(retry_after(Some("1500"), Some("9")), Some(Duration::from_millis(1500)));
        assert_eq!(retry_after(None, Some("7")), Some(Duration::from_secs(7)));
        assert_eq!(retry_after(None, Some("Wed, 21 Oct 2026 07:28:00 GMT")), None);
    }

    #[test]
    fn error_messages_are_found_in_each_providers_shape() {
        let anthropic = r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#;
        let gemini = r#"[{"error":{"code":429,"message":"quota","status":"RESOURCE_EXHAUSTED"}}]"#;
        assert_eq!(error_message(anthropic).as_deref(), Some("slow down"));
        assert_eq!(error_message(gemini).as_deref(), Some("quota"));
        assert_eq!(error_message("not json"), None);
    }

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
