//! Entry points for the fuzz targets in `fuzz/`.

use ide_lsp::Encoding;
use serde_json::Value;

use crate::fim::Family;

/// Reads `bytes` as a local model's or Copilot's answer for a gap in a text made of the same bytes, and cuts the
/// proposal as the providers do.
pub fn answer(bytes: &[u8]) {
    let input = String::from_utf8_lossy(bytes);
    let split = input
        .char_indices()
        .nth(input.chars().count() / 2)
        .map_or(0, |(i, _)| i);
    let (text, suffix) = input.split_at(split);
    let caret = text.len();
    let stops = Family::QwenCoder.stops();
    if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
        for read in [crate::fim::answer::ollama, crate::fim::answer::lmstudio] {
            if let Ok(proposal) = read(&value) {
                let _ = crate::fim::answer::clean(&proposal, suffix, stops);
            }
        }
        for encoding in [Encoding::Utf8, Encoding::Utf16, Encoding::Utf32] {
            let _ = crate::copilot::answer::completions(&value, &input, caret, encoding);
            let _ = crate::copilot::answer::edits(&value, &input, "file:///a", 1, encoding);
        }
    }
    let _ = crate::fim::answer::clean(text, suffix, stops);
}
