//! Entry points for the fuzz targets in `fuzz/`.

use serde_json::{Map, Value, json};

use crate::protocol::{self, Message};
use crate::types::Capabilities;
use crate::{convert, jsonc, launch};

/// Reads `bytes` as what an adapter writes: framed messages, each parsed and its body turned into the crate's types
/// as a session does with responses and events. Every message the crate writes again reads back as itself.
pub fn messages(bytes: &[u8]) {
    let mut reader = bytes;
    while let Ok(Some(body)) = ide_process::framing::read(&mut reader) {
        let Ok(message) = protocol::parse(&body) else {
            continue;
        };
        let frame = message.to_frame();
        let again = ide_process::framing::read(&mut frame.as_slice())
            .ok()
            .flatten()
            .map(|body| protocol::parse(&body));
        assert!(
            matches!(&again, Some(Ok(read)) if *read == message),
            "{message:?} reads back as {again:?}"
        );
        let body = match &message {
            Message::Response { body, .. } | Message::Event { body, .. } => body.clone(),
            Message::Request { arguments, .. } => arguments.clone(),
        };
        convert_all(&body);
    }
}

/// Reads `bytes` as a `launch.json`, with comments, variables and all. Taking out comments and trailing commas keeps
/// every byte's place and leaves plain JSON alone, and a configuration written out again reads back as itself.
pub fn launch_json(bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    let plain = jsonc::strip(&text);
    assert_eq!(plain.len(), text.len());
    assert!(
        plain
            .bytes()
            .zip(text.bytes())
            .all(|(a, b)| (a == b'\n') == (b == b'\n')),
        "line breaks moved"
    );
    if serde_json::from_str::<Value>(&text).is_ok() {
        assert_eq!(plain, text, "plain JSON changed");
    }
    let variables = launch::Variables {
        workspace_folder: "/work".into(),
        file: Some("/work/src/main.rs".into()),
        line: Some(1),
        selected_text: Some("x".into()),
        env: vec![("HOME".into(), "/home".into())],
    };
    let Ok(entries) = launch::parse_launch_json(&text, &variables) else {
        return;
    };
    let written: Vec<Value> = entries
        .iter()
        .map(|entry| match &entry.config {
            Ok(config) if settled(config) => Value::Object(config.arguments.clone()),
            _ => json!({}),
        })
        .collect();
    let file = json!({"version": "0.2.0", "configurations": written}).to_string();
    let again = launch::parse_launch_json(&file, &variables).expect("written launch.json reads");
    assert_eq!(again.len(), entries.len());
    for (n, (entry, read)) in entries.iter().zip(&again).enumerate() {
        let config = match &entry.config {
            Ok(config) if settled(config) => config,
            _ => continue,
        };
        let name = config
            .arguments
            .get("name")
            .and_then(Value::as_str)
            .map_or_else(|| format!("#{}", n + 1), str::to_string);
        let expected = launch::LaunchConfig {
            name: name.clone(),
            ..config.clone()
        };
        assert_eq!(read.name, name);
        assert_eq!(read.config, Ok(expected));
    }
}

/// Whether reading `config` again leaves it as it is: nothing left to fill in, to pick, or to take from the settings
/// for one system.
fn settled(config: &launch::LaunchConfig) -> bool {
    !config.process_pick
        && !["osx", "windows", "linux"]
            .iter()
            .any(|key| config.arguments.contains_key(*key))
        && !has_variable(&config.arguments)
}

fn has_variable(object: &Map<String, Value>) -> bool {
    fn any(value: &Value) -> bool {
        match value {
            Value::String(text) => text.contains("${"),
            Value::Array(items) => items.iter().any(any),
            Value::Object(fields) => fields.values().any(any),
            _ => false,
        }
    }
    object.values().any(any)
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
