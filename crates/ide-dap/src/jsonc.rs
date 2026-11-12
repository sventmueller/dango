//! JSON with comments and trailing commas, as `.vscode/launch.json` is written.

/// `text` as plain JSON: comments become spaces and trailing commas go, outside strings only, so positions in error
/// messages still match the file.
pub(crate) fn strip(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i = (i + 1).min(bytes.len());
                out.extend_from_slice(&bytes[start..i]);
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out.push(b' ');
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let end = text[i + 2..].find("*/").map_or(bytes.len(), |n| i + 2 + n + 2);
                out.extend(bytes[i..end].iter().map(|&b| if b == b'\n' { b'\n' } else { b' ' }));
                i = end;
            }
            b',' => {
                let next = bytes[i + 1..]
                    .iter()
                    .position(|b| !b.is_ascii_whitespace())
                    .map(|n| bytes[i + 1 + n]);
                // A comma before a comment that ends the list is caught on a second pass.
                out.push(if matches!(next, Some(b'}' | b']')) { b' ' } else { b',' });
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    let once = String::from_utf8(out).unwrap_or_default();
    if once == text { once } else { strip(&once) }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn comments_and_trailing_commas_go_but_strings_keep_them() {
        let text = r#"{
            // the configurations
            "version": "0.2.0", /* inline */
            "configurations": [
                {"url": "http://localhost:3000//x", "glob": "/*.js", "q": "a\"//b",},
                // last,
            ],
        }"#;
        let value: Value = serde_json::from_str(&strip(text)).unwrap();
        assert_eq!(
            value,
            json!({"version": "0.2.0", "configurations": [{"url": "http://localhost:3000//x", "glob": "/*.js", "q": "a\"//b"}]})
        );
    }

    #[test]
    fn broken_input_does_not_panic() {
        for text in ["\"", "/*", "{,", "\"\\", "//", ",", "[1,/*", "ü,]"] {
            let _ = strip(text);
        }
    }
}
