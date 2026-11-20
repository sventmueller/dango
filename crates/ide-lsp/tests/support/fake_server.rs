//! A language server for the tests: it keeps the documents it is told about, answers the editor's requests from
//! them in the position encoding it picked, and obeys `fake/…` requests that make it crash, stall or ask the client.
//!
//! Environment: `FAKE_ENCODING` is the encoding it prefers when offered, `FAKE_LOG` a file it appends every
//! method it receives to, `FAKE_EXIT_AT_START` makes it exit at once with code 7, and `FAKE_SYNC` is the JSON of the
//! `textDocumentSync` capability it announces in place of incremental changes with saved texts. `FAKE_SHAPES` makes
//! it also format and give inlay hints, and answer completion and document symbols in their other, plain shapes.
//! `FAKE_HOVER_MS` holds each hover's answer back for that many milliseconds.

use ide_core::LockIgnoringPoison;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

type Out = Arc<Mutex<io::Stdout>>;

fn main() {
    if std::env::var_os("FAKE_EXIT_AT_START").is_some() {
        std::process::exit(7);
    }
    let out: Out = Arc::new(Mutex::new(io::stdout()));
    let (tx, rx) = channel();
    std::thread::spawn(move || read_all(&tx));
    let mut server = Fake {
        out,
        encoding: "utf-16".into(),
        docs: HashMap::new(),
        received: vec![],
        shutdown: false,
        cancelled: Arc::new(Mutex::new(HashSet::new())),
        asked: HashMap::new(),
        next: 0,
        tokens: 0,
        sent_tokens: HashMap::new(),
        changes: Value::Null,
        saved: vec![],
    };
    while let Ok(message) = rx.recv() {
        server.handle(&message);
    }
    std::process::exit(if server.shutdown { 0 } else { 1 });
}

fn read_all(tx: &Sender<Value>) {
    let mut input = BufReader::new(io::stdin());
    loop {
        let mut length = 0;
        loop {
            let mut line = String::new();
            if input.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; length];
        if input.read_exact(&mut body).is_err() {
            return;
        }
        if let Ok(message) = serde_json::from_slice(&body)
            && tx.send(message).is_err()
        {
            return;
        }
    }
}

fn send(out: &Out, message: &Value) {
    let body = serde_json::to_vec(message).unwrap_or_default();
    let mut out = out.lock_ignoring_poison();
    let _ = write!(out, "Content-Length: {}\r\n\r\n", body.len());
    let _ = out.write_all(&body);
    let _ = out.flush();
}

struct Fake {
    out: Out,
    encoding: String,
    docs: HashMap<String, (i64, String)>,
    received: Vec<String>,
    shutdown: bool,
    cancelled: Arc<Mutex<HashSet<String>>>,
    /// Requests sent to the client for `fake/ask`, by their id, with the id of the `fake/ask` to answer.
    asked: HashMap<String, Value>,
    next: u64,
    tokens: u64,
    /// How many numbers the last semantic tokens answer for each document had.
    sent_tokens: HashMap<String, usize>,
    /// The content changes of the last `didChange`.
    changes: Value,
    /// The text of each `didSave`, null where it came without.
    saved: Vec<Value>,
}

impl Fake {
    fn handle(&mut self, message: &Value) {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match (method, id) {
            (Some(method), Some(id)) => {
                self.note(method);
                self.request(method, id, &params);
            }
            (Some(method), None) => {
                self.note(method);
                self.notification(method, &params);
            }
            (None, Some(id)) => {
                if let Some(original) = self.asked.remove(&id.to_string()) {
                    let answer = message.get("result").cloned().unwrap_or(Value::Null);
                    let error = message.get("error").cloned();
                    self.reply(&original, json!({"answer": answer, "error": error}));
                }
            }
            (None, None) => {}
        }
    }

    fn note(&mut self, method: &str) {
        self.received.push(method.to_string());
        if let Some(path) = std::env::var_os("FAKE_LOG")
            && let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path)
        {
            let _ = writeln!(file, "{} {method}", std::process::id());
        }
    }

    fn reply(&self, id: &Value, result: Value) {
        send(&self.out, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }

    fn fail(&self, id: &Value, code: i64, message: &str) {
        send(
            &self.out,
            &json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        );
    }

    fn notify(&self, method: &str, params: Value) {
        send(
            &self.out,
            &json!({"jsonrpc": "2.0", "method": method, "params": params}),
        );
    }

    fn notification(&mut self, method: &str, params: &Value) {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
        match method {
            "textDocument/didOpen" => {
                let doc = &params["textDocument"];
                let text = doc["text"].as_str().unwrap_or_default().to_string();
                self.docs
                    .insert(uri.clone(), (doc["version"].as_i64().unwrap_or(0), text));
                self.publish(&uri);
            }
            "textDocument/didChange" => {
                let version = params["textDocument"]["version"].as_i64().unwrap_or(0);
                self.changes = params["contentChanges"].clone();
                let changes = params["contentChanges"].as_array().cloned().unwrap_or_default();
                let encoding = self.encoding.clone();
                if let Some((v, text)) = self.docs.get_mut(&uri) {
                    *v = version;
                    for change in changes {
                        let new = change["text"].as_str().unwrap_or_default();
                        match change.get("range") {
                            Some(range) => {
                                let start = offset(text, &range["start"], &encoding);
                                let end = offset(text, &range["end"], &encoding);
                                text.replace_range(start..end, new);
                            }
                            None => *text = new.to_string(),
                        }
                    }
                }
                self.publish(&uri);
            }
            "textDocument/didClose" => {
                self.docs.remove(&uri);
            }
            "textDocument/didSave" => self.saved.push(params.get("text").cloned().unwrap_or(Value::Null)),
            "$/cancelRequest" => {
                self.cancelled.lock_ignoring_poison().insert(params["id"].to_string());
            }
            "exit" => std::process::exit(if self.shutdown { 0 } else { 1 }),
            _ => {}
        }
    }

    fn request(&mut self, method: &str, id: Value, params: &Value) {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
        if let Some(result) = self.other_shape(method, &uri, params) {
            return self.reply(&id, result);
        }
        let result = match method {
            "initialize" => self.initialize(params),
            "shutdown" => {
                self.shutdown = true;
                Value::Null
            }
            "textDocument/hover" => {
                let hover = self.hover(&uri, &params["position"]);
                return self.later(id, hover);
            }
            "textDocument/completion" => self.completion(&uri, &params["position"]),
            "completionItem/resolve" => {
                let mut item = params.clone();
                item["documentation"] = json!({"kind": "markdown", "value": "resolved"});
                item
            }
            "textDocument/definition" => self
                .occurrences(&uri, &params["position"])
                .first()
                .cloned()
                .unwrap_or(Value::Null),
            "textDocument/references" => Value::Array(self.occurrences(&uri, &params["position"])),
            "textDocument/implementation" => Value::Array(self.occurrences(&uri, &params["position"])),
            "textDocument/typeDefinition" | "textDocument/declaration" => self.link(&uri, &params["position"], method),
            "workspace/symbol" => self.project_symbols(params["query"].as_str().unwrap_or_default()),
            "workspaceSymbol/resolve" => {
                let mut symbol = params.clone();
                symbol["location"]["range"] = symbol["data"]["range"].clone();
                symbol
            }
            "textDocument/prepareCallHierarchy" | "textDocument/prepareTypeHierarchy" => self
                .hierarchy_item(&uri, &params["position"])
                .map_or(Value::Null, |item| json!([item])),
            "callHierarchy/incomingCalls" | "callHierarchy/outgoingCalls" => self.calls(&params["item"], method),
            "typeHierarchy/supertypes" | "typeHierarchy/subtypes" => {
                let mut item = params["item"].clone();
                let prefix = if method.ends_with("supertypes") { "super" } else { "sub" };
                item["name"] = json!(format!("{prefix} of {}", item["name"].as_str().unwrap_or_default()));
                json!([item])
            }
            "textDocument/prepareRename" => self.word_range(&uri, &params["position"]).unwrap_or(Value::Null),
            "textDocument/rename" => self.rename(&uri, &params["position"], &params["newName"]),
            "textDocument/semanticTokens/full" | "textDocument/semanticTokens/full/delta" => self.tokens(&uri, method),
            "textDocument/diagnostic" => self.pulled(&uri, params),
            "textDocument/documentSymbol" => json!([{
                "name": "outer", "kind": 12,
                "range": self.range(&uri, 0, 0, 0, 1), "selectionRange": self.range(&uri, 0, 0, 0, 1),
                "children": [{"name": "inner", "kind": 13, "range": self.range(&uri, 0, 0, 0, 1), "selectionRange": self.range(&uri, 0, 0, 0, 1)}],
            }]),
            "textDocument/foldingRange" => json!([{"startLine": 0, "endLine": 1}]),
            "textDocument/signatureHelp" => json!({
                "signatures": [{"label": "f(a, b)", "parameters": [{"label": [2, 3]}, {"label": [5, 6]}]}],
                "activeSignature": 0,
                "activeParameter": 1,
            }),
            "textDocument/codeAction" => json!([{
                "title": "fix",
                "kind": "quickfix",
                "diagnostics": params["context"]["diagnostics"],
                "data": {"uri": uri},
            }]),
            "codeAction/resolve" => {
                let mut action = params.clone();
                let target = action["data"]["uri"].clone();
                let range = action["diagnostics"][0]["range"].clone();
                action["edit"] =
                    json!({"changes": {target.as_str().unwrap_or_default(): [{"range": range, "newText": "fixed"}]}});
                action
            }
            "workspace/executeCommand" => {
                let edit = params["arguments"][0].clone();
                send(
                    &self.out,
                    &json!({"jsonrpc": "2.0", "id": "edit", "method": "workspace/applyEdit", "params": {"edit": edit}}),
                );
                Value::Null
            }
            "fake/crash" => std::process::exit(3),
            "fake/notify" => {
                self.notify(params["method"].as_str().unwrap_or_default(), params["params"].clone());
                Value::Null
            }
            "fake/ask" => {
                self.next += 1;
                let asked = format!("s-{}", self.next);
                self.asked.insert(json!(asked).to_string(), id);
                let message =
                    json!({"jsonrpc": "2.0", "id": asked, "method": params["method"], "params": params["params"]});
                send(&self.out, &message);
                return;
            }
            "fake/sleep" => return self.sleep(id, params["ms"].as_u64().unwrap_or(0)),
            _ => return self.fail(&id, -32601, "unknown method"),
        };
        self.reply(&id, result);
    }

    /// The answers of `FAKE_SHAPES`, formatting and inlay hints, which only it announces, and what the tests ask
    /// about what the server heard.
    fn other_shape(&self, method: &str, uri: &str, params: &Value) -> Option<Value> {
        let shapes = std::env::var_os("FAKE_SHAPES").is_some();
        Some(match method {
            "textDocument/completion" if shapes => json!([{"label": "plain"}]),
            "textDocument/documentSymbol" if shapes => json!([{
                "name": "flat", "kind": 12, "containerName": "outer",
                "location": {"uri": uri, "range": self.range(uri, 0, 0, 0, 1)},
            }]),
            "textDocument/formatting" => json!([{"range": self.range(uri, 0, 0, 0, 0), "newText": "// formatted\n"}]),
            "textDocument/rangeFormatting" => json!([{"range": params["range"], "newText": "x"}]),
            "textDocument/inlayHint" => json!([{"position": params["range"]["start"], "label": ": i32"}]),
            "fake/text" => self
                .docs
                .get(uri)
                .map_or(Value::Null, |(version, text)| json!({"version": version, "text": text})),
            "fake/received" => json!(self.received),
            "fake/changes" => self.changes.clone(),
            "fake/saved" => json!(self.saved),
            "fake/encoding" => json!(self.encoding),
            _ => return None,
        })
    }

    fn initialize(&mut self, params: &Value) -> Value {
        let offered: Vec<String> = params["capabilities"]["general"]["positionEncodings"]
            .as_array()
            .map(|list| list.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let wanted = std::env::var("FAKE_ENCODING").unwrap_or_else(|_| "utf-16".into());
        self.encoding = if offered.contains(&wanted) {
            wanted
        } else {
            "utf-16".into()
        };
        let sync = std::env::var("FAKE_SYNC")
            .ok()
            .and_then(|sync| serde_json::from_str(&sync).ok())
            .unwrap_or_else(|| json!({"openClose": true, "change": 2, "save": {"includeText": true}}));
        let shapes = std::env::var_os("FAKE_SHAPES").is_some();
        let diagnostics = std::env::var_os("FAKE_DIAGNOSTICS_REGISTERED_LATER")
            .is_none()
            .then(|| json!({"interFileDependencies": false, "workspaceDiagnostics": false}));
        json!({
            "capabilities": {
                "positionEncoding": self.encoding,
                "textDocumentSync": sync,
                "documentFormattingProvider": shapes,
                "documentRangeFormattingProvider": shapes,
                "inlayHintProvider": shapes,
                "hoverProvider": true,
                "completionProvider": {"resolveProvider": true, "triggerCharacters": ["."]},
                "definitionProvider": true,
                "referencesProvider": true,
                "implementationProvider": true,
                "typeDefinitionProvider": true,
                "declarationProvider": true,
                "workspaceSymbolProvider": {"resolveProvider": true},
                "callHierarchyProvider": true,
                "typeHierarchyProvider": true,
                "renameProvider": {"prepareProvider": true},
                "documentSymbolProvider": true,
                "foldingRangeProvider": true,
                "diagnosticProvider": diagnostics,
                "signatureHelpProvider": {"triggerCharacters": ["("]},
                "codeActionProvider": {"resolveProvider": true},
                "executeCommandProvider": {"commands": ["fake.apply"]},
                "semanticTokensProvider": {
                    "legend": {"tokenTypes": ["keyword", "variable"], "tokenModifiers": ["declaration"]},
                    "full": {"delta": true},
                },
            },
            "serverInfo": {"name": "fake"},
        })
    }

    fn sleep(&self, id: Value, ms: u64) {
        let out = self.out.clone();
        let cancelled = self.cancelled.clone();
        std::thread::spawn(move || {
            for _ in 0..ms / 10 {
                std::thread::sleep(Duration::from_millis(10));
                if cancelled.lock_ignoring_poison().contains(&id.to_string()) {
                    send(
                        &out,
                        &json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32800, "message": "cancelled"}}),
                    );
                    return;
                }
            }
            send(&out, &json!({"jsonrpc": "2.0", "id": id, "result": "slept"}));
        });
    }

    /// Sends `result`, made from the documents as they are now, after the `FAKE_HOVER_MS` milliseconds.
    fn later(&self, id: Value, result: Value) {
        let ms = std::env::var("FAKE_HOVER_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(0);
        if ms == 0 {
            return self.reply(&id, result);
        }
        let out = self.out.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(ms));
            send(&out, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
        });
    }

    /// One diagnostic for each `error` in the document.
    fn publish(&self, uri: &str) {
        let Some((version, text)) = self.docs.get(uri) else {
            return;
        };
        let mut diagnostics = vec![];
        for (n, line) in text.split('\n').enumerate() {
            for (byte, _) in line.match_indices("error") {
                let start = column(line, byte, &self.encoding);
                let end = column(line, byte + 5, &self.encoding);
                diagnostics.push(json!({"range": span(n, start, end), "message": "an error", "severity": 1}));
            }
        }
        self.notify(
            "textDocument/publishDiagnostics",
            json!({"uri": uri, "version": version, "diagnostics": diagnostics}),
        );
    }

    /// The word at `position`, as a range.
    fn word_range(&self, uri: &str, position: &Value) -> Option<Value> {
        let (_, text) = self.docs.get(uri)?;
        let line_number = usize::try_from(position["line"].as_u64()?).ok()?;
        let line = text.split('\n').nth(line_number)?;
        let at = byte_of(line, position["character"].as_u64()?, &self.encoding);
        let word = |c: char| c.is_alphanumeric() || c == '_';
        let start = line[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| word(*c))
            .last()
            .map_or(at, |(i, _)| i);
        let end = line[at..]
            .char_indices()
            .find(|(_, c)| !word(*c))
            .map_or(line.len(), |(i, _)| at + i);
        Some(span(
            line_number,
            column(line, start, &self.encoding),
            column(line, end, &self.encoding),
        ))
    }

    fn word(&self, uri: &str, position: &Value) -> Option<String> {
        let range = self.word_range(uri, position)?;
        let (_, text) = self.docs.get(uri)?;
        let line = text
            .split('\n')
            .nth(usize::try_from(range["start"]["line"].as_u64()?).ok()?)?;
        let start = byte_of(line, range["start"]["character"].as_u64()?, &self.encoding);
        let end = byte_of(line, range["end"]["character"].as_u64()?, &self.encoding);
        Some(line[start..end].to_string())
    }

    fn hover(&self, uri: &str, position: &Value) -> Value {
        match (self.word(uri, position), self.word_range(uri, position)) {
            (Some(word), Some(range)) => {
                json!({"contents": {"kind": "markdown", "value": format!("`{word}`")}, "range": range})
            }
            _ => Value::Null,
        }
    }

    fn completion(&self, uri: &str, position: &Value) -> Value {
        let range = self.word_range(uri, position).unwrap_or(Value::Null);
        json!({"isIncomplete": true, "items": [
            {"label": "println", "insertTextFormat": 2, "textEdit": {"range": range, "newText": "println(${1:x})$0"}},
            {"label": "plain"},
        ]})
    }

    fn occurrences(&self, uri: &str, position: &Value) -> Vec<Value> {
        let (Some(word), Some((_, text))) = (self.word(uri, position), self.docs.get(uri)) else {
            return vec![];
        };
        let mut found = vec![];
        for (n, line) in text.split('\n').enumerate() {
            for (byte, _) in line.match_indices(word.as_str()) {
                let range = span(
                    n,
                    column(line, byte, &self.encoding),
                    column(line, byte + word.len(), &self.encoding),
                );
                found.push(json!({"uri": uri, "range": range}));
            }
        }
        found
    }

    /// The first occurrence of the word: as a link for a type definition, as a bare location for a declaration.
    fn link(&self, uri: &str, position: &Value, method: &str) -> Value {
        let Some(first) = self.occurrences(uri, position).into_iter().next() else {
            return Value::Null;
        };
        if method.ends_with("declaration") {
            return first;
        }
        json!([{"targetUri": uri, "targetRange": first["range"], "targetSelectionRange": first["range"]}])
    }

    /// Every occurrence of `query` in the open documents as a symbol with its range, and one more without a range for
    /// the query `lazy`, which resolve fills in.
    fn project_symbols(&self, query: &str) -> Value {
        let mut found = vec![];
        for (uri, (_, text)) in &self.docs {
            for (n, line) in text.split('\n').enumerate() {
                for (byte, _) in line.match_indices(query).filter(|_| !query.is_empty()) {
                    let range = span(
                        n,
                        column(line, byte, &self.encoding),
                        column(line, byte + query.len(), &self.encoding),
                    );
                    found.push(json!({"name": query, "kind": 12, "location": {"uri": uri, "range": range}}));
                    if query == "lazy" {
                        found.push(
                            json!({"name": query, "kind": 12, "location": {"uri": uri}, "data": {"range": range}}),
                        );
                    }
                }
            }
        }
        Value::Array(found)
    }

    /// The word at `position` as a hierarchy item.
    fn hierarchy_item(&self, uri: &str, position: &Value) -> Option<Value> {
        let word = self.word(uri, position)?;
        let range = self.word_range(uri, position)?;
        Some(
            json!({"name": word, "kind": 12, "uri": uri, "range": range, "selectionRange": range, "data": {"word": word}}),
        )
    }

    /// One caller or callee of `item`, named after it, called wherever its name occurs in its document.
    fn calls(&self, item: &Value, method: &str) -> Value {
        let uri = item["uri"].as_str().unwrap_or_default();
        let ranges: Vec<Value> = self
            .occurrences(uri, &item["selectionRange"]["start"])
            .into_iter()
            .map(|l| l["range"].clone())
            .collect();
        let mut neighbour = item.clone();
        let (prefix, key) = if method.ends_with("incomingCalls") {
            ("caller", "from")
        } else {
            ("callee", "to")
        };
        neighbour["name"] = json!(format!("{prefix} of {}", item["name"].as_str().unwrap_or_default()));
        json!([{key: neighbour, "fromRanges": ranges}])
    }

    fn rename(&self, uri: &str, position: &Value, new_name: &Value) -> Value {
        let edits: Vec<Value> = self
            .occurrences(uri, position)
            .into_iter()
            .map(|l| json!({"range": l["range"], "newText": new_name}))
            .collect();
        json!({"changes": {uri: edits}})
    }

    /// A `keyword` token over every `let`; each answer gets a new result id, deltas replace everything.
    fn tokens(&mut self, uri: &str, method: &str) -> Value {
        let Some((_, text)) = self.docs.get(uri) else {
            return Value::Null;
        };
        let mut data = vec![];
        let (mut last_line, mut last_start) = (0u64, 0u64);
        for (n, line) in text.split('\n').enumerate() {
            for (byte, _) in line.match_indices("let") {
                let n = n as u64;
                let start = column(line, byte, &self.encoding);
                let delta_start = if n == last_line { start - last_start } else { start };
                data.extend([
                    n - last_line,
                    delta_start,
                    column(line, byte + 3, &self.encoding) - start,
                    0,
                    1,
                ]);
                (last_line, last_start) = (n, start);
            }
        }
        self.tokens += 1;
        let previous = self.sent_tokens.insert(uri.to_string(), data.len()).unwrap_or(0);
        if method.ends_with("delta") {
            json!({"resultId": self.tokens.to_string(), "edits": [{"start": 0, "deleteCount": previous, "data": data}]})
        } else {
            json!({"resultId": self.tokens.to_string(), "data": data})
        }
    }

    fn pulled(&self, uri: &str, params: &Value) -> Value {
        let Some((version, _)) = self.docs.get(uri) else {
            return Value::Null;
        };
        let id = version.to_string();
        if params["previousResultId"].as_str() == Some(id.as_str()) {
            return json!({"kind": "unchanged", "resultId": id});
        }
        json!({"kind": "full", "resultId": id, "items": [{"range": self.range(uri, 0, 0, 0, 1), "message": "pulled"}]})
    }

    fn range(&self, _uri: &str, a: u64, b: u64, c: u64, d: u64) -> Value {
        json!({"start": {"line": a, "character": b}, "end": {"line": c, "character": d}})
    }
}

fn span(line: usize, start: u64, end: u64) -> Value {
    json!({"start": {"line": line, "character": start}, "end": {"line": line, "character": end}})
}

/// The column of byte `byte` in `line`, counted in `encoding`.
fn column(line: &str, byte: usize, encoding: &str) -> u64 {
    let prefix = &line[..byte];
    let count = match encoding {
        "utf-8" => prefix.len(),
        "utf-32" => prefix.chars().count(),
        _ => prefix.encode_utf16().count(),
    };
    count as u64
}

/// The byte offset of `column`, counted in `encoding`, in `line`.
fn byte_of(line: &str, column: u64, encoding: &str) -> usize {
    let mut counted = 0u64;
    for (byte, c) in line.char_indices() {
        if counted >= column {
            return byte;
        }
        counted += match encoding {
            "utf-8" => c.len_utf8() as u64,
            "utf-32" => 1,
            _ => c.len_utf16() as u64,
        };
    }
    line.len()
}

/// The byte offset of an LSP position in `text`.
fn offset(text: &str, position: &Value, encoding: &str) -> usize {
    let line = position["line"].as_u64().unwrap_or(0);
    let character = position["character"].as_u64().unwrap_or(0);
    let mut start = 0;
    for (n, l) in text.split('\n').enumerate() {
        if n as u64 == line {
            return start + byte_of(l, character, encoding);
        }
        start += l.len() + 1;
    }
    text.len()
}
