//! A stand-in for copilot-language-server in the tests: it keeps the documents it is told about, signs in through a
//! pretend device flow and proposes completions and next edits from the text it holds.
//!
//! Environment: `FAKE_SIGNED_IN=1` starts signed in, `FAKE_NO_SUBSCRIPTION=1` reports an account without Copilot,
//! `FAKE_LOG` names a file it appends every method it receives to. A document containing `SLOW` is never answered.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};

use serde_json::{Value, json};

fn main() {
    let mut fake = Fake {
        signed_in: std::env::var_os("FAKE_SIGNED_IN").is_some(),
        no_subscription: std::env::var_os("FAKE_NO_SUBSCRIPTION").is_some(),
        docs: HashMap::new(),
        log: std::env::var("FAKE_LOG").ok(),
    };
    let mut input = BufReader::new(io::stdin());
    while let Some(message) = read(&mut input) {
        if fake.handle(&message) {
            return;
        }
    }
}

fn read(input: &mut impl BufRead) -> Option<Value> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse().ok()?;
        }
    }
    let mut body = vec![0; length];
    input.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn send(message: &Value) {
    let body = serde_json::to_vec(message).unwrap_or_default();
    let mut out = io::stdout().lock();
    let _ = write!(out, "Content-Length: {}\r\n\r\n", body.len());
    let _ = out.write_all(&body);
    let _ = out.flush();
}

struct Fake {
    signed_in: bool,
    no_subscription: bool,
    /// Text and version by URI.
    docs: HashMap<String, (String, i64)>,
    log: Option<String>,
}

impl Fake {
    /// Handles one message; true when the client said `exit`.
    fn handle(&mut self, message: &Value) -> bool {
        let method = message["method"].as_str().unwrap_or_default();
        let params = &message["params"];
        self.note(method, params);
        let answer = match method {
            "exit" => return true,
            "initialize" => json!({"capabilities": {
                "positionEncoding": "utf-16",
                "textDocumentSync": {"openClose": true, "change": 2},
                "executeCommandProvider": {"commands": ["github.copilot.finishDeviceFlow"]},
                "inlineCompletionProvider": {},
            }}),
            "shutdown" => Value::Null,
            "textDocument/didOpen" => {
                let doc = &params["textDocument"];
                let entry = (
                    doc["text"].as_str().unwrap_or_default().to_string(),
                    doc["version"].as_i64().unwrap_or(0),
                );
                self.docs
                    .insert(doc["uri"].as_str().unwrap_or_default().to_string(), entry);
                return false;
            }
            "textDocument/didChange" => {
                self.change(params);
                return false;
            }
            "textDocument/didClose" => {
                self.docs
                    .remove(params["textDocument"]["uri"].as_str().unwrap_or_default());
                return false;
            }
            "checkStatus" => self.status(),
            "signIn" if self.signed_in => json!({"status": "AlreadySignedIn", "user": "octocat"}),
            "signIn" => json!({
                "status": "PromptUserDeviceFlow",
                "userCode": "ABCD-EFGH",
                "verificationUri": "https://github.com/login/device",
                "expiresIn": 900,
                "interval": 5,
                "command": {"command": "github.copilot.finishDeviceFlow", "arguments": [], "title": "Sign in"},
            }),
            "signOut" => {
                self.signed_in = false;
                json!({"status": "NotSignedIn"})
            }
            "workspace/executeCommand" if params["command"] == "github.copilot.finishDeviceFlow" => {
                self.signed_in = true;
                self.status()
            }
            "workspace/executeCommand" => Value::Null,
            "textDocument/inlineCompletion" | "textDocument/copilotInlineEdit" if !self.signed_in => {
                send(
                    &json!({"jsonrpc": "2.0", "id": message["id"], "error": {"code": 1000, "message": "Not signed in"}}),
                );
                return false;
            }
            "textDocument/inlineCompletion" => match self.completion(params) {
                Some(answer) => answer,
                None => return false,
            },
            "textDocument/copilotInlineEdit" => match self.edit(params) {
                Some(answer) => answer,
                None => return false,
            },
            _ if message.get("id").is_none() => return false,
            _ => {
                send(&json!({"jsonrpc": "2.0", "id": message["id"], "error": {"code": -32601, "message": "unknown"}}));
                return false;
            }
        };
        if message.get("id").is_some() {
            send(&json!({"jsonrpc": "2.0", "id": message["id"], "result": answer}));
        }
        false
    }

    fn note(&self, method: &str, params: &Value) {
        let Some(log) = &self.log else { return };
        let line = match method {
            "$/cancelRequest" => format!("{method} {}\n", params["id"]),
            "workspace/executeCommand" => format!("{method} {} {}\n", params["command"], params["arguments"]),
            _ => format!("{method}\n"),
        };
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn status(&self) -> Value {
        match (self.signed_in, self.no_subscription) {
            (false, _) => json!({"status": "NotSignedIn"}),
            (true, true) => json!({"status": "NotAuthorized", "user": "octocat"}),
            (true, false) => json!({"status": "OK", "user": "octocat"}),
        }
    }

    fn change(&mut self, params: &Value) {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let Some((text, version)) = self.docs.get_mut(uri) else {
            return;
        };
        *version = params["textDocument"]["version"].as_i64().unwrap_or(0);
        for change in params["contentChanges"].as_array().into_iter().flatten() {
            let new = change["text"].as_str().unwrap_or_default();
            if change.get("range").is_none() {
                *text = new.to_string();
                continue;
            }
            let start = offset(text, &change["range"]["start"]);
            let end = offset(text, &change["range"]["end"]);
            text.replace_range(start..end, new);
        }
    }

    /// The caret's line so far, with `completion()` after it.
    fn completion(&self, params: &Value) -> Option<Value> {
        let (text, _) = self.docs.get(params["textDocument"]["uri"].as_str()?)?;
        if text.contains("SLOW") {
            return None;
        }
        let line = params["position"]["line"].as_u64()?;
        let caret = offset(text, &params["position"]);
        let start = text[..caret].rfind('\n').map_or(0, |i| i + 1);
        Some(json!({"items": [{
            "insertText": format!("{}completion()", &text[start..caret]),
            "range": {"start": {"line": line, "character": 0}, "end": params["position"]},
            "command": {"command": "github.copilot.didAcceptCompletionItem", "arguments": ["c1"]},
        }]}))
    }

    /// `old_name` renamed to `new_name` on the first line holding it.
    fn edit(&self, params: &Value) -> Option<Value> {
        let uri = params["textDocument"]["uri"].as_str()?;
        let (text, version) = self.docs.get(uri)?;
        if text.contains("SLOW") {
            return None;
        }
        let Some((line, column)) = text
            .split('\n')
            .enumerate()
            .find_map(|(n, l)| l.find("old_name").map(|c| (n, l[..c].encode_utf16().count())))
        else {
            return Some(json!({"edits": []}));
        };
        Some(json!({"edits": [{
            "text": "new_name",
            "textDocument": {"uri": uri, "version": version},
            "range": {"start": {"line": line, "character": column}, "end": {"line": line, "character": column + 8}},
            "command": {"command": "github.copilot.didAcceptCompletionItem", "arguments": ["e1"]},
        }]}))
    }
}

/// The byte offset of an LSP position counted in UTF-16.
fn offset(text: &str, position: &Value) -> usize {
    let number = |key: &str| usize::try_from(position[key].as_u64().unwrap_or(0)).unwrap_or(usize::MAX);
    let (line, character) = (number("line"), number("character"));
    let start: usize = text.split('\n').take(line).map(|l| l.len() + 1).sum();
    let start = start.min(text.len());
    let rest = text[start..].split('\n').next().unwrap_or_default();
    let mut units = 0;
    for (i, c) in rest.char_indices() {
        if units >= character {
            return start + i;
        }
        units += c.len_utf16();
    }
    start + rest.len()
}
