//! Entry points for the fuzz targets in `fuzz/`.

use lsp_types::{ApplyWorkspaceEditParams, Position, PublishDiagnosticsParams, Range};
use serde::Deserialize;
use serde_json::Value;

use crate::encoding::Encoding;
use crate::text::Text;

const ENCODINGS: [Encoding; 3] = [Encoding::Utf8, Encoding::Utf16, Encoding::Utf32];

const INSERTED: &str = "x\r\n";

/// Reads `bytes` as what a server writes: framed messages, the first of which stands for an open document's text.
/// Every range and position in the later ones is recounted, moved onto the text and edited into it in each of the
/// protocol's units, as answers, diagnostics and workspace edits are. Each body framed again reads back as itself,
/// a position on a character reads back in every unit, and every edit makes the text a string edit would.
pub fn messages(bytes: &[u8]) {
    let mut reader = bytes;
    let mut document: Option<Text> = None;
    while let Ok(Some(body)) = ide_process::framing::read(&mut reader) {
        let framed = ide_process::framing::frame(&body);
        assert_eq!(
            ide_process::framing::read(&mut framed.as_slice()).ok().flatten(),
            Some(body.clone())
        );
        let Some(text) = document.as_mut() else {
            document = Some(Text::new(&String::from_utf8_lossy(&body)));
            continue;
        };
        let Ok(message) = serde_json::from_slice::<Value>(&body) else {
            continue;
        };
        if let Some(params) = message.get("params") {
            let _ = PublishDiagnosticsParams::deserialize(params);
            let _ = ApplyWorkspaceEditParams::deserialize(params);
        }
        each_value(&message, &mut |value| {
            if let Ok(range) = Range::deserialize(value) {
                for encoding in ENCODINGS {
                    edit(text, range, encoding);
                }
            } else if let Ok(position) = Position::deserialize(value) {
                for encoding in ENCODINGS {
                    reads_back(text, position, encoding);
                }
            }
        });
    }
}

/// Replaces `range` with [`INSERTED`] and compares the result with the same edit made on the text as a string.
fn edit(text: &mut Text, range: Range, encoding: Encoding) {
    let before = text.text();
    let (start, end) = if (range.end.line, range.end.character) < (range.start.line, range.start.character) {
        (range.end, range.start)
    } else {
        (range.start, range.end)
    };
    let at = |position| offset(&before, text.clamp(text.convert(position, encoding, Encoding::Utf16)));
    let from = at(start);
    let to = at(end).max(from);
    let expected = format!("{}{INSERTED}{}", &before[..from], &before[to..]);
    text.replace(range, INSERTED, encoding);
    assert_eq!(text.text(), expected, "{range:?} in {encoding:?}");
}

/// Checks that `position`, once on the text, is the same place counted in each unit.
fn reads_back(text: &Text, position: Position, encoding: Encoding) {
    let place = text.clamp(text.convert(position, encoding, Encoding::Utf16));
    for unit in ENCODINGS {
        let there = text.convert(place, Encoding::Utf16, unit);
        assert_eq!(
            text.convert(there, unit, Encoding::Utf16),
            place,
            "{position:?} in {unit:?}"
        );
    }
}

/// The byte offset in `text` of `position`, a character boundary on one of its lines counted in UTF-16, with lines
/// ending at `\n`, `\r\n` and `\r` as the protocol says.
fn offset(text: &str, position: Position) -> usize {
    let bytes = text.as_bytes();
    let mut start = 0;
    for _ in 0..position.line {
        let Some(at) = bytes[start..].iter().position(|&b| b == b'\n' || b == b'\r') else {
            return text.len();
        };
        let at = start + at;
        start = at + if bytes[at..].starts_with(b"\r\n") { 2 } else { 1 };
    }
    let mut units = 0;
    for (byte, c) in text[start..].char_indices() {
        if units >= position.character || c == '\n' || c == '\r' {
            return start + byte;
        }
        units += u32::try_from(c.len_utf16()).unwrap_or(2);
    }
    text.len()
}

fn each_value(value: &Value, f: &mut impl FnMut(&Value)) {
    f(value);
    match value {
        Value::Array(items) => items.iter().for_each(|item| each_value(item, f)),
        Value::Object(fields) => fields.values().for_each(|item| each_value(item, f)),
        _ => {}
    }
}
