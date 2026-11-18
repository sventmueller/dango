//! What the providers' streams carry besides their answers: the error objects inside a stream, and each event's JSON.
//! The client itself is `ide_net::api::Http`, which the edit predictions share.

use serde_json::Value;

pub use ide_net::api::Http;

use crate::chat::{ErrorKind, ProviderError};

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
        ErrorKind::BadRequest if ide_net::api::too_long(&message) => ErrorKind::ContextLength,
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
    fn errors_inside_a_stream_keep_their_kind() {
        let overloaded = serde_json::json!({"type": "overloaded_error", "message": "busy"});
        assert_eq!(stream_error(&overloaded).kind, ErrorKind::Overloaded);
        assert_eq!(
            stream_error(&serde_json::json!({"code": 429})).kind,
            ErrorKind::RateLimit
        );
    }
}
