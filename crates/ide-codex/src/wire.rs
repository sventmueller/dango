//! The app-server's framing on stdio: one JSON-RPC 2.0 message per line, without the `"jsonrpc"` member, which the
//! server neither sends nor needs (`docs/codex.md`).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// The id of a request; the server's own requests use numbers, and either side may use strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    /// A number.
    Number(i64),
    /// A string.
    Text(String),
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Id::Number(n) => write!(f, "{n}"),
            Id::Text(s) => f.write_str(s),
        }
    }
}

/// The error a request was answered with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    /// The JSON-RPC code, such as `-32600` for a request the server refuses in its state.
    pub code: i64,
    /// What the server says.
    pub message: String,
    /// What the server attached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// The request is not valid now, such as one before `initialize`; the server also uses it for unknown methods.
    pub const INVALID_REQUEST: i64 = -32600;
    /// The method is not known.
    pub const METHOD_NOT_FOUND: i64 = -32601;

    /// An error with `code` and `message`.
    pub fn new(code: i64, message: impl Into<String>) -> RpcError {
        RpcError {
            code,
            message: message.into(),
            data: None,
        }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

/// One line on the wire.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A request that waits for an answer.
    Request {
        /// Its id.
        id: Id,
        /// The method.
        method: String,
        /// Its parameters; `Null` when there are none.
        params: Value,
    },
    /// A notification.
    Notification {
        /// The method.
        method: String,
        /// Its parameters; `Null` when there are none.
        params: Value,
    },
    /// A successful answer.
    Response {
        /// The request's id.
        id: Id,
        /// The result.
        result: Value,
    },
    /// A failed answer.
    Error {
        /// The request's id; `None` when the server could not read it.
        id: Option<Id>,
        /// The error.
        error: RpcError,
    },
}

impl Message {
    /// The message on `line`; `None` for a line that is no JSON-RPC message, which the reader skips.
    pub fn parse(line: &str) -> Option<Message> {
        let Value::Object(mut map) = serde_json::from_str::<Value>(line.trim()).ok()? else {
            return None;
        };
        let id = map.remove("id").and_then(|id| serde_json::from_value::<Id>(id).ok());
        let params = map.remove("params").unwrap_or(Value::Null);
        if let Some(Value::String(method)) = map.remove("method") {
            return Some(match id {
                Some(id) => Message::Request { id, method, params },
                None => Message::Notification { method, params },
            });
        }
        if let Some(error) = map.remove("error") {
            let error = serde_json::from_value(error).ok()?;
            return Some(Message::Error { id, error });
        }
        let result = map.remove("result")?;
        Some(Message::Response { id: id?, result })
    }

    /// The message as one line, without the line break.
    pub fn encode(&self) -> String {
        let mut map = Map::new();
        match self {
            Message::Request { id, method, params } => {
                map.insert("id".into(), json!(id));
                map.insert("method".into(), json!(method));
                with_params(&mut map, params);
            }
            Message::Notification { method, params } => {
                map.insert("method".into(), json!(method));
                with_params(&mut map, params);
            }
            Message::Response { id, result } => {
                map.insert("id".into(), json!(id));
                map.insert("result".into(), result.clone());
            }
            Message::Error { id, error } => {
                map.insert("id".into(), json!(id));
                map.insert("error".into(), json!(error));
            }
        }
        Value::Object(map).to_string()
    }
}

fn with_params(map: &mut Map<String, Value>, params: &Value) {
    if !params.is_null() {
        map.insert("params".into(), params.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_read_as_the_four_kinds_and_the_jsonrpc_member_is_ignored() {
        let request = Message::parse(r#"{"id":900,"method":"item/fileChange/requestApproval","params":{"a":1}}"#);
        assert_eq!(
            request,
            Some(Message::Request {
                id: Id::Number(900),
                method: "item/fileChange/requestApproval".into(),
                params: json!({"a": 1}),
            })
        );
        let note = Message::parse(r#"{"method":"turn/started","params":{},"emittedAtMs":1}"#);
        assert!(matches!(note, Some(Message::Notification { method, .. }) if method == "turn/started"));
        let answer = Message::parse(r#"{"jsonrpc":"2.0","id":"s4","result":{"account":null}}"#);
        assert_eq!(
            answer,
            Some(Message::Response {
                id: Id::Text("s4".into()),
                result: json!({"account": null}),
            })
        );
        let error = Message::parse(r#"{"error":{"code":-32600,"message":"Not initialized"},"id":0}"#);
        assert_eq!(
            error,
            Some(Message::Error {
                id: Some(Id::Number(0)),
                error: RpcError::new(RpcError::INVALID_REQUEST, "Not initialized"),
            })
        );
    }

    #[test]
    fn what_is_no_message_is_skipped() {
        for line in ["", "garbage", "[1,2]", r#"{"id":1}"#, r#"{"error":"text","id":1}"#] {
            assert_eq!(Message::parse(line), None, "{line}");
        }
    }

    #[test]
    fn written_lines_carry_no_jsonrpc_member_and_no_empty_params() {
        let line = Message::Request {
            id: Id::Number(1),
            method: "config/mcpServer/reload".into(),
            params: Value::Null,
        }
        .encode();
        assert_eq!(line, r#"{"id":1,"method":"config/mcpServer/reload"}"#);
        let line = Message::Notification {
            method: "initialized".into(),
            params: Value::Null,
        }
        .encode();
        assert_eq!(line, r#"{"method":"initialized"}"#);
        let line = Message::Response {
            id: Id::Number(900),
            result: json!({"decision": "accept"}),
        }
        .encode();
        assert!(!line.contains("jsonrpc") && !line.contains('\n'), "{line}");
        assert_eq!(Message::parse(&line).map(|m| m.encode()), Some(line));
    }
}
