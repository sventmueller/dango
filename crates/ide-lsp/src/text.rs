//! The text of an open document as the server was told it: to recount positions and to reopen it after a restart.

use std::borrow::Cow;

use lsp_types::{Position, Range};
use ropey::Rope;

use crate::encoding::{self, Encoding};

/// A document's text as a rope: an edit touches only the pieces it changes, and a copy shares them all, so an answer
/// being recounted can hold the text while the next keystroke edits it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Text {
    /// Its line breaks are `\n`, `\r\n` and `\r`, the protocol's three: ropey with `cr_lines` and without
    /// `unicode_lines`.
    rope: Rope,
}

impl Text {
    pub(crate) fn new(text: &str) -> Text {
        Text {
            rope: Rope::from_str(text),
        }
    }

    /// Line `n` without its line break.
    pub(crate) fn line(&self, n: u32) -> Option<Cow<'_, str>> {
        self.content(usize::try_from(n).ok()?)
    }

    pub(crate) fn text(&self) -> String {
        self.rope.to_string()
    }

    /// `position`, counted in `from`, recounted in `to`.
    pub(crate) fn convert(&self, position: Position, from: Encoding, to: Encoding) -> Position {
        let character = self.line(position.line).map_or(position.character, |line| {
            encoding::convert(&line, position.character, from, to)
        });
        Position::new(position.line, character)
    }

    /// `position`, in UTF-16, moved onto the text: past a line means its end, past the text the text's end.
    pub(crate) fn clamp(&self, position: Position) -> Position {
        let (line, content, byte) = self.locate(position, Encoding::Utf16);
        Position::new(
            u32::try_from(line).unwrap_or(u32::MAX),
            encoding::column_at(&content, byte, Encoding::Utf16),
        )
    }

    /// Replaces `range`, counted in `encoding`, with `new`; positions past a line or the text mean its end.
    pub(crate) fn replace(&mut self, range: Range, new: &str, encoding: Encoding) {
        let (start, end) = if (range.end.line, range.end.character) < (range.start.line, range.start.character) {
            (range.end, range.start)
        } else {
            (range.start, range.end)
        };
        let from = self.char_at(start, encoding);
        let to = self.char_at(end, encoding).max(from);
        self.rope.remove(from..to);
        self.rope.insert(from, new);
    }

    /// The character index of `position`.
    fn char_at(&self, position: Position, encoding: Encoding) -> usize {
        let (line, _, byte) = self.locate(position, encoding);
        self.rope.byte_to_char(self.rope.line_to_byte(line) + byte)
    }

    /// The line index, its content and the byte offset within that of `position`.
    fn locate(&self, position: Position, encoding: Encoding) -> (usize, Cow<'_, str>, usize) {
        let last = self.rope.len_lines() - 1;
        match usize::try_from(position.line) {
            Ok(n) if n <= last => {
                let content = self.content(n).unwrap_or_default();
                let byte = encoding::byte_at(&content, position.character, encoding);
                (n, content, byte)
            }
            _ => {
                let content = self.content(last).unwrap_or_default();
                let byte = content.len();
                (last, content, byte)
            }
        }
    }

    fn content(&self, n: usize) -> Option<Cow<'_, str>> {
        let line = Cow::from(self.rope.get_line(n)?);
        let kept = strip_break(&line).len();
        Some(match line {
            Cow::Borrowed(line) => Cow::Borrowed(&line[..kept]),
            Cow::Owned(mut line) => {
                line.truncate(kept);
                Cow::Owned(line)
            }
        })
    }
}

fn strip_break(line: &str) -> &str {
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
        assert_eq!(Text::new(text).line(1).as_deref(), Some("b"));
        assert_eq!(Text::new(text).line(3).as_deref(), Some("d"));
        assert_eq!(Text::new(text).text(), text);
        assert_eq!(Text::new("a\u{2028}b").line(0).as_deref(), Some("a\u{2028}b"));
        assert_eq!(edited("a\rb", range((1, 0), (1, 0)), "\n", Encoding::Utf16), "a\r\nb");
        assert_eq!(edited("a\r\nb", range((0, 1), (1, 0)), "", Encoding::Utf16), "ab");
        let mut joined = Text::new("a\rb");
        joined.replace(range((1, 0), (1, 0)), "\n", Encoding::Utf16);
        assert_eq!(joined.line(1).as_deref(), Some("b"), "the joined break is one");
    }

    #[test]
    fn a_line_across_the_rope_s_pieces_reads_whole() {
        let long = format!("{}\n{}\n", "a".repeat(5000), "b".repeat(3));
        let text = Text::new(&long);
        assert_eq!(text.line(0).map(|l| l.len()), Some(5000));
        assert_eq!(text.line(1).as_deref(), Some("bbb"));
        assert_eq!(text.line(2).as_deref(), Some(""));
        assert_eq!(text.line(3), None);
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
