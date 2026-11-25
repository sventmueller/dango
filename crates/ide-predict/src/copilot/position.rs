//! Byte offsets of the request's text and the line and column of the protocol, in the unit the server counts in.

use ide_lsp::Encoding;
use ide_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent};

fn units(text: &str, encoding: Encoding) -> u32 {
    let count = match encoding {
        Encoding::Utf8 => text.len(),
        Encoding::Utf16 => text.encode_utf16().count(),
        Encoding::Utf32 => text.chars().count(),
    };
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn width(c: char, encoding: Encoding) -> usize {
    match encoding {
        Encoding::Utf8 => c.len_utf8(),
        Encoding::Utf16 => c.len_utf16(),
        Encoding::Utf32 => 1,
    }
}

/// The position of byte `offset` of `text`, which must be on a character boundary.
pub(crate) fn position(text: &str, offset: usize, encoding: Encoding) -> Position {
    let before = &text[..offset.min(text.len())];
    let line = u32::try_from(before.matches('\n').count()).unwrap_or(u32::MAX);
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    Position::new(line, units(&before[start..], encoding))
}

/// The byte offset of `position` in `text`; a column past the end of its line means the line's end, a line past
/// the text's end means none.
pub(crate) fn offset(text: &str, position: Position, encoding: Encoding) -> Option<usize> {
    let mut start = 0;
    for _ in 0..position.line {
        start += text[start..].find('\n')? + 1;
    }
    let line = text[start..].split('\n').next().unwrap_or_default();
    let wanted = usize::try_from(position.character).unwrap_or(usize::MAX);
    let mut counted = 0;
    for (i, c) in line.char_indices() {
        if counted >= wanted {
            return Some(start + i);
        }
        counted += width(c, encoding);
    }
    Some(start + line.len())
}

/// The range of `start..end` of `text`.
pub(crate) fn range(text: &str, start: usize, end: usize, encoding: Encoding) -> Range {
    Range::new(position(text, start, encoding), position(text, end, encoding))
}

/// The one change that turns `old` into `new`: the part between what both start and end with, with its range in
/// UTF-16 of `old`, as `ide_lsp::Server::change` takes it.
pub(crate) fn change(old: &str, new: &str) -> TextDocumentContentChangeEvent {
    let (start, old_end, new_end) = differing(old, new);
    TextDocumentContentChangeEvent {
        range: Some(range(old, start, old_end, Encoding::Utf16)),
        range_length: None,
        text: new[start..new_end].to_string(),
    }
}

/// Where `a` and `b` differ: the common prefix's length, and the ends of the differing parts in `a` and in `b`, all
/// on character boundaries.
pub(crate) fn differing(a: &str, b: &str) -> (usize, usize, usize) {
    let mut start = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    while !a.is_char_boundary(start) || !b.is_char_boundary(start) {
        start -= 1;
    }
    let room = a.len().min(b.len()) - start;
    let mut tail = a
        .bytes()
        .rev()
        .zip(b.bytes().rev())
        .take(room)
        .take_while(|(x, y)| x == y)
        .count();
    while !a.is_char_boundary(a.len() - tail) || !b.is_char_boundary(b.len() - tail) {
        tail -= 1;
    }
    (start, a.len() - tail, b.len() - tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_and_offsets_agree_in_every_unit() {
        let text = "fn ä() {\n    \"😀\" + x\n}";
        for encoding in [Encoding::Utf8, Encoding::Utf16, Encoding::Utf32] {
            for (offset, _) in text.char_indices().chain([(text.len(), ' ')]) {
                let at = position(text, offset, encoding);
                assert_eq!(offset_of(text, at, encoding), offset, "{encoding:?} {offset}");
            }
        }
        let x = text.find('x').unwrap();
        assert_eq!(position(text, x, Encoding::Utf16), Position::new(1, 11));
        assert_eq!(position(text, x, Encoding::Utf8), Position::new(1, 13));
        assert_eq!(position(text, x, Encoding::Utf32), Position::new(1, 10));
        assert_eq!(
            offset(text, Position::new(0, 99), Encoding::Utf16),
            Some("fn ä() {".len())
        );
        assert_eq!(offset(text, Position::new(5, 0), Encoding::Utf16), None);
    }

    fn offset_of(text: &str, at: Position, encoding: Encoding) -> usize {
        offset(text, at, encoding).unwrap()
    }

    #[test]
    fn one_change_covers_what_differs() {
        let change = change("let a = 1;\nlet b = 2;\n", "let a = 1;\nlet bc = 2;\n");
        assert_eq!(change.range, Some(Range::new(Position::new(1, 5), Position::new(1, 5))));
        assert_eq!(change.text, "c");
        assert_eq!(differing("aaa", "aa"), (2, 3, 2));
        assert_eq!(differing("x😀y", "x😁y"), (1, 5, 5));
        assert_eq!(differing("same", "same"), (4, 4, 4));
    }
}
