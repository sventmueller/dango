//! Between the document model's UTF-16 offsets and the protocol's lines and UTF-16 columns.

use std::ops::Range;

use ide_document::{Change, Text, Unit};
use lsp_types::{Position, Range as LspRange};

fn column(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The range an edit replaced, as `textDocument/didChange` wants it.
pub fn change_range(change: &Change) -> LspRange {
    LspRange::new(
        Position::new(column(change.start.line), column(change.start.utf16)),
        Position::new(column(change.old_end.line), column(change.old_end.utf16)),
    )
}

/// The position of UTF-16 offset `offset`.
pub fn position(text: &Text, offset: usize) -> Option<Position> {
    let byte = text.utf16_to_byte(offset.min(text.len_utf16())).ok()?;
    let point = text.point(byte).ok()?;
    Some(Position::new(column(point.line), column(point.utf16)))
}

/// The UTF-16 offset of `position`, read as the protocol asks when it lies beyond a line or the text.
pub fn offset(text: &Text, position: Position) -> usize {
    let byte = text.lsp_offset(position.line, position.character, Unit::Utf16);
    text.byte_to_utf16(byte).unwrap_or_else(|_| text.len_utf16())
}

/// The UTF-16 offsets of `range`.
pub fn range(text: &Text, range: LspRange) -> Range<usize> {
    let start = offset(text, range.start);
    start..offset(text, range.end).max(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_and_offsets_agree_in_utf16() {
        let text = Text::new("a😀b\nzweite");
        assert_eq!(position(&text, 3), Some(Position::new(0, 3)));
        assert_eq!(offset(&text, Position::new(0, 3)), 3);
        assert_eq!(offset(&text, Position::new(1, 2)), 7);
        assert_eq!(
            offset(&text, Position::new(9, 0)),
            text.len_utf16(),
            "past the text is its end"
        );
        assert_eq!(
            range(&text, LspRange::new(Position::new(1, 0), Position::new(1, 99))),
            5..11
        );
    }
}
