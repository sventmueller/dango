//! The text of an open document as the server was told it: to recount positions and to reopen it after a restart.

use lsp_types::{Position, Range};

use crate::encoding::{self, Encoding};

/// A document's text as lines that keep their line break, so edits touch only the lines they change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Text {
    /// Never empty; every line but the last ends with `\n`, `\r\n` or `\r`, the protocol's line breaks.
    lines: Vec<String>,
}

impl Text {
    pub(crate) fn new(text: &str) -> Text {
        Text { lines: split(text) }
    }

    /// Line `n` without its line break.
    pub(crate) fn line(&self, n: u32) -> Option<&str> {
        self.lines.get(usize::try_from(n).ok()?).map(|line| content(line))
    }

    pub(crate) fn text(&self) -> String {
        self.lines.concat()
    }

    /// `position`, counted in `from`, recounted in `to`.
    pub(crate) fn convert(&self, position: Position, from: Encoding, to: Encoding) -> Position {
        let character = self.line(position.line).map_or(position.character, |line| {
            encoding::convert(line, position.character, from, to)
        });
        Position::new(position.line, character)
    }

    /// `position`, in UTF-16, moved onto the text: past a line means its end, past the text the text's end.
    pub(crate) fn clamp(&self, position: Position) -> Position {
        let (line, byte) = self.locate(position, Encoding::Utf16);
        Position::new(
            u32::try_from(line).unwrap_or(u32::MAX),
            encoding::column_at(content(&self.lines[line]), byte, Encoding::Utf16),
        )
    }

    /// Replaces `range`, counted in `encoding`, with `new`; positions past a line or the text mean its end.
    pub(crate) fn replace(&mut self, range: Range, new: &str, encoding: Encoding) {
        let (start, end) = if (range.end.line, range.end.character) < (range.start.line, range.start.character) {
            (range.end, range.start)
        } else {
            (range.start, range.end)
        };
        let last = self.lines.len() - 1;
        let (a, a_byte) = self.locate(start, encoding);
        let (b, b_byte) = self.locate(end, encoding);
        // One line of context on each side: an edit can join a lone `\r` with a `\n` into one line break.
        let first = a.saturating_sub(1);
        let past = (b + 1).min(last);
        let mut joined = String::new();
        let (mut from, mut to) = (0, 0);
        for i in first..=past {
            if i == a {
                from = joined.len() + a_byte;
            }
            if i == b {
                to = joined.len() + b_byte;
            }
            joined.push_str(&self.lines[i]);
        }
        joined.replace_range(from..to, new);
        let mut pieces = split(&joined);
        if past < last {
            pieces.pop();
        }
        self.lines.splice(first..=past, pieces);
    }

    /// The line index and byte offset within it of `position`.
    fn locate(&self, position: Position, encoding: Encoding) -> (usize, usize) {
        let last = self.lines.len() - 1;
        match usize::try_from(position.line) {
            Ok(n) if n <= last => (
                n,
                encoding::byte_at(content(&self.lines[n]), position.character, encoding),
            ),
            _ => (last, content(&self.lines[last]).len()),
        }
    }
}

fn split(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let end = match bytes[i] {
            b'\n' => i + 1,
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => i + 2,
            b'\r' => i + 1,
            _ => {
                i += 1;
                continue;
            }
        };
        lines.push(text[start..end].to_string());
        start = end;
        i = end;
    }
    lines.push(text[start..].to_string());
    lines
}

fn content(line: &str) -> &str {
    line.strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .or_else(|| line.strip_suffix('\r'))
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(a: (u32, u32), b: (u32, u32)) -> Range {
        Range::new(Position::new(a.0, a.1), Position::new(b.0, b.1))
    }

    fn edited(text: &str, r: Range, new: &str, encoding: Encoding) -> String {
        let mut t = Text::new(text);
        t.replace(r, new, encoding);
        assert_eq!(t, Text::new(&t.text()), "the lines are split as a fresh text would be");
        t.text()
    }

    #[test]
    fn edits_within_and_across_lines_land_where_utf16_says() {
        let text = "fn 😀() {\n    x\n}\n";
        assert_eq!(
            edited(text, range((0, 3), (0, 5)), "go", Encoding::Utf16),
            "fn go() {\n    x\n}\n"
        );
        assert_eq!(edited(text, range((0, 8), (2, 0)), "", Encoding::Utf16), "fn 😀() }\n");
        assert_eq!(
            edited(text, range((1, 4), (1, 5)), "a\nb", Encoding::Utf16),
            "fn 😀() {\n    a\nb\n}\n"
        );
        assert_eq!(
            edited(text, range((0, 9), (0, 9)), "x", Encoding::Utf8),
            "fn 😀()x {\n    x\n}\n"
        );
    }

    #[test]
    fn line_breaks_of_every_kind_are_kept_and_can_merge() {
        let text = "a\r\nb\rc\nd";
        assert_eq!(Text::new(text).line(1), Some("b"));
        assert_eq!(Text::new(text).text(), text);
        assert_eq!(edited("a\rb", range((1, 0), (1, 0)), "\n", Encoding::Utf16), "a\r\nb");
        assert_eq!(edited("a\r\nb", range((0, 1), (1, 0)), "", Encoding::Utf16), "ab");
    }

    #[test]
    fn positions_past_the_end_mean_the_end_and_reversed_ranges_are_put_in_order() {
        assert_eq!(edited("ab\ncd", range((0, 9), (9, 9)), "!", Encoding::Utf16), "ab!");
        assert_eq!(edited("ab\ncd", range((1, 1), (0, 1)), "", Encoding::Utf16), "ad");
        assert_eq!(edited("", range((0, 0), (0, 0)), "x\n", Encoding::Utf16), "x\n");
        assert_eq!(edited("x\n", range((1, 0), (1, 0)), "y", Encoding::Utf16), "x\ny");
    }

    #[test]
    fn a_position_is_recounted_on_its_own_line() {
        let text = Text::new("ä\n😀x");
        let p = text.convert(Position::new(1, 2), Encoding::Utf16, Encoding::Utf8);
        assert_eq!(p, Position::new(1, 4));
        assert_eq!(text.convert(p, Encoding::Utf8, Encoding::Utf16), Position::new(1, 2));
        assert_eq!(
            text.convert(Position::new(7, 3), Encoding::Utf16, Encoding::Utf8),
            Position::new(7, 3)
        );
    }
}
