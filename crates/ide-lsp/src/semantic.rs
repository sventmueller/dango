//! Semantic tokens: the server's packed, relative numbers turned into positioned tokens with names from its legend.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::encoding::Encoding;
use crate::error::Error;
use crate::text::Text;

/// The names a server gives token types and modifiers; tokens refer to them by index.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Legend {
    /// Type names, such as `function` or `variable`.
    pub token_types: Vec<String>,
    /// Modifier names, such as `readonly`; a token's bit `n` stands for the `n`th.
    pub token_modifiers: Vec<String>,
}

/// One token: a stretch of one line, with columns in UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Its line.
    pub line: u32,
    /// Its first column.
    pub start: u32,
    /// Its length.
    pub length: u32,
    /// Its type, an index into the legend's types.
    pub kind: u32,
    /// Its modifiers, a bit set over the legend's modifiers.
    pub modifiers: u32,
}

/// The tokens of a whole document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTokens {
    /// The names the indices in the tokens refer to.
    pub legend: Arc<Legend>,
    /// In document order.
    pub tokens: Vec<Token>,
}

impl SemanticTokens {
    /// The type name of `token`.
    pub fn kind(&self, token: &Token) -> Option<&str> {
        self.legend
            .token_types
            .get(usize::try_from(token.kind).ok()?)
            .map(String::as_str)
    }

    /// The modifier names of `token`.
    pub fn modifiers(&self, token: &Token) -> Vec<&str> {
        self.legend
            .token_modifiers
            .iter()
            .enumerate()
            .filter(|(bit, _)| *bit < 32 && token.modifiers & (1 << bit) != 0)
            .map(|(_, name)| name.as_str())
            .collect()
    }
}

/// What the client keeps between requests to ask only for changes.
#[derive(Debug, Clone, Default)]
pub(crate) struct Previous {
    pub(crate) result_id: Option<String>,
    /// Shared, as each request takes it along to apply the answer to.
    pub(crate) data: Arc<[u32]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Answer {
    result_id: Option<String>,
    data: Option<Vec<u32>>,
    edits: Option<Vec<Edit>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Edit {
    start: usize,
    delete_count: usize,
    #[serde(default)]
    data: Vec<u32>,
}

/// The packed numbers after `answer` to a full or delta request, applied to `previous`.
pub(crate) fn apply(previous: &Previous, answer: Value) -> Result<Previous, Error> {
    if answer.is_null() {
        return Ok(Previous::default());
    }
    let answer: Answer = serde_json::from_value(answer).map_err(|e| Error::Malformed(e.to_string()))?;
    let data = match (answer.data, answer.edits) {
        (Some(data), _) => data,
        (None, Some(mut edits)) => {
            let mut data = previous.data.to_vec();
            // Every edit's offsets refer to the previous array, so applying from the back keeps them valid.
            edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
            for edit in edits {
                let end = edit.start.saturating_add(edit.delete_count);
                if end > data.len() {
                    return Err(Error::Malformed("a semantic tokens edit past the end".into()));
                }
                data.splice(edit.start..end, edit.data);
            }
            data
        }
        (None, None) => return Err(Error::Malformed("semantic tokens without data or edits".into())),
    };
    Ok(Previous {
        result_id: answer.result_id,
        data: data.into(),
    })
}

/// The tokens `data` describes, with columns recounted from `encoding` to UTF-16 along `text`.
pub(crate) fn decode(data: &[u32], legend: Arc<Legend>, text: Option<&Text>, encoding: Encoding) -> SemanticTokens {
    let mut tokens = Vec::with_capacity(data.len() / 5);
    let (mut line, mut start) = (0u32, 0u32);
    let text = text.filter(|_| encoding != Encoding::Utf16);
    let mut current = None;
    for &[delta_line, delta_start, length, kind, modifiers] in data.as_chunks::<5>().0 {
        if delta_line > 0 {
            line = line.saturating_add(delta_line);
            start = delta_start;
        } else {
            start = start.saturating_add(delta_start);
        }
        if let Some(text) = text
            && current.as_ref().is_none_or(|(n, _)| *n != line)
        {
            current = Some((line, text.line(line)));
        }
        let (first, length) = match current.as_ref().and_then(|(_, content)| content.as_deref()) {
            None => (start, length),
            Some(text) => {
                let first = crate::encoding::convert(text, start, encoding, Encoding::Utf16);
                let end = crate::encoding::convert(text, start.saturating_add(length), encoding, Encoding::Utf16);
                (first, end.saturating_sub(first))
            }
        };
        tokens.push(Token {
            line,
            start: first,
            length,
            kind,
            modifiers,
        });
    }
    SemanticTokens { legend, tokens }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn legend() -> Arc<Legend> {
        Arc::new(Legend {
            token_types: vec!["variable".into(), "function".into()],
            token_modifiers: vec!["declaration".into(), "readonly".into()],
        })
    }

    #[test]
    fn relative_numbers_become_absolute_tokens_with_names() {
        let data = [1, 4, 3, 1, 0b01, 0, 6, 2, 0, 0b11, 2, 1, 1, 0, 0];
        let tokens = decode(&data, legend(), None, Encoding::Utf16);
        let positions: Vec<(u32, u32, u32)> = tokens.tokens.iter().map(|t| (t.line, t.start, t.length)).collect();
        assert_eq!(positions, [(1, 4, 3), (1, 10, 2), (3, 1, 1)]);
        assert_eq!(tokens.kind(&tokens.tokens[0]), Some("function"));
        assert_eq!(tokens.modifiers(&tokens.tokens[1]), ["declaration", "readonly"]);
    }

    #[test]
    fn utf8_columns_are_recounted_on_their_line() {
        let text = Text::new("let ä = 😀x;");
        // `x` starts at byte 13 and UTF-16 column 10.
        let tokens = decode(&[0, 13, 1, 0, 0, 0, 0, 0, 0, 0], legend(), Some(&text), Encoding::Utf8);
        assert_eq!((tokens.tokens[0].start, tokens.tokens[0].length), (10, 1));
    }

    #[test]
    fn a_delta_edits_the_previous_numbers() {
        let first = apply(
            &Previous::default(),
            json!({"resultId": "1", "data": [0, 0, 1, 0, 0, 0, 2, 1, 0, 0]}),
        )
        .unwrap();
        let delta = json!({"resultId": "2", "edits": [
            {"start": 5, "deleteCount": 5, "data": [1, 0, 3, 1, 0]},
            {"start": 0, "deleteCount": 0, "data": [0, 0, 0, 0, 0]},
        ]});
        let second = apply(&first, delta).unwrap();
        assert_eq!(second.result_id.as_deref(), Some("2"));
        assert_eq!(&*second.data, &[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 3, 1, 0]);
        assert!(apply(&first, json!({"edits": [{"start": 9, "deleteCount": 5}]})).is_err());
        assert!(apply(&first, Value::Null).unwrap().data.is_empty());
    }
}
