//! Google's Gemini API: `streamGenerateContent` with function calls, thought summaries and the model list.
//!
//! Gemini attaches signatures of its reasoning to parts; each must go back on the part it came with, or the model
//! loses its reasoning between a call and its result. A function response names its call's id and name, or the
//! model tends to answer with nothing.

use serde_json::{Value, json};

use crate::chat::{Cancel, Delta, ErrorKind, ProviderError, Reply, Request, Stop, TurnUsage};
use crate::conversation::{Block, Message, Role, merged};
use crate::http::{self, Http};
use crate::keys::ApiKey;
use crate::models::ModelInfo;
use crate::provider::{Endpoint, Wire};
use crate::sse::Events;

/// The start of the ids this app gives calls that came without one; such ids are not sent back.
const LOCAL_ID: &str = "gemini-call-";

fn headers(key: Option<&ApiKey>) -> Vec<(&'static str, String)> {
    key.map(|key| vec![("x-goog-api-key", key.expose().to_string())])
        .unwrap_or_default()
}

/// The request body.
pub(crate) fn body(request: &Request<'_>) -> Value {
    let contents: Vec<Value> = merged(request.messages).iter().filter_map(content).collect();
    let mut thinking = json!({"includeThoughts": true});
    if let Some(effort) = request.effort {
        thinking["thinkingLevel"] = json!(effort);
    }
    let mut config = json!({"thinkingConfig": thinking});
    if let Some(max) = request.max_tokens {
        config["maxOutputTokens"] = json!(max);
    }
    let mut body = json!({
        "systemInstruction": {"parts": [{"text": request.system}]},
        "contents": contents,
        "generationConfig": config,
    });
    if !request.tools.is_empty() {
        let declarations: Vec<Value> = request
            .tools
            .iter()
            .map(
                |tool| json!({"name": tool.name, "description": tool.description, "parametersJsonSchema": tool.schema}),
            )
            .collect();
        body["tools"] = json!([{"functionDeclarations": declarations}]);
    }
    body
}

fn signed(mut part: Value, signature: Option<&String>) -> Value {
    if let Some(signature) = signature {
        part["thoughtSignature"] = json!(signature);
    }
    part
}

/// One message as Gemini content.
fn content(message: &Message) -> Option<Value> {
    let mut parts = Vec::new();
    for block in &message.content {
        match block {
            Block::Text { text, signature } if !text.is_empty() || signature.is_some() => {
                parts.push(signed(json!({"text": text}), signature.as_ref()));
            }
            Block::Thinking {
                text,
                signature: Some(signature),
                wire: Wire::Gemini,
            } => parts.push(signed(json!({"text": text, "thought": true}), Some(signature))),
            Block::ToolUse {
                id,
                name,
                input,
                signature,
                ..
            } => {
                let mut call = json!({"name": name, "args": input});
                if !id.starts_with(LOCAL_ID) {
                    call["id"] = json!(id);
                }
                parts.push(signed(json!({"functionCall": call}), signature.as_ref()));
            }
            Block::Image { media_type, data } => {
                parts.push(json!({"inlineData": {"mimeType": media_type, "data": data}}));
            }
            Block::ToolResult {
                tool_use_id,
                name,
                content,
                is_error,
                ..
            } => {
                let response = if *is_error {
                    json!({"error": content})
                } else {
                    json!({"result": content})
                };
                let mut answer = json!({"name": name, "response": response});
                if !tool_use_id.starts_with(LOCAL_ID) {
                    answer["id"] = json!(tool_use_id);
                }
                parts.push(json!({"functionResponse": answer}));
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        return None;
    }
    let role = match message.role {
        Role::User => "user",
        Role::Assistant => "model",
    };
    Some(json!({"role": role, "parts": parts}))
}

/// What the stream has said so far.
/// Takes `chunks` as a stream would, for the fuzz target.
#[cfg(feature = "fuzzing")]
pub(crate) fn read_all(chunks: &[Value]) {
    let mut answer = Answer::default();
    for chunk in chunks {
        if answer.take(chunk, &mut |_| {}).is_err() {
            break;
        }
    }
}

#[derive(Default)]
struct Answer {
    blocks: Vec<Block>,
    calls: usize,
    usage: TurnUsage,
    finish: Option<String>,
    blocked: Option<String>,
    model: Option<String>,
}

impl Answer {
    fn take(&mut self, chunk: &Value, on: &mut dyn FnMut(Delta)) -> Result<(), ProviderError> {
        if let Some(error) = chunk.get("error") {
            return Err(http::stream_error(error));
        }
        if let Some(model) = chunk.get("modelVersion").and_then(Value::as_str) {
            self.model = Some(model.to_string());
        }
        if let Some(reason) = chunk["promptFeedback"].get("blockReason").and_then(Value::as_str) {
            self.blocked = Some(reason.to_ascii_lowercase());
        }
        if let Some(usage) = chunk.get("usageMetadata") {
            let field = |name: &str| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
            let read = field("cachedContentTokenCount");
            self.usage = TurnUsage {
                input: field("promptTokenCount").saturating_sub(read),
                output: field("candidatesTokenCount") + field("thoughtsTokenCount"),
                cache_read: read,
                cache_write: 0,
                cost_usd: None,
            };
        }
        let Some(candidate) = chunk.get("candidates").and_then(|c| c.get(0)) else {
            return Ok(());
        };
        if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
            self.finish = Some(reason.to_string());
        }
        for part in candidate["content"]
            .get("parts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.part(part, on);
        }
        Ok(())
    }

    fn part(&mut self, part: &Value, on: &mut dyn FnMut(Delta)) {
        let signature = part.get("thoughtSignature").and_then(Value::as_str).map(str::to_string);
        if let Some(call) = part.get("functionCall") {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map_or_else(|| format!("{LOCAL_ID}{}", self.calls), str::to_string);
            self.calls += 1;
            let name = call.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
            let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
            on(Delta::ToolStarted {
                id: id.clone(),
                name: name.clone(),
            });
            on(Delta::ToolInput {
                id: id.clone(),
                json: args.to_string(),
            });
            let mut block = crate::anthropic::tool_use(id, name, &args.to_string());
            if let Block::ToolUse { signature: slot, .. } = &mut block {
                *slot = signature;
            }
            self.blocks.push(block);
            return;
        }
        let Some(text) = part.get("text").and_then(Value::as_str) else {
            return;
        };
        let thought = part.get("thought").and_then(Value::as_bool).unwrap_or(false);
        if thought {
            on(Delta::Thinking(text.to_string()));
        } else if !text.is_empty() {
            on(Delta::Text(text.to_string()));
        }
        match (self.blocks.last_mut(), thought) {
            (
                Some(Block::Thinking {
                    text: all,
                    signature: slot,
                    ..
                }),
                true,
            )
            | (
                Some(Block::Text {
                    text: all,
                    signature: slot,
                }),
                false,
            ) => {
                all.push_str(text);
                if signature.is_some() {
                    *slot = signature;
                }
            }
            (_, true) => self.blocks.push(Block::Thinking {
                text: text.to_string(),
                signature,
                wire: Wire::Gemini,
            }),
            (_, false) => self.blocks.push(Block::Text {
                text: text.to_string(),
                signature,
            }),
        }
    }
}

fn stop(reason: &str, calls: bool) -> Stop {
    match reason {
        "STOP" if calls => Stop::ToolUse,
        "STOP" => Stop::EndTurn,
        "MAX_TOKENS" => Stop::MaxTokens,
        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" | "IMAGE_SAFETY" => {
            Stop::Refusal(Some(reason.to_ascii_lowercase()))
        }
        other => Stop::Other(other.to_ascii_lowercase()),
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
    let url = endpoint.url(&format!("models/{}:streamGenerateContent?alt=sse", request.model));
    let reader = http.post_stream(&url, &headers(key), &body(request))?;
    let mut answer = Answer::default();
    for event in Events::new(reader) {
        if cancel.is_cancelled() {
            break;
        }
        let event = event.map_err(|e| ProviderError::new(ErrorKind::Network, e.to_string()))?;
        answer.take(&http::parse(&event.data)?, on)?;
    }
    let (usage, model) = (answer.usage, answer.model.clone());
    if cancel.is_cancelled() {
        answer
            .blocks
            .retain(|b| matches!(b, Block::Text { text, .. } if !text.is_empty()));
        return Ok(Reply {
            message: Message {
                role: Role::Assistant,
                content: answer.blocks,
            },
            usage,
            stop: Stop::Cancelled,
            model,
        });
    }
    let stop = match (&answer.blocked, &answer.finish) {
        (Some(reason), _) => Stop::Refusal(Some(reason.clone())),
        (None, Some(reason)) => stop(reason, answer.calls > 0),
        (None, None) => {
            return Err(ProviderError::new(ErrorKind::Incomplete, ""));
        }
    };
    if matches!(stop, Stop::Refusal(_)) {
        answer.blocks.retain(|b| !matches!(b, Block::ToolUse { .. }));
    }
    Ok(Reply {
        message: Message {
            role: Role::Assistant,
            content: answer.blocks,
        },
        usage,
        stop,
        model,
    })
}

/// The models the key may use that generate content.
pub(crate) fn models(http: &Http, endpoint: &Endpoint, key: Option<&ApiKey>) -> Result<Vec<ModelInfo>, ProviderError> {
    let mut out = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let mut url = endpoint.url("models?pageSize=1000");
        if let Some(token) = &token {
            url.push_str(&format!("&pageToken={token}"));
        }
        let page = http.get_json(&url, &headers(key))?;
        for model in page.get("models").and_then(Value::as_array).into_iter().flatten() {
            let generates = model["supportedGenerationMethods"]
                .as_array()
                .is_some_and(|methods| methods.iter().any(|m| m == "generateContent"));
            let Some(name) = model.get("name").and_then(Value::as_str).filter(|_| generates) else {
                continue;
            };
            out.push(ModelInfo {
                id: name.trim_start_matches("models/").to_string(),
                name: model.get("displayName").and_then(Value::as_str).map(str::to_string),
                context: model.get("inputTokenLimit").and_then(Value::as_u64),
                max_output: model.get("outputTokenLimit").and_then(Value::as_u64),
                price: None,
            });
        }
        token = page
            .get("nextPageToken")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string);
        if token.is_none() {
            return Ok(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_and_answers_keep_their_ids_and_signatures_and_local_ids_stay_home() {
        let mut call = crate::anthropic::tool_use("c1".into(), "read_file".into(), r#"{"path":"a"}"#);
        if let Block::ToolUse { signature, .. } = &mut call {
            *signature = Some("sig".into());
        }
        let local = crate::anthropic::tool_use(format!("{LOCAL_ID}1"), "glob".into(), "{}");
        let messages = [
            Message::user("hi"),
            Message {
                role: Role::Assistant,
                content: vec![call, local],
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
                    Block::ToolResult {
                        tool_use_id: format!("{LOCAL_ID}1"),
                        name: "glob".into(),
                        content: "no".into(),
                        is_error: true,
                        note: None,
                    },
                ],
            },
        ];
        let request = Request {
            model: "gemini-x",
            system: "sys",
            messages: &messages,
            tools: &[],
            max_tokens: None,
            effort: None,
        };
        let sent = body(&request);
        let model = &sent["contents"][1];
        assert_eq!(model["role"], "model");
        assert_eq!(model["parts"][0]["thoughtSignature"], "sig");
        assert_eq!(model["parts"][0]["functionCall"]["id"], "c1");
        assert!(model["parts"][1]["functionCall"].get("id").is_none());
        let answers = &sent["contents"][2]["parts"];
        assert_eq!(answers[0]["functionResponse"]["response"]["result"], "x");
        assert_eq!(answers[1]["functionResponse"]["response"]["error"], "no");
        assert!(
            sent["generationConfig"]["thinkingConfig"]
                .get("thinkingLevel")
                .is_none()
        );
    }
}
