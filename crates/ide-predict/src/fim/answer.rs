//! What a local model answered: the text in Ollama's and LM Studio's JSON, cut down to what fits the gap.

use serde_json::Value;

use crate::Error;

/// The text of an answer from Ollama's `/api/generate`.
pub(crate) fn ollama(answer: &Value) -> Result<String, Error> {
    answer
        .get("response")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::Malformed(shape(answer)))
}

/// The text of an answer from LM Studio's `/v1/completions`.
pub(crate) fn lmstudio(answer: &Value) -> Result<String, Error> {
    answer
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::Malformed(shape(answer)))
}

fn shape(answer: &Value) -> String {
    let mut text = answer.to_string();
    if text.len() > 200 {
        let mut end = 200;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

/// Characters that close what the line opened; a model that writes them again before the caret's rest of the line
/// would double them.
const CLOSERS: &[char] = &[')', ']', '}', '>', '"', '\'', '`', ';', ','];

/// What `text`, the model's answer for the gap before `suffix`, proposes: cut at the first of `stops`, cut where it
/// repeats the rest of the caret's line or, at the end of a line, the next line; `None` when nothing is left.
pub(crate) fn clean(text: &str, suffix: &str, stops: &[&str]) -> Option<String> {
    let mut text = text;
    if let Some(at) = stops.iter().filter_map(|stop| text.find(stop)).min() {
        text = &text[..at];
    }
    let rest = suffix.split('\n').next().unwrap_or_default().trim();
    let kept = if rest.is_empty() {
        before_repeated_line(text, suffix)
    } else {
        // Text that wraps onto new lines in the middle of a line would leave the line's rest behind it.
        let line = text.split('\n').next().unwrap_or_default();
        without_rest(line, rest)
    };
    let kept = kept.trim_end();
    (!kept.trim().is_empty()).then(|| kept.to_string())
}

fn without_rest<'a>(line: &'a str, rest: &str) -> &'a str {
    let trimmed = line.trim_end();
    if let Some(cut) = trimmed.strip_suffix(rest) {
        return cut;
    }
    let closers: String = rest.chars().take_while(|c| CLOSERS.contains(c)).collect();
    let mut ends = closers
        .char_indices()
        .map(|(i, c)| i + c.len_utf8())
        .collect::<Vec<_>>();
    ends.reverse();
    ends.into_iter()
        .find_map(|end| trimmed.strip_suffix(&closers[..end]))
        .unwrap_or(line)
}

fn before_repeated_line<'a>(text: &'a str, suffix: &str) -> &'a str {
    let Some(next) = suffix
        .split('\n')
        .skip(1)
        .map(str::trim_end)
        .find(|line| !line.trim().is_empty())
    else {
        return text;
    };
    let mut start = 0;
    for line in text.split('\n') {
        if line.trim_end() == next {
            return &text[..start];
        }
        start += line.len() + 1;
    }
    text
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_text_is_read_from_each_servers_answer() {
        assert_eq!(ollama(&json!({"response": "x + 1", "done": true})), Ok("x + 1".into()));
        assert_eq!(
            lmstudio(&json!({"choices": [{"text": "y", "finish_reason": "stop"}]})),
            Ok("y".into())
        );
        assert!(matches!(ollama(&json!({"choices": []})), Err(Error::Malformed(_))));
        assert!(matches!(lmstudio(&json!([1, 2])), Err(Error::Malformed(_))));
    }

    #[test]
    fn a_proposal_ends_at_the_first_stop_mark() {
        let stops = ["<|endoftext|>", "<|fim_prefix|>"];
        assert_eq!(
            clean("a + b<|fim_prefix|>junk<|endoftext|>", "\n", &stops).as_deref(),
            Some("a + b")
        );
        assert_eq!(clean("<|endoftext|>", "\n", &stops), None);
    }

    #[test]
    fn text_that_repeats_the_rest_of_the_line_is_cut() {
        assert_eq!(clean("a, b);", ");", &[]).as_deref(), Some("a, b"));
        assert_eq!(clean("a, b)", ");\nnext", &[]).as_deref(), Some("a, b"));
        assert_eq!(clean("len()", ")", &[]).as_deref(), Some("len("));
        assert_eq!(clean("x > 0) {\n    run();\n}", ") {\n", &[]).as_deref(), Some("x > 0"));
        assert_eq!(clean("value", " + 1;", &[]).as_deref(), Some("value"));
        assert_eq!(clean("\nfn other() {}", ")", &[]), None);
    }

    #[test]
    fn at_the_end_of_a_line_a_proposal_stops_before_it_repeats_the_next_line() {
        let suffix = "\n}\n";
        assert_eq!(
            clean("    let x = 1;\n    x\n}\n", suffix, &[]).as_deref(),
            Some("    let x = 1;\n    x")
        );
        assert_eq!(
            clean("if a {\n        b();\n    }", "\n}\n", &[]).as_deref(),
            Some("if a {\n        b();\n    }")
        );
        assert_eq!(clean("}", suffix, &[]), None);
        assert_eq!(clean("   \n\n", "", &[]), None);
    }
}
