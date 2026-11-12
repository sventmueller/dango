//! A prediction from Copilot: an inline completion and next edits asked for together, answered once both are in.

use std::sync::{Arc, Mutex};

use ide_lsp::Ticket;
use serde_json::{Value, json};

use super::{Account, Copilot, Core, Job, answer, error_of, lock, position};
use crate::{Cancel, Error, Indent, Prediction, Provider, Reply, Request, on_own_thread, texts};

/// The two answers of one prediction, gathered until both are in.
struct Join {
    cancel: Cancel,
    state: Mutex<Gathered>,
}

struct Gathered {
    reply: Option<Reply>,
    left: u8,
    completions: Option<Result<Vec<Prediction>, Error>>,
    edits: Vec<Prediction>,
}

impl Join {
    fn completions(&self, answer: Result<Vec<Prediction>, Error>) {
        let mut state = lock(&self.state);
        state.completions = Some(answer);
        self.done(state);
    }

    /// Next edits are a bonus: a server that does not offer them, or fails at them, still gives completions.
    fn edits(&self, answer: Result<Vec<Prediction>, Error>) {
        let mut state = lock(&self.state);
        if let Ok(edits) = answer {
            state.edits = edits;
        }
        self.done(state);
    }

    fn done(&self, mut state: std::sync::MutexGuard<'_, Gathered>) {
        state.left = state.left.saturating_sub(1);
        if state.left > 0 {
            return;
        }
        let Some(reply) = state.reply.take() else { return };
        let edits = std::mem::take(&mut state.edits);
        let answer = match state.completions.take() {
            Some(Ok(mut found)) => {
                for edit in edits {
                    if !found.iter().any(|p| p.range == edit.range && p.text == edit.text) {
                        found.push(edit);
                    }
                }
                Ok(found)
            }
            Some(Err(_)) if !edits.is_empty() => Ok(edits),
            Some(Err(error)) => Err(error),
            None => Ok(edits),
        };
        drop(state);
        if self.cancel.is_cancelled() {
            // A cancel ends the requests on the caller's thread; the reply must not run there.
            on_own_thread(move || reply(Err(Error::Cancelled)));
        } else {
            reply(answer);
        }
    }
}

impl Provider for Copilot {
    fn name(&self) -> &str {
        "Copilot"
    }

    fn predict(&self, request: Request, cancel: Cancel, reply: Reply) {
        let core = self.core.clone();
        let job: Job = Box::new(move || core.predict(request, cancel, reply));
        if let Err(stopped) = self.jobs.send(job) {
            on_own_thread(stopped.0);
        }
    }

    fn accepted(&self, prediction: &Prediction) {
        self.core.accepted(prediction);
    }
}

impl Core {
    /// Tells the server the text and asks it, on the provider's thread for predictions.
    fn predict(&self, request: Request, cancel: Cancel, reply: Reply) {
        if cancel.is_cancelled() {
            return reply(Err(Error::Cancelled));
        }
        match self.shared.account() {
            Account::NeedsSignIn => return reply(Err(Error::SignIn)),
            Account::NoSubscription { .. } => {
                return reply(Err(Error::Unavailable(texts::no_subscription())));
            }
            _ => {}
        }
        let uri = ide_lsp::uri::from_path(&request.path);
        let version = self.sync(&request, &uri);
        let encoding = self.server.encoding();
        let caret = clamp(&request.text, request.selection.end);
        let at = position::position(&request.text, caret, encoding);
        let document = json!({"uri": uri.as_str(), "version": version});
        let (tab_size, insert_spaces) = match request.indent {
            Indent::Spaces(n) => (u32::from(n), true),
            Indent::Tabs => (4, false),
        };
        let completion = json!({
            "textDocument": document,
            "position": at,
            "context": {"triggerKind": 2},
            "formattingOptions": {"tabSize": tab_size, "insertSpaces": insert_spaces},
        });
        let edit = json!({"textDocument": document, "position": at});
        let join = Arc::new(Join {
            cancel: cancel.clone(),
            state: Mutex::new(Gathered {
                reply: Some(reply),
                left: 2,
                completions: None,
                edits: Vec::new(),
            }),
        });
        let text = Arc::new(request.text);
        let down = self.is_down();
        let mut tickets: Vec<Ticket> = Vec::new();
        let (to, text_c, shared) = (join.clone(), text.clone(), self.shared.clone());
        match self
            .server
            .request_value("textDocument/inlineCompletion", completion, move |answer| {
                let found = answer
                    .map_err(|e| error_of(e, down))
                    .and_then(|v| answer::completions(&v, &text_c, caret, encoding));
                if found == Err(Error::SignIn) {
                    shared.set(Account::NeedsSignIn);
                }
                to.completions(found);
            }) {
            Ok(ticket) => tickets.push(ticket),
            Err(error) => join.completions(Err(error_of(error, down))),
        }
        let (to, uri_e) = (join.clone(), uri.as_str().to_string());
        match self
            .server
            .request_value("textDocument/copilotInlineEdit", edit, move |answer| {
                let found = answer
                    .map_err(|e| error_of(e, down))
                    .and_then(|v| answer::edits(&v, &text, &uri_e, version, encoding));
                to.edits(found);
            }) {
            Ok(ticket) => tickets.push(ticket),
            Err(error) => join.edits(Err(error_of(error, down))),
        }
        cancel.on_cancel(move || tickets.iter().for_each(Ticket::cancel));
    }

    fn accepted(&self, prediction: &Prediction) {
        let Some(command) = prediction.feedback.as_ref() else {
            return;
        };
        let Some(name) = command.get("command").and_then(Value::as_str) else {
            return;
        };
        let arguments = command.get("arguments").cloned().unwrap_or_else(|| json!([]));
        let params = json!({"command": name, "arguments": arguments});
        let _ = self.server.request_value("workspace/executeCommand", params, |_| {});
    }
}

fn clamp(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}
