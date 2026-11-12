//! OpenAI's chat completions, which OpenAI, OpenRouter, Ollama, LM Studio and many other servers speak: the request,
//! the streamed answer and the model list.
//!
//! Reasoning arrives under different names: OpenRouter and Ollama send `reasoning`, LM Studio and DeepSeek-style
//! servers `reasoning_content`; OpenAI itself streams none. It is shown but never sent back, which some servers
//! reject.

use serde_json::{Value, json};

use crate::chat::{Cancel, Delta, ErrorKind, ProviderError, Reply, Request, Stop, TurnUsage};
use crate::conversation::{Block, Message, Role, merged};
use crate::http::{self, Http};
use crate::keys::ApiKey;
use crate::models::ModelInfo;
use crate::prices::Price;
use crate::provider::{Endpoint, ProviderKind, Wire};
use crate::sse::Events;

fn headers(key: Option<&ApiKey>) -> Vec<(&'static str, String)> {
    key.map(|key| vec![("authorization", format!("Bearer {}", key.expose()))])
        .unwrap_or_default()
}

/// The request body.
pub(crate) fn body(request: &Request<'_>, kind: ProviderKind) -> Value {
    let mut messages = vec![json!({"role": "system", "content": request.system})];
    for message in merged(request.messages) {
        messages.extend(self::message(&message));
    }
    let mut body = json!({
        "model": request.model,
        "stream": true,
        "stream_options": {"include_usage": true},
        "messages": messages,
    });
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|tool| {
                json!({"type": "function", "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.schema,
                }})
            })
            .collect();
    }
    if let Some(max) = request.max_tokens {
        let field = if kind == ProviderKind::OpenAi {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        body[field] = json!(max);
    }
    if let Some(effort) = request.effort {
        if kind == ProviderKind::OpenRouter {
            body["reasoning"] = json!({"effort": effort});
        } else {
            body["reasoning_effort"] = json!(effort);
        }
    }
    body
}

/// One message of the conversation as one or more chat messages; tool results come first, as their own messages.
fn message(message: &Message) -> Vec<Value> {
    match message.role {
        Role::User => {
            let mut out: Vec<Value> = message
                .content
                .iter()
                .filter_map(|block| match block {
                    Block::ToolResult {
                        tool_use_id, content, ..
                    } => Some(json!({"role": "tool", "tool_call_id": tool_use_id, "content": content})),
                    _ => None,
                })
                .collect();
            let text = message.text();
            let images: Vec<Value> = message
                .content
                .iter()
                .filter_map(|block| match block {
                    Block::Image { media_type, data } => Some(json!({
                        "type": "image_url",
                        "image_url": {"url": format!("data:{media_type};base64,{data}")},
                    })),
                    _ => None,
                })
                .collect();
            if !images.is_empty() {
                let mut parts = vec![json!({"type": "text", "text": text})];
                parts.extend(images);
                out.push(json!({"role": "user", "content": parts}));
            } else if !text.is_empty() {
                out.push(json!({"role": "user", "content": text}));
            }
            out
        }
        Role::Assistant => {
            let calls: Vec<Value> = message
                .tool_uses()
                .map(|(id, name, input)| {
                    json!({"id": id, "type": "function", "function": {"name": name, "arguments": input.to_string()}})
                })
                .collect();
            let text = message.text();
            if text.is_empty() && calls.is_empty() {
                return vec![];
            }
            let mut out =
                json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) }});
            if !calls.is_empty() {
                out["tool_calls"] = Value::Array(calls);
            }
            vec![out]
        }
    }
}

/// A tool call while it streams.
#[derive(Default)]
struct Call {
    id: Option<String>,
    name: String,
    arguments: String,
    announced: bool,
}

/// What the stream has said so far.
/// Takes `chunks` as a stream would and builds the message, for the fuzz target.
#[cfg(feature = "fuzzing")]
pub(crate) fn read_all(chunks: &[Value]) {
    let mut answer = Answer::default();
    for chunk in chunks {
        if answer.take(chunk, &mut |_| {}).is_err() {
            break;
        }
    }
    let _ = answer.message(true);
}

#[derive(Default)]
struct Answer {
    text: String,
    thinking: String,
    calls: Vec<Call>,
    usage: TurnUsage,
    finish: Option<String>,
    model: Option<String>,
}

impl Answer {
    fn take(&mut self, chunk: &Value, on: &mut dyn FnMut(Delta)) -> Result<(), ProviderError> {
        if let Some(error) = chunk.get("error") {
            return Err(http::stream_error(error));
        }
        if let Some(model) = chunk.get("model").and_then(Value::as_str) {
            self.model = Some(model.to_string());
        }
        if let Some(usage) = chunk.get("usage").filter(|u| u.is_object()) {
            self.usage(usage);
        }
        let Some(choice) = chunk.get("choices").and_then(|c| c.get(0)) else {
            return Ok(());
        };
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish = Some(reason.to_string());
        }
        let delta = &choice["delta"];
        if let Some(text) = delta.get("content").and_then(Value::as_str).filter(|t| !t.is_empty()) {
            self.text.push_str(text);
            on(Delta::Text(text.to_string()));
        }
        for field in ["reasoning", "reasoning_content"] {
            if let Some(text) = delta.get(field).and_then(Value::as_str).filter(|t| !t.is_empty()) {
                self.thinking.push_str(text);
                on(Delta::Thinking(text.to_string()));
            }
        }
        for (position, call) in delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            self.call(position, call, on);
        }
        Ok(())
    }

    fn call(&mut self, position: usize, part: &Value, on: &mut dyn FnMut(Delta)) {
        let index = part
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|i| usize::try_from(i).ok())
            .unwrap_or(position);
        if self.calls.len() <= index {
            self.calls.resize_with(index + 1, Call::default);
        }
        let generated = format!("call_{index}");
        let call = &mut self.calls[index];
        if let Some(id) = part.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()) {
            call.id = Some(id.to_string());
        }
        let function = &part["function"];
        if let Some(name) = function.get("name").and_then(Value::as_str) {
            call.name.push_str(name);
        }
        let id = call.id.clone().unwrap_or(generated);
        if !call.announced && !call.name.is_empty() {
            call.announced = true;
            on(Delta::ToolStarted {
                id: id.clone(),
                name: call.name.clone(),
            });
        }
        let arguments = match function.get("arguments") {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Object(object)) => Value::Object(object.clone()).to_string(),
            _ => String::new(),
        };
        if !arguments.is_empty() {
            call.arguments.push_str(&arguments);
            on(Delta::ToolInput { id, json: arguments });
        }
    }

    fn usage(&mut self, usage: &Value) {
        let field = |value: &Value, name: &str| value.get(name).and_then(Value::as_u64).unwrap_or(0);
        let details = &usage["prompt_tokens_details"];
        let read = field(details, "cached_tokens");
        let write = field(details, "cache_write_tokens");
        self.usage = TurnUsage {
            input: field(usage, "prompt_tokens").saturating_sub(read + write),
            output: field(usage, "completion_tokens"),
            cache_read: read,
            cache_write: write,
            cost_usd: usage.get("cost").and_then(Value::as_f64),
        };
    }

    fn message(self, keep_calls: bool) -> Message {
        let mut content = Vec::new();
        if !self.thinking.is_empty() {
            content.push(Block::Thinking {
                text: self.thinking,
                signature: None,
                wire: Wire::OpenAiChat,
            });
        }
        if !self.text.is_empty() {
            content.push(Block::text(&self.text));
        }
        if keep_calls {
            for (index, call) in self.calls.into_iter().enumerate() {
                if call.name.is_empty() {
                    continue;
                }
                let id = call.id.unwrap_or_else(|| format!("call_{index}"));
                content.push(crate::anthropic::tool_use(id, call.name, &call.arguments));
            }
        }
        Message {
            role: Role::Assistant,
            content,
        }
    }
}

fn stop(reason: &str, calls: bool) -> Stop {
    match reason {
        "tool_calls" | "function_call" => Stop::ToolUse,
        "stop" if calls => Stop::ToolUse,
        "stop" => Stop::EndTurn,
        "length" => Stop::MaxTokens,
        "content_filter" => Stop::Refusal(Some("content_filter".into())),
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
    let mut headers = headers(key);
    headers.push(("accept", "text/event-stream".to_string()));
    let body = body(request, endpoint.kind);
    let reader = http.post_stream(&endpoint.url("chat/completions"), &headers, &body)?;
    let mut answer = Answer::default();
    for event in Events::new(reader) {
        if cancel.is_cancelled() {
            break;
        }
        let event = event.map_err(|e| ProviderError::new(ErrorKind::Network, e.to_string()))?;
        if event.data.trim() == "[DONE]" {
            break;
        }
        answer.take(&http::parse(&event.data)?, on)?;
    }
    let (usage, model) = (answer.usage, answer.model.clone());
    if cancel.is_cancelled() {
        let text = std::mem::take(&mut answer.text);
        return Ok(Reply {
            message: Message {
                role: Role::Assistant,
                content: if text.is_empty() {
                    vec![]
                } else {
                    vec![Block::text(&text)]
                },
            },
            usage,
            stop: Stop::Cancelled,
            model,
        });
    }
    let Some(reason) = answer.finish.clone() else {
        return Err(ProviderError::new(ErrorKind::Incomplete, ""));
    };
    let stop = stop(&reason, answer.calls.iter().any(|c| !c.name.is_empty()));
    let keep_calls = stop == Stop::ToolUse || stop == Stop::MaxTokens;
    Ok(Reply {
        message: answer.message(keep_calls),
        usage,
        stop,
        model,
    })
}

/// Whether a listed model is plainly not one to chat with, such as OpenAI's embedding or speech models.
fn not_for_chat(kind: ProviderKind, id: &str) -> bool {
    let words: &[&str] = match kind {
        ProviderKind::OpenAi => &[
            "embedding",
            "tts",
            "whisper",
            "dall-e",
            "transcribe",
            "moderation",
            "image",
            "audio",
            "realtime",
            "davinci",
            "babbage",
            "search",
        ],
        _ => &["embed"],
    };
    words.iter().any(|word| id.contains(word))
}

/// A price per token as OpenRouter lists it, in dollars per million tokens.
fn per_million(value: &Value) -> Option<f64> {
    let per_token = match value {
        Value::String(text) => text.parse::<f64>().ok()?,
        value => value.as_f64()?,
    };
    (per_token >= 0.0).then_some(per_token * 1_000_000.0)
}

/// The models the endpoint lists, those that cannot take tools left out where the list says.
pub(crate) fn models(http: &Http, endpoint: &Endpoint, key: Option<&ApiKey>) -> Result<Vec<ModelInfo>, ProviderError> {
    let list = http.get_json(&endpoint.url("models"), &headers(key))?;
    let models = list
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let id = model.get("id").and_then(Value::as_str)?;
            if not_for_chat(endpoint.kind, id) {
                return None;
            }
            let tools = model.get("supported_parameters").and_then(Value::as_array);
            if tools.is_some_and(|p| !p.iter().any(|p| p == "tools")) {
                return None;
            }
            let pricing = &model["pricing"];
            let price = per_million(&pricing["prompt"])
                .zip(per_million(&pricing["completion"]))
                .map(|(input, output)| {
                    let mut price = Price::new(input, output);
                    price.cache_read = per_million(&pricing["input_cache_read"]).unwrap_or(input);
                    price.cache_write = per_million(&pricing["input_cache_write"]).unwrap_or(input);
                    price
                });
            Some(ModelInfo {
                id: id.to_string(),
                name: model.get("name").and_then(Value::as_str).map(str::to_string),
                context: model.get("context_length").and_then(Value::as_u64),
                max_output: model["top_provider"]
                    .get("max_completion_tokens")
                    .and_then(Value::as_u64),
                price,
            })
        })
        .collect();
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::ToolSpec;

    #[test]
    fn tool_results_become_tool_messages_before_the_users_text() {
        let messages = [
            Message::user("hi"),
            Message {
                role: Role::Assistant,
                content: vec![
                    Block::Thinking {
                        text: "hm".into(),
                        signature: None,
                        wire: Wire::OpenAiChat,
                    },
                    crate::anthropic::tool_use("c1".into(), "read_file".into(), r#"{"path":"a"}"#),
                ],
            },
            Message {
                role: Role::User,
                content: vec![
                    Block::ToolResult {
                        tool_use_id: "c1".into(),
                        name: "read_file".into(),
                        content: "x".into(),
                        is_error: false,
                        note: None,
                    },
                    Block::text("more"),
                ],
            },
        ];
        let tools = [ToolSpec {
            name: "read_file".into(),
            description: "Reads".into(),
            schema: json!({"type": "object"}),
        }];
        let request = Request {
            model: "m",
            system: "sys",
            messages: &messages,
            tools: &tools,
            max_tokens: Some(100),
            effort: Some("low"),
        };
        let sent = body(&request, ProviderKind::OpenAi);
        let roles: Vec<&str> = sent["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["system", "user", "assistant", "tool", "user"]);
        assert_eq!(
            sent["messages"][2]["content"],
            Value::Null,
            "reasoning is not sent back"
        );
        assert_eq!(
            sent["messages"][2]["tool_calls"][0]["function"]["arguments"],
            r#"{"path":"a"}"#
        );
        assert_eq!(sent["max_completion_tokens"], 100);
        assert_eq!(sent["reasoning_effort"], "low");
        let routed = body(&request, ProviderKind::OpenRouter);
        assert_eq!(
            (routed["max_tokens"].clone(), routed["reasoning"]["effort"].clone()),
            (json!(100), json!("low"))
        );
    }

    #[test]
    fn listed_prices_per_token_become_prices_per_million() {
        assert_eq!(
            per_million(&json!("0.000003")).map(|p| (p * 1e6).round() / 1e6),
            Some(3.0)
        );
        assert!(not_for_chat(ProviderKind::OpenAi, "text-embedding-3-large"));
        assert!(!not_for_chat(ProviderKind::Ollama, "qwen3:8b"));
    }
}
