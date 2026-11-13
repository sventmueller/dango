//! HTTPS to the providers, checked against the system's certificates; failures become `ProviderError`s with what the
//! provider said and how long it asks to wait.

use std::io::BufReader;
use std::time::Duration;

use ide_net::ureq;
use serde_json::Value;

use crate::chat::{ErrorKind, ProviderError};

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
        let agent = ide_net::client(ide_net::Client {
            purpose: "agent",
            connect: Duration::from_secs(20),
            read: Duration::from_secs(300),
            status_as_error: false,
        });
        Http { agent }
    }

    /// Posts `body` and returns the response's body to read as a stream, or the failure.
    pub(crate) fn post_stream(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &Value,
    ) -> Result<BufReader<ureq::BodyReader<'static>>, ProviderError> {
        let mut call = self.agent.post(url);
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
        let mut response = self
            .agent
            .post(url)
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

    /// Ends every request of this client and its clones at once, also one waiting for an answer, by closing their
    /// connections; the server sees the client go. The client takes no further requests.
    pub fn close(&self) {
        self.agent.clone().shutdown();
    }

    /// Gets `url` and reads its JSON.
    pub fn get_json(&self, url: &str, headers: &[(&str, String)]) -> Result<Value, ProviderError> {
        let mut call = self.agent.get(url);
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
    pub(crate) fn get_text(&self, url: &str, limit: u64) -> Result<(String, String), ProviderError> {
        use std::io::Read;
        let mut response = self.agent.get(url).call().map_err(network)?;
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

/// The failure an error object inside a stream stands for, such as Anthropic's `overloaded_error`.
pub(crate) fn stream_error(error: &Value) -> ProviderError {
    let kind = match error.get("type").and_then(Value::as_str) {
        Some("overloaded_error") => ErrorKind::Overloaded,
        Some("rate_limit_error") => ErrorKind::RateLimit,
        Some("api_error") => ErrorKind::Server,
        Some("authentication_error") => ErrorKind::Auth,
        Some("permission_error") => ErrorKind::Permission,
        Some("invalid_request_error") => ErrorKind::BadRequest,
        _ => match error
            .get("code")
            .and_then(Value::as_u64)
            .and_then(|c| u16::try_from(c).ok())
        {
            Some(status) => ProviderError::kind_of(status),
            None => ErrorKind::Server,
        },
    };
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let kind = match kind {
        ErrorKind::BadRequest if crate::chat::too_long(&message) => ErrorKind::ContextLength,
        kind => kind,
    };
    ProviderError::new(kind, message)
}

/// The JSON of one event's data.
pub(crate) fn parse(data: &str) -> Result<Value, ProviderError> {
    serde_json::from_str(data).map_err(|e| ProviderError::new(ErrorKind::Malformed, format!("{e}: {data}")))
}

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
        let overloaded = serde_json::json!({"type": "overloaded_error", "message": "busy"});
        assert_eq!(stream_error(&overloaded).kind, ErrorKind::Overloaded);
        assert_eq!(
            stream_error(&serde_json::json!({"code": 429})).kind,
            ErrorKind::RateLimit
        );
    }
}
