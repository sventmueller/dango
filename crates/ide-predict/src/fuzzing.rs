//! Entry points for the fuzz targets in `fuzz/`.

use ide_lsp::Encoding;
use serde_json::Value;

use crate::Prediction;
use crate::fim::Family;

/// Reads `bytes` as a local model's or Copilot's answer for a gap in a text made of the same bytes, and cuts the
/// proposal as the providers do. A cut proposal is a start of the answer without a stop, blank or trailing space, and
/// on one line when the caret's line goes on; Copilot's proposals each change the text, between characters of it.
pub fn answer(bytes: &[u8]) {
    let input = String::from_utf8_lossy(bytes);
    let split = input
        .char_indices()
        .nth(input.chars().count() / 2)
        .map_or(0, |(i, _)| i);
    let (text, suffix) = input.split_at(split);
    let caret = text.len();
    if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
        for read in [crate::fim::answer::ollama, crate::fim::answer::lmstudio] {
            if let Ok(proposal) = read(&value) {
                cut(&proposal, suffix);
            }
        }
        for encoding in [Encoding::Utf8, Encoding::Utf16, Encoding::Utf32] {
            if let Ok(found) = crate::copilot::answer::completions(&value, &input, caret, encoding) {
                changes(&input, &found);
            }
            if let Ok(found) = crate::copilot::answer::edits(&value, &input, "file:///a", 1, encoding) {
                changes(&input, &found);
            }
        }
    }
    cut(text, suffix);
}

/// Cuts `proposal` for the gap before `suffix` and checks what is left.
fn cut(proposal: &str, suffix: &str) {
    let stops = Family::QwenCoder.stops();
    let Some(kept) = crate::fim::answer::clean(proposal, suffix, stops) else {
        return;
    };
    assert!(proposal.starts_with(&kept), "{kept:?} is no start of {proposal:?}");
    assert!(!stops.iter().any(|stop| kept.contains(stop)), "{kept:?} keeps a stop");
    assert!(!kept.trim().is_empty() && kept.trim_end() == kept, "{kept:?}");
    if !suffix.split('\n').next().unwrap_or_default().trim().is_empty() {
        assert!(!kept.contains('\n'), "{kept:?} wraps in the middle of a line");
    }
}

/// Checks that each of `found` lies between characters of `text`, changes it, and is there only once.
fn changes(text: &str, found: &[Prediction]) {
    for (n, prediction) in found.iter().enumerate() {
        let range = prediction.range.clone();
        assert!(range.start <= range.end && range.end <= text.len(), "{range:?}");
        assert!(
            text.is_char_boundary(range.start) && text.is_char_boundary(range.end),
            "{range:?}"
        );
        assert_ne!(&text[range.clone()], prediction.text, "{prediction:?} changes nothing");
        let twice = found[..n]
            .iter()
            .any(|other| other.range == range && other.text == prediction.text);
        assert!(!twice, "{prediction:?} twice");
    }
}
