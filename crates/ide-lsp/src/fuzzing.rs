//! Entry points for the fuzz targets in `fuzz/`.

use lsp_types::{ApplyWorkspaceEditParams, Position, PublishDiagnosticsParams, Range};
use serde::Deserialize;
use serde_json::Value;

use crate::encoding::Encoding;
use crate::text::Text;

const ENCODINGS: [Encoding; 3] = [Encoding::Utf8, Encoding::Utf16, Encoding::Utf32];

/// Reads `bytes` as what a server writes: framed messages, the first of which stands for an open document's text.
/// Every range and position in the later ones is recounted, moved onto the text and edited into it in each of the
/// protocol's units, as answers, diagnostics and workspace edits are.
pub fn messages(bytes: &[u8]) {
    let mut reader = bytes;
    let mut document: Option<Text> = None;
    while let Ok(Some(body)) = ide_process::framing::read(&mut reader) {
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
                    let _ = text.clamp(text.convert(range.end, encoding, Encoding::Utf16));
                    text.replace(range, "x\r\n", encoding);
                }
            } else if let Ok(position) = Position::deserialize(value) {
                for encoding in ENCODINGS {
                    let _ = text.convert(position, Encoding::Utf16, encoding);
                }
            }
        });
    }
}

fn each_value(value: &Value, f: &mut impl FnMut(&Value)) {
    f(value);
    match value {
        Value::Array(items) => items.iter().for_each(|item| each_value(item, f)),
        Value::Object(fields) => fields.values().for_each(|item| each_value(item, f)),
        _ => {}
    }
}
