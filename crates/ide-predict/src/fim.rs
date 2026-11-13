//! Fill-in-the-middle with a local model: Ollama through `/api/generate` with `suffix`, LM Studio through
//! `/v1/completions` with the marks of the model's family (Review C16). The code stays on this Mac.
//!
//! Each request has a connection of its own; cancelling closes it at once, so that the server notices the client
//! gone and stops generating instead of spending the memory and GPU time the agents need (R9).

pub(crate) mod answer;
mod family;

use std::ops::Range;

use ide_agent::chat::{ErrorKind, ProviderError};
use ide_agent::http::Http;
use serde_json::{Value, json};

pub use family::Family;

use crate::{Cancel, Error, Prediction, Provider, Reply, Request, texts};

/// Where Ollama answers unless the settings say otherwise.
pub const OLLAMA_BASE: &str = "http://localhost:11434";

/// Where LM Studio's server answers unless the settings say otherwise.
pub const LMSTUDIO_BASE: &str = "http://localhost:1234";

/// The most of the text before the caret a request carries; more costs a local model time without helping much.
const PREFIX_BYTES: usize = 8 * 1024;
/// The most of the text after the caret a request carries.
const SUFFIX_BYTES: usize = 2 * 1024;
/// The most tokens a proposal may take.
const MAX_TOKENS: u32 = 128;
/// OpenAI's completions take at most four stop sequences, and servers that copy the API may refuse more; the rest
/// are cut from the answer here.
const STOPS_SENT: usize = 4;
/// Low, so that the same gap gets about the same proposal.
const TEMPERATURE: f64 = 0.2;

/// Proposals from a model under Ollama, which knows each model's fill-in-the-middle marks from its template.
pub struct Ollama {
    base: String,
    model: String,
}

impl Ollama {
    /// `model` at the server at `base`, such as [`OLLAMA_BASE`].
    pub fn new(base: &str, model: &str) -> Ollama {
        Ollama {
            base: root(base),
            model: model.trim().to_string(),
        }
    }

    /// The models installed at `base`; blocks, so call it off the main thread.
    pub fn models(base: &str) -> Result<Vec<String>, Error> {
        let list = Http::new()
            .get_json(&format!("{}/api/tags", root(base)), &[])
            .map_err(|e| failure("Ollama", "", &e))?;
        names(&list, "models", "name")
    }
}

impl Provider for Ollama {
    fn name(&self) -> &str {
        "Ollama"
    }

    fn predict(&self, request: Request, cancel: Cancel, reply: Reply) {
        if self.model.is_empty() {
            return answer_off_thread(reply, Err(Error::Unavailable(texts::no_model())));
        }
        let gap = Gap::of(&request);
        let stops = Family::of(&self.model).map_or(&[][..], Family::stops);
        let body = json!({
            "model": self.model,
            "prompt": gap.prefix,
            "suffix": gap.suffix,
            "stream": false,
            "options": {"temperature": TEMPERATURE, "num_predict": MAX_TOKENS},
        });
        let ask = Ask {
            service: "Ollama",
            model: self.model.clone(),
            url: format!("{}/api/generate", self.base),
            body,
            read: answer::ollama,
            stops,
        };
        ask.send(gap, cancel, reply);
    }

    fn accepted(&self, _: &Prediction) {}
}

/// Proposals from a model under LM Studio, asked without a chat template in the marks of its [`Family`].
pub struct LmStudio {
    base: String,
    model: String,
}

impl LmStudio {
    /// `model` at the server at `base`, such as [`LMSTUDIO_BASE`].
    pub fn new(base: &str, model: &str) -> LmStudio {
        LmStudio {
            base: root(base),
            model: model.trim().to_string(),
        }
    }

    /// The models the server at `base` offers; blocks, so call it off the main thread.
    pub fn models(base: &str) -> Result<Vec<String>, Error> {
        let list = Http::new()
            .get_json(&format!("{}/v1/models", root(base)), &[])
            .map_err(|e| failure("LM Studio", "", &e))?;
        names(&list, "data", "id")
    }
}

impl Provider for LmStudio {
    fn name(&self) -> &str {
        "LM Studio"
    }

    fn predict(&self, request: Request, cancel: Cancel, reply: Reply) {
        if self.model.is_empty() {
            return answer_off_thread(reply, Err(Error::Unavailable(texts::no_model())));
        }
        let Some(family) = Family::of(&self.model) else {
            return answer_off_thread(reply, Err(Error::Unavailable(texts::no_fill_in(&self.model))));
        };
        let gap = Gap::of(&request);
        let body = json!({
            "model": self.model,
            "prompt": family.prompt(&gap.prefix, &gap.suffix),
            "max_tokens": MAX_TOKENS,
            "temperature": TEMPERATURE,
            "stop": &family.stops()[..family.stops().len().min(STOPS_SENT)],
            "stream": false,
        });
        let ask = Ask {
            service: "LM Studio",
            model: self.model.clone(),
            url: format!("{}/v1/completions", self.base),
            body,
            read: answer::lmstudio,
            stops: family.stops(),
        };
        ask.send(gap, cancel, reply);
    }

    fn accepted(&self, _: &Prediction) {}
}

/// The base without a trailing `/v1`, which the agent's settings for the same server carry.
fn root(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    base.strip_suffix("/v1").unwrap_or(base).to_string()
}

fn names(list: &Value, field: &str, key: &str) -> Result<Vec<String>, Error> {
    let items = list
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Malformed(format!("no {field} in the list of models")))?;
    let mut names: Vec<String> = items
        .iter()
        .filter_map(|item| item.get(key).and_then(Value::as_str).map(str::to_string))
        .collect();
    names.sort();
    Ok(names)
}

/// The text around the caret a request carries, and the bytes of the document a proposal replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Gap {
    prefix: String,
    suffix: String,
    range: Range<usize>,
}

impl Gap {
    fn of(request: &Request) -> Gap {
        let text = &request.text;
        let end = clamp(text, request.selection.end);
        let start = clamp(text, request.selection.start.min(end));
        let mut from = start.saturating_sub(PREFIX_BYTES);
        while !text.is_char_boundary(from) {
            from += 1;
        }
        if from > 0
            && let Some(line) = text[from..start].find('\n')
        {
            from += line + 1;
        }
        let mut to = (end + SUFFIX_BYTES).min(text.len());
        while !text.is_char_boundary(to) {
            to -= 1;
        }
        Gap {
            prefix: text[from..start].to_string(),
            suffix: text[end..to].to_string(),
            range: start..end,
        }
    }
}

fn clamp(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// One request to a local server.
struct Ask {
    service: &'static str,
    model: String,
    url: String,
    body: Value,
    read: fn(&Value) -> Result<String, Error>,
    stops: &'static [&'static str],
}

impl Ask {
    /// Sends the request on a thread of its own and replies there.
    fn send(self, gap: Gap, cancel: Cancel, reply: Reply) {
        let name = format!("predict-{}", self.service);
        let service = self.service;
        ide_core::thread::detach_answering(
            &name,
            reply,
            move |reply| {
                let answer = self.run(&gap, &cancel);
                reply(if cancel.is_cancelled() {
                    Err(Error::Cancelled)
                } else {
                    answer
                });
            },
            |reply, error| reply(Err(Error::Unavailable(texts::failed(service, &error.to_string())))),
        );
    }

    fn run(&self, gap: &Gap, cancel: &Cancel) -> Result<Vec<Prediction>, Error> {
        let http = Http::new();
        let closing = http.clone();
        cancel.on_cancel(move || closing.close());
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let answer = http
            .post_json(&self.url, &self.body)
            .map_err(|e| failure(self.service, &self.model, &e))?;
        let text = (self.read)(&answer)?;
        Ok(answer::clean(&text, &gap.suffix, self.stops)
            .map(|text| Prediction {
                range: gap.range.clone(),
                text,
                feedback: None,
            })
            .into_iter()
            .collect())
    }
}

/// Replies on a thread of its own, as the trait promises, when there is nothing to ask.
fn answer_off_thread(reply: Reply, answer: Result<Vec<Prediction>, Error>) {
    crate::on_own_thread(move || reply(answer));
}

/// What a failed request means for the user.
fn failure(service: &str, model: &str, error: &ProviderError) -> Error {
    let message = error.message.to_ascii_lowercase();
    match error.kind {
        ErrorKind::Network => Error::Unavailable(texts::unreachable(service)),
        ErrorKind::Malformed => Error::Malformed(error.message.clone()),
        _ if message.contains("does not support insert") || message.contains("does not support fim") => {
            Error::Unavailable(texts::no_fill_in(model))
        }
        ErrorKind::NotFound if !model.is_empty() => Error::Unavailable(texts::unknown_model(service, model)),
        _ if !model.is_empty() && message.contains("not found") => {
            Error::Unavailable(texts::unknown_model(service, model))
        }
        _ => Error::Unavailable(texts::failed(service, error.message.trim())),
    }
}

#[cfg(test)]
mod tests;
