//! Copilot's answers to `textDocument/inlineCompletion` and `textDocument/copilotInlineEdit`, as proposals in bytes
//! of the request's text.

use std::ops::Range;

use ide_lsp::Encoding;
use ide_lsp::lsp_types::Range as LspRange;
use serde_json::Value;

use super::position::{differing, offset};
use crate::{Error, Prediction};

/// The proposals of an `InlineCompletionList`, or of a bare list of items, made at byte `caret` of `text`.
pub(crate) fn completions(
    answer: &Value,
    text: &str,
    caret: usize,
    encoding: Encoding,
) -> Result<Vec<Prediction>, Error> {
    let items = match answer {
        Value::Null => return Ok(Vec::new()),
        Value::Array(items) => items,
        Value::Object(list) => match list.get("items") {
            Some(Value::Array(items)) => items,
            _ => return Err(Error::Malformed("an inline completion list without items".into())),
        },
        _ => return Err(Error::Malformed("not an inline completion list".into())),
    };
    let mut found = Vec::new();
    for item in items {
        let insert = match item.get("insertText") {
            Some(Value::String(insert)) => insert.as_str(),
            Some(snippet) => match snippet.get("value").and_then(Value::as_str) {
                Some(value) => value,
                None => continue,
            },
            None => continue,
        };
        let range = match item.get("range") {
            None | Some(Value::Null) => caret..caret,
            Some(range) => match bytes(range, text, encoding) {
                Some(range) => range,
                None => continue,
            },
        };
        if let Some(prediction) = smallest(text, range, insert, command(item)) {
            push(&mut found, prediction);
        }
    }
    Ok(found)
}

/// The proposals of a `copilotInlineEdit` answer that belong to the document `uri` at `version`.
pub(crate) fn edits(
    answer: &Value,
    text: &str,
    uri: &str,
    version: i32,
    encoding: Encoding,
) -> Result<Vec<Prediction>, Error> {
    let edits = match answer.get("edits") {
        None | Some(Value::Null) if answer.is_null() || answer.is_object() => return Ok(Vec::new()),
        Some(Value::Array(edits)) => edits,
        _ => return Err(Error::Malformed("an inline edit answer without edits".into())),
    };
    let mut found = Vec::new();
    for edit in edits {
        let document = edit.get("textDocument");
        let same_uri = document
            .and_then(|d| d.get("uri"))
            .and_then(Value::as_str)
            .is_none_or(|other| same(other, uri));
        let same_version = document
            .and_then(|d| d.get("version"))
            .and_then(Value::as_i64)
            .is_none_or(|other| other == i64::from(version));
        let (Some(new), Some(range)) = (edit.get("text").and_then(Value::as_str), edit.get("range")) else {
            continue;
        };
        if !same_uri || !same_version {
            continue;
        }
        let Some(range) = bytes(range, text, encoding) else {
            continue;
        };
        if let Some(prediction) = smallest(text, range, new, command(edit)) {
            push(&mut found, prediction);
        }
    }
    Ok(found)
}

fn same(a: &str, b: &str) -> bool {
    let parse = |s: &str| {
        s.parse::<ide_lsp::lsp_types::Uri>()
            .ok()
            .map(|u| ide_lsp::uri::normalize(&u))
    };
    a == b || parse(a).is_some_and(|a| parse(b).is_some_and(|b| a == b))
}

fn command(item: &Value) -> Option<Value> {
    item.get("command").filter(|c| c.is_object()).cloned()
}

fn bytes(range: &Value, text: &str, encoding: Encoding) -> Option<Range<usize>> {
    let range: LspRange = serde_json::from_value(range.clone()).ok()?;
    let start = offset(text, range.start, encoding)?;
    let end = offset(text, range.end, encoding)?;
    (start <= end).then_some(start..end)
}

/// The proposal that replaces `range` of `text` with `new`, shrunk to what actually changes, so that text Copilot
/// repeats from before the caret becomes an insertion at the caret; none when nothing changes.
fn smallest(text: &str, range: Range<usize>, new: &str, feedback: Option<Value>) -> Option<Prediction> {
    let old = &text[range.clone()];
    let (start, old_end, new_end) = differing(old, new);
    if start == old_end && start == new_end {
        return None;
    }
    Some(Prediction {
        range: range.start + start..range.start + old_end,
        text: new[start..new_end].to_string(),
        feedback,
    })
}

fn push(found: &mut Vec<Prediction>, prediction: Prediction) {
    if !found
        .iter()
        .any(|p| p.range == prediction.range && p.text == prediction.text)
    {
        found.push(prediction);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const TEXT: &str = "fn main() {\n    let x = foo\n}\n";

    #[test]
    fn a_completion_that_repeats_the_line_becomes_an_insertion_at_the_caret() {
        let caret = TEXT.find("foo").unwrap() + 3;
        let answer = json!({"items": [{
            "insertText": "    let x = foo(1);",
            "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 15}},
            "command": {"command": "github.copilot.didAcceptCompletionItem", "arguments": ["id-1"]},
        }]});
        let found = completions(&answer, TEXT, caret, Encoding::Utf16).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].range.clone(), found[0].text.as_str()), (caret..caret, "(1);"));
        assert_eq!(found[0].feedback.as_ref().unwrap()["arguments"][0], "id-1");
    }

    #[test]
    fn items_without_a_range_insert_at_the_caret_and_odd_items_are_skipped() {
        let answer = json!([
            {"insertText": "bar"},
            {"insertText": {"kind": "snippet", "value": "baz"}},
            {"insertText": "bar"},
            {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}},
            {"insertText": "x", "range": {"start": {"line": 40, "character": 0}, "end": {"line": 40, "character": 0}}},
        ]);
        let found = completions(&answer, TEXT, 5, Encoding::Utf16).unwrap();
        let texts: Vec<&str> = found.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, ["bar", "baz"]);
        assert_eq!(completions(&Value::Null, TEXT, 5, Encoding::Utf16), Ok(vec![]));
        assert!(completions(&json!("no"), TEXT, 5, Encoding::Utf16).is_err());
    }

    #[test]
    fn a_next_edit_replaces_text_away_from_the_caret_for_its_document_and_version_only() {
        let uri = "file:///p/main.rs";
        let edit = |uri: &str, version: i32| {
            json!({
                "text": "fn start() {",
                "textDocument": {"uri": uri, "version": version},
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 11}},
                "command": {"command": "github.copilot.didAcceptCompletionItem", "arguments": ["e"]},
            })
        };
        let answer = json!({"edits": [edit(uri, 4), edit("file:///p/other.rs", 4), edit(uri, 3)]});
        let found = edits(&answer, TEXT, uri, 4, Encoding::Utf16).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].range.clone(), found[0].text.as_str()), (3..7, "start"));
        assert_eq!(edits(&json!({}), TEXT, uri, 4, Encoding::Utf16), Ok(vec![]));
        assert!(edits(&json!({"edits": 3}), TEXT, uri, 4, Encoding::Utf16).is_err());
    }
}
