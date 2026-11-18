//! The protocol's messages as they cross the wire: requests, responses and events, each a JSON object in a
//! Content-Length frame (`ide_process::framing`).

use serde_json::{Map, Value, json};

/// One message, from either side.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Asks the other side to do something.
    Request {
        /// The sender's running number.
        seq: i64,
        /// What is asked, such as `stackTrace`.
        command: String,
        /// Its arguments; `Null` without.
        arguments: Value,
    },
    /// Answers a request.
    Response {
        /// The sender's running number.
        seq: i64,
        /// The `seq` of the request it answers.
        request_seq: i64,
        /// Whether the request succeeded.
        success: bool,
        /// The request's command.
        command: String,
        /// Why it failed, in the adapter's words; for some failures a short code such as `cancelled`.
        message: Option<String>,
        /// The answer; `Null` without.
        body: Value,
    },
    /// Tells the client something happened.
    Event {
        /// The sender's running number.
        seq: i64,
        /// What happened, such as `stopped`.
        event: String,
        /// The details; `Null` without.
        body: Value,
    },
}

/// What is wrong with a message body.
#[derive(Debug)]
pub(crate) enum ParseError {
    /// It is not JSON.
    Json(serde_json::Error),
    /// It is JSON, but not an object.
    NotAnObject,
    /// A field the message's type needs is missing, such as a request's `command`.
    Missing {
        /// The message's type.
        kind: &'static str,
        /// The field.
        field: &'static str,
    },
    /// Its `type` is none the protocol knows.
    UnknownType(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Json(error) => write!(f, "a message is not JSON: {error}"),
            ParseError::NotAnObject => f.write_str("a message is not an object"),
            ParseError::Missing { kind, field } => write!(f, "a {kind} without {field}"),
            ParseError::UnknownType(kind) => write!(f, "a message of unknown type {kind:?}"),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::Json(error) => Some(error),
            _ => None,
        }
    }
}

fn missing(kind: &'static str, field: &'static str) -> ParseError {
    ParseError::Missing { kind, field }
}

/// Reads one message body; an error says what is wrong with it.
pub(crate) fn parse(body: &[u8]) -> Result<Message, ParseError> {
    let Value::Object(mut object) = serde_json::from_slice::<Value>(body).map_err(ParseError::Json)? else {
        return Err(ParseError::NotAnObject);
    };
    let seq = object.get("seq").and_then(Value::as_i64).unwrap_or(0);
    let kind = take_string(&mut object, "type").ok_or(missing("message", "type"))?;
    match kind.as_str() {
        "request" => Ok(Message::Request {
            seq,
            command: take_string(&mut object, "command").ok_or(missing("request", "command"))?,
            arguments: object.remove("arguments").unwrap_or(Value::Null),
        }),
        "response" => Ok(Message::Response {
            seq,
            request_seq: object
                .get("request_seq")
                .and_then(Value::as_i64)
                .ok_or(missing("response", "request_seq"))?,
            success: object.get("success").and_then(Value::as_bool).unwrap_or(false),
            command: take_string(&mut object, "command").unwrap_or_default(),
            message: error_message(&mut object),
            body: object.remove("body").unwrap_or(Value::Null),
        }),
        "event" => Ok(Message::Event {
            seq,
            event: take_string(&mut object, "event").ok_or(missing("event", "event"))?,
            body: object.remove("body").unwrap_or(Value::Null),
        }),
        other => Err(ParseError::UnknownType(other.to_string())),
    }
}

impl Message {
    /// The message as JSON, ready to frame.
    pub(crate) fn to_json(&self) -> Value {
        match self {
            Message::Request {
                seq,
                command,
                arguments,
            } => {
                let mut message = json!({"seq": seq, "type": "request", "command": command});
                if !arguments.is_null() {
                    message["arguments"] = arguments.clone();
                }
                message
            }
            Message::Response {
                seq,
                request_seq,
                success,
                command,
                message,
                body,
            } => {
                let mut out = json!({"seq": seq, "type": "response", "request_seq": request_seq,
                    "success": success, "command": command});
                if let Some(text) = message {
                    out["message"] = json!(text);
                }
                if !body.is_null() {
                    out["body"] = body.clone();
                }
                out
            }
            Message::Event { seq, event, body } => {
                let mut message = json!({"seq": seq, "type": "event", "event": event});
                if !body.is_null() {
                    message["body"] = body.clone();
                }
                message
            }
        }
    }

    /// The message framed for the wire.
    pub(crate) fn to_frame(&self) -> Vec<u8> {
        let body = serde_json::to_vec(&self.to_json()).unwrap_or_default();
        ide_process::framing::frame(&body)
    }
}

fn take_string(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    match object.remove(key) {
        Some(Value::String(text)) => Some(text),
        _ => None,
    }
}

/// A failed response's message; the readable text is in `body.error.format` when the adapter sends one.
fn error_message(object: &mut Map<String, Value>) -> Option<String> {
    let formatted = object
        .get("body")
        .and_then(|body| body.get("error"))
        .and_then(|error| error.get("format"))
        .and_then(Value::as_str)
        .map(str::to_string);
    formatted.or_else(|| take_string(object, "message"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_of_message_reads_back_as_it_was_written() {
        let messages = [
            Message::Request {
                seq: 1,
                command: "initialize".into(),
                arguments: json!({"adapterID": "lldb"}),
            },
            Message::Response {
                seq: 2,
                request_seq: 1,
                success: false,
                command: "launch".into(),
                message: Some("no such file".into()),
                body: Value::Null,
            },
            Message::Event {
                seq: 3,
                event: "stopped".into(),
                body: json!({"reason": "breakpoint", "threadId": 1}),
            },
        ];
        for message in messages {
            let bytes = serde_json::to_vec(&message.to_json()).unwrap();
            assert_eq!(parse(&bytes).unwrap(), message);
        }
    }

    #[test]
    fn a_formatted_error_is_preferred_over_the_short_message() {
        let body = br#"{"seq":4,"type":"response","request_seq":3,"success":false,"command":"evaluate",
            "message":"evaluate","body":{"error":{"id":1,"format":"use of undeclared identifier 'x'"}}}"#;
        let Message::Response { message, .. } = parse(body).unwrap() else {
            panic!("not a response");
        };
        assert_eq!(message.as_deref(), Some("use of undeclared identifier 'x'"));
    }

    #[test]
    fn broken_messages_are_errors() {
        for body in [
            &b"[]"[..],
            b"{}",
            b"{\"type\":\"response\"}",
            b"{\"type\":\"x\"}",
            b"not json",
        ] {
            assert!(parse(body).is_err(), "{}", String::from_utf8_lossy(body));
        }
    }
}
