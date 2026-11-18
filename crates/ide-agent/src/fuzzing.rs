//! Entry points for the fuzz targets in `fuzz/`.

use serde_json::Value;

use crate::chat::Delta;
use crate::conversation::Block;
use crate::sse::{Event, Events};

/// Reads `bytes` as a server-sent event stream and hands its events to the stream readers of all three providers.
/// The events written out again read back as themselves, and what OpenAI's and Gemini's readers tell while they read
/// is what ends up in their message.
pub fn stream(bytes: &[u8]) {
    let events: Vec<Event> = Events::new(bytes).map_while(Result::ok).collect();
    let written = written(&events);
    let again: Vec<Event> = Events::new(written.as_slice()).map_while(Result::ok).collect();
    assert_eq!(again, events);
    let chunks: Vec<Value> = events
        .iter()
        .filter_map(|event| crate::http::parse(&event.data).ok())
        .collect();
    crate::anthropic::read_all(&chunks);
    let (told, message) = crate::openai::read_all(&chunks);
    told_is_kept(&told, &message.content);
    let (told, blocks) = crate::gemini::read_all(&chunks);
    told_is_kept(&told, &blocks);
    let started: Vec<(&str, &str)> = told
        .iter()
        .filter_map(|delta| match delta {
            Delta::ToolStarted { id, name } => Some((id.as_str(), name.as_str())),
            _ => None,
        })
        .collect();
    let calls: Vec<(&str, &str)> = blocks
        .iter()
        .filter_map(|block| match block {
            Block::ToolUse { id, name, .. } => Some((id.as_str(), name.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(started, calls);
}

/// `events` as a server writes them, one `data:` line for each line of their data.
fn written(events: &[Event]) -> Vec<u8> {
    let mut out = String::new();
    for event in events {
        if let Some(name) = &event.event {
            out.push_str(&format!("event: {name}\n"));
        }
        for line in event.data.split('\n') {
            out.push_str(&format!("data: {line}\n"));
        }
        out.push('\n');
    }
    out.into_bytes()
}

/// Checks that the text and reasoning told along the way are the text and reasoning of `content`.
fn told_is_kept(told: &[Delta], content: &[Block]) {
    let (mut text, mut thinking) = (String::new(), String::new());
    for delta in told {
        match delta {
            Delta::Text(part) => text.push_str(part),
            Delta::Thinking(part) => thinking.push_str(part),
            _ => {}
        }
    }
    let (mut kept_text, mut kept_thinking) = (String::new(), String::new());
    for block in content {
        match block {
            Block::Text { text, .. } => kept_text.push_str(text),
            Block::Thinking { text, .. } => kept_thinking.push_str(text),
            _ => {}
        }
    }
    assert_eq!(text, kept_text);
    assert_eq!(thinking, kept_thinking);
}
