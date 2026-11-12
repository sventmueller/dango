//! Anthropic's Messages API: the request, the streamed answer and the model list, for Anthropic itself and for
//! servers that speak its protocol.
//!
//! Only Anthropic itself gets the newer request fields (automatic cache breakpoint, eagerly streamed tool input,
//! server-side refusal fallback); a server that copies the protocol may reject fields it does not know.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::chat::{Cancel, Delta, ErrorKind, ProviderError, Reply, Request, Stop, TurnUsage};
use crate::conversation::{Block, Message, Role, merged};
use crate::http::{self, Http};
use crate::keys::ApiKey;
use crate::models::ModelInfo;
use crate::provider::{Endpoint, Wire};
use crate::sse::Events;

/// The API version Anthropic itself is asked for.
const VERSION: &str = "2026-10-21";
/// The version servers that copy the protocol know.
const COMPATIBLE_VERSION: &str = "2023-06-01";
/// The beta of `fallbacks: "default"`.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// The answer's token limit when none is set, as the API recommends for streamed requests.
const MAX_TOKENS: u64 = 64_000;
/// The same for servers that copy the protocol, whose models often allow less.
const COMPATIBLE_MAX_TOKENS: u64 = 32_000;

/// Whether `model` thinks adaptively and takes an effort; older models take a fixed budget, which the agent does not
/// use.
pub(crate) fn adaptive(model: &str) -> bool {
    [
        "claude-fable-5",
        "claude-mythos-5",
        "claude-opus-5",
        "claude-opus-4-6",
        "claude-opus-4-7",
        "claude-opus-4-8",
        "claude-sonnet-5",
        "claude-sonnet-4-6",
        "claude-haiku-5",
    ]
    .iter()
    .any(|family| model.starts_with(family))
}

/// Whether Anthropic answers a refusal of `model` with another model when asked to.
fn takes_fallbacks(model: &str) -> bool {
    [
        "claude-fable-5-1",
        "claude-opus-5-5",
        "claude-opus-5",
        "claude-sonnet-5-5",
    ]
    .contains(&model)
}

/// The headers of a request to `endpoint`.
fn headers(endpoint: &Endpoint, key: Option<&ApiKey>, model: Option<&str>) -> Vec<(&'static str, String)> {
    let first_party = endpoint.is_first_party();
    let mut headers = vec![(
        "anthropic-version",
        if first_party { VERSION } else { COMPATIBLE_VERSION }.to_string(),
    )];
    if let Some(key) = key {
        headers.push(("x-api-key", key.expose().to_string()));
        if !first_party {
            headers.push(("authorization", format!("Bearer {}", key.expose())));
        }
    }
    if first_party && model.is_some_and(takes_fallbacks) {
        headers.push(("anthropic-beta", FALLBACK_BETA.to_string()));
    }
    headers
}

/// The request body.
pub(crate) fn body(request: &Request<'_>, first_party: bool) -> Value {
    let tools: Vec<Value> = request
        .tools
        .iter()
        .map(|tool| {
            let mut spec = json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.schema,
            });
            if first_party {
                spec["eager_input_streaming"] = json!(true);
            }
            spec
        })
        .collect();
    let limit = if first_party { MAX_TOKENS } else { COMPATIBLE_MAX_TOKENS };
    let mut body = json!({
        "model": request.model,
        "max_tokens": request.max_tokens.unwrap_or(limit),
        "stream": true,
        "system": [{"type": "text", "text": request.system, "cache_control": {"type": "ephemeral"}}],
        "messages": messages(request.messages),
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
    }
    if adaptive(request.model) {
        body["thinking"] = json!({"type": "adaptive", "display": "summarized"});
        if let Some(effort) = request.effort {
            body["output_config"] = json!({"effort": effort});
        }
    }
    if first_party {
        body["cache_control"] = json!({"type": "ephemeral"});
        if takes_fallbacks(request.model) {
            body["fallbacks"] = json!("default");
        }
    }
    body
}

/// The conversation in the API's shape: thinking only from this protocol, no empty text, one message per role.
fn messages(messages: &[Message]) -> Vec<Value> {
    merged(messages)
        .iter()
        .filter_map(|message| {
            let content: Vec<Value> = message.content.iter().filter_map(block).collect();
            if content.is_empty() {
                return None;
            }
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            Some(json!({"role": role, "content": content}))
        })
        .collect()
}

fn block(block: &Block) -> Option<Value> {
    match block {
        Block::Text { text, .. } if text.is_empty() => None,
        Block::Text { text, .. } => Some(json!({"type": "text", "text": text})),
        Block::Thinking {
            text,
            signature: Some(signature),
            wire: Wire::Anthropic,
        } => Some(json!({"type": "thinking", "thinking": text, "signature": signature})),
        Block::Thinking { .. } => None,
        Block::RedactedThinking { data } => Some(json!({"type": "redacted_thinking", "data": data})),
        Block::Image { media_type, data } => Some(json!({
            "type": "image",
            "source": {"type": "base64", "media_type": media_type, "data": data},
        })),
        Block::ToolUse { id, name, input, .. } => {
            Some(json!({"type": "tool_use", "id": id, "name": name, "input": input}))
        }
        Block::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } => Some(json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": content, "is_error": is_error})),
    }
}

/// A content block while it streams.
enum Slot {
    Text(String),
    Thinking { text: String, signature: Option<String> },
    Redacted(String),
    Tool { id: String, name: String, json: String },
    Ignored,
}

/// What the stream has said so far.
/// Takes `events` as a stream would and builds the message, for the fuzz target.
#[cfg(feature = "fuzzing")]
pub(crate) fn read_all(events: &[Value]) {
    let mut answer = Answer::default();
    for event in events {
        if !matches!(answer.take(event, &mut |_| {}), Ok(false)) {
            break;
        }
    }
    let _ = answer.message(true, true);
}

#[derive(Default)]
struct Answer {
    slots: BTreeMap<u64, Slot>,
    usage: TurnUsage,
    stop: Option<Stop>,
    model: Option<String>,
}

impl Answer {
    /// Takes one event; true once the message is complete.
    fn take(&mut self, event: &Value, on: &mut dyn FnMut(Delta)) -> Result<bool, ProviderError> {
        let index = event.get("index").and_then(Value::as_u64).unwrap_or(0);
        match event.get("type").and_then(Value::as_str).unwrap_or_default() {
            "message_start" => {
                let message = &event["message"];
                self.model = message.get("model").and_then(Value::as_str).map(str::to_string);
                self.usage(&message["usage"]);
            }
            "content_block_start" => self.start(index, &event["content_block"], on),
            "content_block_delta" => self.delta(index, &event["delta"], on),
            "message_delta" => {
                self.usage(&event["usage"]);
                if let Some(reason) = event["delta"].get("stop_reason").and_then(Value::as_str) {
                    let category = event["delta"]
                        .get("stop_details")
                        .and_then(|d| d.get("category"))
                        .and_then(Value::as_str);
                    self.stop = Some(stop(reason, category));
                }
            }
            "message_stop" => return Ok(true),
            "error" => return Err(http::stream_error(&event["error"])),
            _ => {}
        }
        Ok(false)
    }

    fn usage(&mut self, usage: &Value) {
        let field = |name: &str| usage.get(name).and_then(Value::as_u64);
        if let Some(n) = field("input_tokens") {
            self.usage.input = n;
        }
        if let Some(n) = field("output_tokens") {
            self.usage.output = n;
        }
        if let Some(n) = field("cache_read_input_tokens") {
            self.usage.cache_read = n;
        }
        if let Some(n) = field("cache_creation_input_tokens") {
            self.usage.cache_write = n;
        }
    }

    fn start(&mut self, index: u64, block: &Value, on: &mut dyn FnMut(Delta)) {
        let text = |name: &str| block.get(name).and_then(Value::as_str).unwrap_or_default().to_string();
        let slot = match block.get("type").and_then(Value::as_str).unwrap_or_default() {
            "text" => Slot::Text(text("text")),
            "thinking" => Slot::Thinking {
                text: text("thinking"),
                signature: block
                    .get("signature")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            },
            "redacted_thinking" => Slot::Redacted(text("data")),
            "tool_use" => {
                on(Delta::ToolStarted {
                    id: text("id"),
                    name: text("name"),
                });
                Slot::Tool {
                    id: text("id"),
                    name: text("name"),
                    json: String::new(),
                }
            }
            "fallback" => {
                self.slots.retain(|_, slot| matches!(slot, Slot::Text(_)));
                let model = |side: &str| {
                    block[side]
                        .get("model")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                on(Delta::Fallback {
                    from: model("from"),
                    to: model("to"),
                });
                Slot::Ignored
            }
            _ => Slot::Ignored,
        };
        self.slots.insert(index, slot);
    }

    fn delta(&mut self, index: u64, delta: &Value, on: &mut dyn FnMut(Delta)) {
        let Some(slot) = self.slots.get_mut(&index) else {
            return;
        };
        let text = |name: &str| delta.get(name).and_then(Value::as_str).unwrap_or_default().to_string();
        match (slot, delta.get("type").and_then(Value::as_str).unwrap_or_default()) {
            (Slot::Text(all), "text_delta") => {
                let part = text("text");
                all.push_str(&part);
                on(Delta::Text(part));
            }
            (Slot::Thinking { text: all, .. }, "thinking_delta") => {
                let part = text("thinking");
                all.push_str(&part);
                on(Delta::Thinking(part));
            }
            (Slot::Thinking { signature, .. }, "signature_delta") => *signature = Some(text("signature")),
            (Slot::Tool { id, json, .. }, "input_json_delta") => {
                let part = text("partial_json");
                json.push_str(&part);
                on(Delta::ToolInput {
                    id: id.clone(),
                    json: part,
                });
            }
            _ => {}
        }
    }

    /// The message: only its text when it was cut short, no tool calls when the model was stopped by a refusal,
    /// which can cut a call's input off.
    fn message(self, complete: bool, tools: bool) -> Message {
        let content = self
            .slots
            .into_values()
            .filter_map(|slot| match slot {
                Slot::Text(text) if !text.is_empty() => Some(Block::text(&text)),
                _ if !complete => None,
                Slot::Tool { .. } if !tools => None,
                Slot::Thinking {
                    text,
                    signature: Some(signature),
                } => Some(Block::Thinking {
                    text,
                    signature: Some(signature),
                    wire: Wire::Anthropic,
                }),
                Slot::Redacted(data) => Some(Block::RedactedThinking { data }),
                Slot::Tool { id, name, json } => Some(tool_use(id, name, &json)),
                _ => None,
            })
            .collect();
        Message {
            role: Role::Assistant,
            content,
        }
    }
}

/// A finished tool call; input that is not a JSON object is kept aside and the call gets an empty object.
pub(crate) fn tool_use(id: String, name: String, json: &str) -> Block {
    let parsed = if json.trim().is_empty() {
        Ok(Value::Object(Map::new()))
    } else {
        serde_json::from_str::<Value>(json)
    };
    let (input, invalid) = match parsed {
        Ok(Value::Object(object)) => (Value::Object(object), None),
        _ => (Value::Object(Map::new()), Some(json.to_string())),
    };
    Block::ToolUse {
        id,
        name,
        input,
        signature: None,
        invalid,
    }
}

fn stop(reason: &str, category: Option<&str>) -> Stop {
    match reason {
        "end_turn" | "stop_sequence" => Stop::EndTurn,
        "tool_use" => Stop::ToolUse,
        "max_tokens" | "model_context_window_exceeded" => Stop::MaxTokens,
        "refusal" => Stop::Refusal(category.map(str::to_string)),
        other => Stop::Other(other.to_string()),
    }
}

/// Sends `request` and reads the answer as it streams, telling `on` each part.
pub(crate) fn stream(
    http: &Http,
    endpoint: &Endpoint,
    key: Option<&ApiKey>,
    request: &Request<'_>,
    cancel: &Cancel,
    on: &mut dyn FnMut(Delta),
) -> Result<Reply, ProviderError> {
    let mut headers = headers(endpoint, key, Some(request.model));
    headers.push(("accept", "text/event-stream".to_string()));
    let body = body(request, endpoint.is_first_party());
    let reader = http.post_stream(&endpoint.url("v1/messages"), &headers, &body)?;
    let mut answer = Answer::default();
    let mut complete = false;
    for event in Events::new(reader) {
        if cancel.is_cancelled() {
            break;
        }
        let event = event.map_err(|e| ProviderError::new(ErrorKind::Network, e.to_string()))?;
        if answer.take(&http::parse(&event.data)?, on)? {
            complete = true;
            break;
        }
    }
    if cancel.is_cancelled() {
        let (usage, model) = (answer.usage, answer.model.clone());
        return Ok(Reply {
            message: answer.message(false, false),
            usage,
            stop: Stop::Cancelled,
            model,
        });
    }
    let Some(stop) = answer.stop.clone().filter(|_| complete) else {
        return Err(ProviderError::new(ErrorKind::Incomplete, ""));
    };
    let (usage, model) = (answer.usage, answer.model.clone());
    Ok(Reply {
        message: answer.message(true, !matches!(stop, Stop::Refusal(_))),
        usage,
        stop,
        model,
    })
}

/// The models the key may use, newest first as the API lists them.
pub(crate) fn models(http: &Http, endpoint: &Endpoint, key: Option<&ApiKey>) -> Result<Vec<ModelInfo>, ProviderError> {
    let headers = headers(endpoint, key, None);
    let mut out = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let mut url = endpoint.url("v1/models?limit=1000");
        if let Some(after) = &after {
            url.push_str(&format!("&after_id={after}"));
        }
        let page = http.get_json(&url, &headers)?;
        for model in page.get("data").and_then(Value::as_array).into_iter().flatten() {
            let Some(id) = model.get("id").and_then(Value::as_str) else {
                continue;
            };
            out.push(ModelInfo {
                id: id.to_string(),
                name: model.get("display_name").and_then(Value::as_str).map(str::to_string),
                context: model.get("max_input_tokens").and_then(Value::as_u64),
                max_output: model.get("max_tokens").and_then(Value::as_u64),
                price: crate::prices::anthropic(id).filter(|_| endpoint.is_first_party()),
            });
        }
        let more = page.get("has_more").and_then(Value::as_bool).unwrap_or(false);
        after = page.get("last_id").and_then(Value::as_str).map(str::to_string);
        if !more || after.is_none() {
            return Ok(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::ToolSpec;

    fn request<'a>(messages: &'a [Message], tools: &'a [ToolSpec], model: &'a str) -> Request<'a> {
        Request {
            model,
            system: "Be brief.",
            messages,
            tools,
            max_tokens: None,
            effort: Some("high"),
        }
    }

    #[test]
    fn anthropic_itself_gets_caching_fallbacks_and_eager_tool_input_and_others_do_not() {
        let tools = [ToolSpec {
            name: "read_file".into(),
            description: "Reads".into(),
            schema: json!({"type": "object"}),
        }];
        let messages = [Message::user("hi")];
        let first = body(&request(&messages, &tools, "claude-opus-5-5"), true);
        assert_eq!(first["cache_control"]["type"], "ephemeral");
        assert_eq!(first["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(first["fallbacks"], "default");
        assert_eq!(first["tools"][0]["eager_input_streaming"], true);
        assert_eq!(first["thinking"]["type"], "adaptive");
        assert_eq!(first["output_config"]["effort"], "high");
        assert_eq!(first["max_tokens"], 64_000);
        let other = body(&request(&messages, &tools, "claude-opus-5-5"), false);
        assert!(other.get("cache_control").is_none() && other.get("fallbacks").is_none());
        assert!(other["tools"][0].get("eager_input_streaming").is_none());
        let old = body(&request(&messages, &tools, "claude-haiku-4-5"), true);
        assert!(old.get("thinking").is_none() && old.get("output_config").is_none());
        assert!(old.get("fallbacks").is_none());
    }

    #[test]
    fn the_conversation_keeps_its_own_thinking_drops_others_and_empty_text() {
        let messages = [
            Message::user("hi"),
            Message {
                role: Role::Assistant,
                content: vec![
                    Block::Thinking {
                        text: String::new(),
                        signature: Some("sig".into()),
                        wire: Wire::Anthropic,
                    },
                    Block::Thinking {
                        text: "other".into(),
                        signature: None,
                        wire: Wire::Gemini,
                    },
                    Block::text(""),
                    tool_use("t1".into(), "read_file".into(), r#"{"path":"a"}"#),
                ],
            },
            Message {
                role: Role::User,
                content: vec![Block::ToolResult {
                    tool_use_id: "t1".into(),
                    name: "read_file".into(),
                    content: "x".into(),
                    is_error: false,
                    note: None,
                }],
            },
            Message::user("and?"),
        ];
        let sent = super::messages(&messages);
        assert_eq!(sent.len(), 3, "the tool result and the next question merge");
        assert_eq!(sent[1]["content"].as_array().unwrap().len(), 2);
        assert_eq!(sent[1]["content"][0]["signature"], "sig");
        assert_eq!(sent[2]["content"][0]["type"], "tool_result");
        assert_eq!(sent[2]["content"][1]["text"], "and?");
    }

    #[test]
    fn tool_input_that_is_no_object_is_kept_aside() {
        let Block::ToolUse { input, invalid, .. } = tool_use("t".into(), "x".into(), "[1,") else {
            panic!()
        };
        assert_eq!((input, invalid.as_deref()), (json!({}), Some("[1,")));
        let Block::ToolUse { invalid, .. } = tool_use("t".into(), "x".into(), "") else {
            panic!()
        };
        assert_eq!(invalid, None);
    }
}
