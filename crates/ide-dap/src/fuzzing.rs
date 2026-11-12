//! Entry points for the fuzz targets in `fuzz/`.

use serde_json::Value;

use crate::protocol::{self, Message};
use crate::types::Capabilities;
use crate::{convert, launch};

/// Reads `bytes` as what an adapter writes: framed messages, each parsed and its body turned into the crate's types
/// as a session does with responses and events.
pub fn messages(bytes: &[u8]) {
    let mut reader = bytes;
    while let Ok(Some(body)) = ide_process::framing::read(&mut reader) {
        let Ok(message) = protocol::parse(&body) else {
            continue;
        };
        let body = match &message {
            Message::Response { body, .. } | Message::Event { body, .. } => body.clone(),
            Message::Request { arguments, .. } => arguments.clone(),
        };
        let _ = protocol::parse(&serde_json::to_vec(&message.to_json()).unwrap_or_default());
        convert_all(&body);
    }
}

/// Reads `bytes` as a `launch.json`, with comments, variables and all.
pub fn launch_json(bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    let variables = launch::Variables {
        workspace_folder: "/work".into(),
        file: Some("/work/src/main.rs".into()),
        line: Some(1),
        selected_text: Some("x".into()),
        env: vec![("HOME".into(), "/home".into())],
    };
    let _ = launch::parse_launch_json(&text, &variables);
}

fn convert_all(body: &Value) {
    let _ = Capabilities::from_json(body);
    if let Some(capabilities) = body.get("capabilities") {
        let _ = Capabilities::from_json(capabilities);
    }
    let _ = convert::threads(body);
    let _ = convert::stack_frames(body);
    let _ = convert::scopes(body);
    let _ = convert::variables(body);
    let _ = convert::evaluation(body);
    let _ = convert::breakpoints(body);
    if let Some(breakpoint) = body.get("breakpoint") {
        let _ = convert::breakpoint(breakpoint);
    }
    let _ = convert::stop(body);
    let _ = convert::output(body);
    let _ = convert::program(body);
}
