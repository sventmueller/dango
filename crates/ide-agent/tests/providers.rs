//! Every provider against a stub server answering with hand-made recordings of its stream: what is sent, what comes
//! back, a tool call's round trip, and the failures the loop has to tell apart. No request leaves this Mac.

use std::time::Duration;

use ide_agent::stub::{Answer, Stub};
use ide_agent::{
    ApiKey, Block, Cancel, Delta, Endpoint, ErrorKind, Http, Message, Provider, ProviderKind, Reply, Request, Role,
    Stop, ToolSpec, Wire,
};
use serde_json::{Value, json};

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn provider(stub: &Stub, kind: ProviderKind, key: Option<&str>) -> Provider {
    let base = match kind.wire() {
        Wire::Anthropic => stub.url(),
        Wire::OpenAiChat | Wire::Gemini => format!("{}/v1", stub.url()),
    };
    let endpoint = Endpoint::new(kind.id(), kind, "Stub", &base);
    Provider::new(endpoint, key.and_then(ApiKey::new), Http::new())
}

fn tools() -> Vec<ToolSpec> {
    ["read_file", "grep", "glob"]
        .iter()
        .map(|name| ToolSpec {
            name: (*name).into(),
            description: format!("The {name} tool."),
            schema: json!({"type": "object", "properties": {}}),
        })
        .collect()
}

fn ask(
    provider: &Provider,
    model: &str,
    messages: &[Message],
) -> (Result<Reply, ide_agent::ProviderError>, Vec<Delta>) {
    let tools = tools();
    let request = Request {
        model,
        system: "You are terse.",
        messages,
        tools: &tools,
        max_tokens: None,
        effort: None,
    };
    let mut deltas = Vec::new();
    let reply = provider.stream(&request, &Cancel::new(), &mut |delta| deltas.push(delta));
    (reply, deltas)
}

fn results(reply: &Reply, content: &str) -> Message {
    Message {
        role: Role::User,
        content: reply
            .message
            .tool_uses()
            .map(|(id, name, _)| Block::ToolResult {
                tool_use_id: id.to_string(),
                name: name.to_string(),
                content: content.to_string(),
                is_error: false,
                note: None,
            })
            .collect(),
    }
}

#[test]
fn anthropic_streams_thinking_text_and_a_tool_call_and_takes_its_result_back() {
    let stub = Stub::script(vec![
        Answer::events(&fixture("anthropic-tool-use.sse")),
        Answer::events(&fixture("anthropic-text.sse")),
    ])
    .unwrap();
    let provider = provider(&stub, ProviderKind::AnthropicCompatible, Some("sk-test"));
    let mut conversation = vec![Message::user("What does main.rs do?")];
    let (reply, deltas) = ask(&provider, "claude-opus-5-5", &conversation);
    let reply = reply.unwrap();
    assert_eq!(reply.stop, Stop::ToolUse);
    assert_eq!(reply.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(
        (reply.usage.input, reply.usage.cache_write, reply.usage.output),
        (12, 2000, 87)
    );
    assert!(deltas.contains(&Delta::Thinking("The user wants the file.".into())));
    assert!(deltas.contains(&Delta::ToolStarted {
        id: "toolu_01".into(),
        name: "read_file".into()
    }));
    let [
        Block::Thinking { signature, .. },
        Block::Text { text, .. },
        Block::ToolUse { input, invalid, .. },
    ] = reply.message.content.as_slice()
    else {
        panic!("{:?}", reply.message.content)
    };
    assert_eq!(signature.as_deref(), Some("EqQBCkYIBxgC"));
    assert_eq!(text, "Let me read it.");
    assert_eq!((input, invalid), (&json!({"path": "src/main.rs"}), &None));

    conversation.push(reply.message.clone());
    conversation.push(results(&reply, "fn main() { println!(\"hello\"); }"));
    let (second, _) = ask(&provider, "claude-opus-5-5", &conversation);
    let second = second.unwrap();
    assert_eq!(
        (second.stop, second.message.text()),
        (Stop::EndTurn, "It prints hello.".to_string())
    );
    assert_eq!(second.usage.cache_read, 2000);

    let seen = stub.seen();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        (seen[0].method.as_str(), seen[0].path.as_str()),
        ("POST", "/v1/messages")
    );
    assert_eq!(seen[0].header("x-api-key"), Some("sk-test"));
    assert_eq!(seen[0].header("authorization"), Some("Bearer sk-test"));
    assert_eq!(seen[0].header("anthropic-version"), Some("2023-06-01"));
    let sent = seen[1].json();
    let messages = sent["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1]["content"][0]["signature"], "EqQBCkYIBxgC");
    assert_eq!(messages[1]["content"][2]["id"], "toolu_01");
    assert_eq!(messages[2]["content"][0]["tool_use_id"], "toolu_01");
    assert_eq!(sent["system"][0]["text"], "You are terse.");
    assert_eq!(sent["tools"].as_array().unwrap().len(), 3);
}

#[test]
fn rate_limits_overload_and_bad_keys_are_told_apart() {
    let stub = Stub::script(vec![
        Answer::json(
            429,
            &json!({"type": "error", "error": {"type": "rate_limit_error", "message": "slow down"}}),
        )
        .with_header("retry-after", "3"),
        Answer::json(
            529,
            &json!({"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}}),
        ),
        Answer::json(
            401,
            &json!({"type": "error", "error": {"type": "authentication_error", "message": "bad key"}}),
        ),
        Answer::events(&fixture("anthropic-overloaded.sse")),
        Answer::events(&fixture("anthropic-cut-off.sse")),
    ])
    .unwrap();
    let provider = provider(&stub, ProviderKind::AnthropicCompatible, Some("k"));
    let question = [Message::user("hi")];
    let failure = |n: usize| {
        let error = ask(&provider, "claude-opus-5-5", &question).0.unwrap_err();
        eprintln!("{n}: {error}");
        error
    };
    let limited = failure(1);
    assert_eq!(
        (
            limited.kind,
            limited.status,
            limited.retry_after,
            limited.message.as_str()
        ),
        (
            ErrorKind::RateLimit,
            Some(429),
            Some(Duration::from_secs(3)),
            "slow down"
        )
    );
    assert!(limited.is_retryable());
    let overloaded = failure(2);
    assert_eq!(
        (overloaded.kind, overloaded.is_retryable()),
        (ErrorKind::Overloaded, true)
    );
    let auth = failure(3);
    assert_eq!(
        (auth.kind, auth.is_retryable(), auth.message.as_str()),
        (ErrorKind::Auth, false, "bad key")
    );
    let midstream = failure(4);
    assert_eq!(
        (midstream.kind, midstream.message.as_str()),
        (ErrorKind::Overloaded, "Overloaded")
    );
    let cut = failure(5);
    assert_eq!((cut.kind, cut.message.as_str()), (ErrorKind::Incomplete, ""));
}

#[test]
fn a_fallback_keeps_only_the_text_before_it_and_a_refusal_runs_no_tools() {
    let stub = Stub::script(vec![
        Answer::events(&fixture("anthropic-fallback.sse")),
        Answer::events(&fixture("anthropic-refusal.sse")),
    ])
    .unwrap();
    let provider = provider(&stub, ProviderKind::AnthropicCompatible, Some("k"));
    let question = [Message::user("hi")];
    let (reply, deltas) = ask(&provider, "claude-opus-5-5", &question);
    let reply = reply.unwrap();
    assert!(deltas.contains(&Delta::Fallback {
        from: "claude-opus-5-5".into(),
        to: "claude-opus-4-8".into()
    }));
    assert_eq!(reply.message.content, [Block::text("Partial "), Block::text("answer.")]);
    let (refused, _) = ask(&provider, "claude-opus-5-5", &question);
    let refused = refused.unwrap();
    assert_eq!(refused.stop, Stop::Refusal(Some("cyber".into())));
    assert_eq!(refused.message.content, [Block::text("I will ")]);
}

#[test]
fn cancelling_ends_the_stream_with_the_text_so_far() {
    let stub = Stub::script(vec![Answer::events_slowly(
        &fixture("anthropic-tool-use.sse"),
        Duration::from_millis(30),
    )])
    .unwrap();
    let provider = provider(&stub, ProviderKind::AnthropicCompatible, Some("k"));
    let messages = [Message::user("hi")];
    let tools = tools();
    let request = Request {
        model: "claude-opus-5-5",
        system: "",
        messages: &messages,
        tools: &tools,
        max_tokens: None,
        effort: None,
    };
    let cancel = Cancel::new();
    let reply = provider
        .stream(&request, &cancel, &mut |delta| {
            if delta == Delta::Text("it.".into()) {
                cancel.cancel();
            }
        })
        .unwrap();
    assert_eq!(reply.stop, Stop::Cancelled);
    assert_eq!(reply.message.content, [Block::text("Let me read it.")]);
}

#[test]
fn openai_gathers_parallel_tool_calls_and_cached_tokens() {
    let stub = Stub::script(vec![
        Answer::events(&fixture("openai-tool-calls.sse")),
        Answer::events(&fixture("openai-text.sse")),
    ])
    .unwrap();
    let provider = provider(&stub, ProviderKind::OpenAiCompatible, Some("sk-o"));
    let mut conversation = vec![Message::user("Where is main?")];
    let (reply, deltas) = ask(&provider, "gpt-x", &conversation);
    let reply = reply.unwrap();
    assert_eq!(reply.stop, Stop::ToolUse);
    let calls: Vec<(&str, &str, &Value)> = reply.message.tool_uses().collect();
    assert_eq!(
        calls,
        [
            ("call_a", "grep", &json!({"pattern": "fn main"})),
            ("call_b", "glob", &json!({"pattern": "*.rs"}))
        ]
    );
    assert_eq!(
        (reply.usage.input, reply.usage.cache_read, reply.usage.output),
        (200, 800, 40)
    );
    assert_eq!(
        deltas.iter().filter(|d| matches!(d, Delta::ToolStarted { .. })).count(),
        2
    );

    conversation.push(reply.message.clone());
    conversation.push(results(&reply, "src/main.rs:1"));
    let (second, _) = ask(&provider, "gpt-x", &conversation);
    assert_eq!(second.unwrap().message.text(), "Two files match.");
    let seen = stub.seen();
    assert_eq!(seen[0].path, "/v1/chat/completions");
    assert_eq!(seen[0].header("authorization"), Some("Bearer sk-o"));
    assert_eq!(seen[0].json()["stream_options"]["include_usage"], true);
    let roles: Vec<String> = seen[1].json()["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["role"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "tool", "tool"]);
}

#[test]
fn openrouter_shows_reasoning_and_its_reported_cost_and_fails_on_an_error_chunk() {
    let stub = Stub::script(vec![
        Answer::events(&fixture("openrouter-reasoning.sse")),
        Answer::events(&fixture("openrouter-error.sse")),
    ])
    .unwrap();
    let provider = provider(&stub, ProviderKind::OpenRouter, Some("sk-or"));
    let question = [Message::user("hi")];
    let (reply, deltas) = ask(&provider, "anthropic/claude-x", &question);
    let reply = reply.unwrap();
    assert!(deltas.contains(&Delta::Thinking("Thinking about it.".into())));
    assert_eq!(
        (reply.stop.clone(), reply.message.text()),
        (Stop::EndTurn, "Hello there".to_string())
    );
    assert_eq!(reply.usage.cost_usd, Some(0.00014));
    let error = ask(&provider, "anthropic/claude-x", &question).0.unwrap_err();
    assert_eq!(
        (error.kind, error.message.as_str()),
        (ErrorKind::Server, "Provider disconnected")
    );
}

#[test]
fn ollama_needs_no_key_and_a_call_with_stop_still_waits_for_its_result() {
    let stub = Stub::script(vec![Answer::events(&fixture("ollama-tool-call.sse"))]).unwrap();
    let provider = provider(&stub, ProviderKind::Ollama, None);
    let (reply, _) = ask(&provider, "qwen3:8b", &[Message::user("list docs")]);
    let reply = reply.unwrap();
    assert_eq!(reply.stop, Stop::ToolUse);
    assert!(
        matches!(&reply.message.content[0], Block::Thinking { text, wire: Wire::OpenAiChat, .. } if text == "Need the file list.")
    );
    assert_eq!(reply.message.tool_uses().count(), 1);
    assert_eq!(stub.seen()[0].header("authorization"), None);
}

#[test]
fn gemini_returns_signatures_and_ids_with_the_call_and_its_answer() {
    let stub = Stub::script(vec![
        Answer::events(&fixture("gemini-function-call.sse")),
        Answer::events(&fixture("gemini-text.sse")),
        Answer::events(&fixture("gemini-blocked.sse")),
    ])
    .unwrap();
    let provider = provider(&stub, ProviderKind::Gemini, Some("g-key"));
    let mut conversation = vec![Message::user("Any TODOs?")];
    let (reply, _) = ask(&provider, "gemini-3.5-flash", &conversation);
    let reply = reply.unwrap();
    assert_eq!(reply.stop, Stop::ToolUse);
    assert_eq!(
        (reply.usage.input, reply.usage.cache_read, reply.usage.output),
        (400, 100, 50)
    );
    let [
        Block::Thinking { wire: Wire::Gemini, .. },
        Block::Text { text, .. },
        Block::ToolUse { id, signature, .. },
    ] = reply.message.content.as_slice()
    else {
        panic!("{:?}", reply.message.content)
    };
    assert_eq!(
        (text.as_str(), id.as_str(), signature.as_deref()),
        ("Searching.", "fc_1", Some("c2lnLTE="))
    );

    conversation.push(reply.message.clone());
    conversation.push(results(&reply, "a.rs:3 TODO"));
    let (second, _) = ask(&provider, "gemini-3.5-flash", &conversation);
    assert_eq!(second.unwrap().message.text(), "Found 2 TODOs.");
    let seen = stub.seen();
    assert_eq!(
        seen[0].path,
        "/v1/models/gemini-3.5-flash:streamGenerateContent?alt=sse"
    );
    assert_eq!(seen[0].header("x-goog-api-key"), Some("g-key"));
    let sent = seen[1].json();
    let call = &sent["contents"][1]["parts"];
    assert_eq!(call[0]["text"], "Searching.");
    assert_eq!(call[1]["thoughtSignature"], "c2lnLTE=");
    let answer = &sent["contents"][2]["parts"][0]["functionResponse"];
    assert_eq!(
        (answer["id"].clone(), answer["name"].clone()),
        (json!("fc_1"), json!("grep"))
    );

    let (blocked, _) = ask(&provider, "gemini-3.5-flash", &[Message::user("x")]);
    assert_eq!(blocked.unwrap().stop, Stop::Refusal(Some("safety".into())));
}

#[test]
fn model_lists_page_through_and_carry_limits_and_prices() {
    let stub = Stub::start(|seen| {
        let page = |body: Value| Answer::json(200, &body);
        match seen.path.as_str() {
            "/v1/models?limit=1000" => page(json!({
                "data": [{"id": "claude-opus-5-5", "display_name": "Claude Opus 5.5", "max_input_tokens": 1_000_000, "max_tokens": 128_000}],
                "has_more": true, "last_id": "claude-opus-5-5"})),
            "/v1/models?limit=1000&after_id=claude-opus-5-5" => page(json!({
                "data": [{"id": "claude-haiku-4-5-20251001", "display_name": "Claude Haiku 4.5"}],
                "has_more": false, "last_id": "claude-haiku-4-5-20251001"})),
            "/v1/models" => page(json!({"data": [
                {"id": "router/tooled", "name": "Tooled", "context_length": 200_000,
                 "pricing": {"prompt": "0.000003", "completion": "0.000015"}, "supported_parameters": ["tools"]},
                {"id": "router/plain", "supported_parameters": ["temperature"]},
                {"id": "text-embedding-x"}]})),
            "/v1/models?pageSize=1000" => page(json!({"models": [
                {"name": "models/gemini-3.5-flash", "displayName": "Gemini 3.5 Flash", "inputTokenLimit": 1_048_576,
                 "outputTokenLimit": 65_536, "supportedGenerationMethods": ["generateContent", "countTokens"]},
                {"name": "models/embedding-x", "supportedGenerationMethods": ["embedContent"]}]})),
            _ => Answer::json(404, &json!({"error": {"message": "no"}})),
        }
    })
    .unwrap();
    let anthropic = provider(&stub, ProviderKind::AnthropicCompatible, Some("k"))
        .models()
        .unwrap();
    assert_eq!(anthropic.len(), 2);
    assert_eq!(
        (anthropic[0].context, anthropic[0].max_output),
        (Some(1_000_000), Some(128_000))
    );
    assert_eq!(
        anthropic[1].price, None,
        "list prices are Anthropic's own, not a compatible server's"
    );
    let routed = provider(&stub, ProviderKind::OpenRouter, Some("k")).models().unwrap();
    let ids: Vec<&str> = routed.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["router/tooled"]);
    assert!((routed[0].price.unwrap().output - 15.0).abs() < 1e-9);
    let openai = provider(&stub, ProviderKind::OpenAiCompatible, Some("k"))
        .models()
        .unwrap();
    assert!(openai.iter().all(|m| m.id != "text-embedding-x"));
    let gemini = provider(&stub, ProviderKind::Gemini, Some("k")).models().unwrap();
    assert_eq!(gemini.len(), 1);
    assert_eq!(
        (gemini[0].id.as_str(), gemini[0].context),
        ("gemini-3.5-flash", Some(1_048_576))
    );
}

#[test]
fn a_service_without_its_key_is_not_asked_at_all() {
    let provider = Provider::new(Endpoint::preset(ProviderKind::Anthropic).unwrap(), None, Http::new());
    let error = provider.models().unwrap_err();
    assert_eq!((error.kind, error.message.as_str()), (ErrorKind::NoKey, "Anthropic"));
}
