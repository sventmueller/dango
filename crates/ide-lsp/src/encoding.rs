//! Columns within a line, counted in UTF-16 code units as the editor does or in the unit a server chose.

use lsp_types::PositionEncodingKind;

/// The unit a server counts columns in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Encoding {
    /// Bytes of UTF-8.
    Utf8,
    /// UTF-16 code units, the protocol's default and the editor's own unit.
    #[default]
    Utf16,
    /// Unicode scalar values.
    Utf32,
}

impl Encoding {
    /// The encoding a server announced; none or an unknown one means UTF-16, which every server must support.
    pub fn from_kind(kind: Option<&PositionEncodingKind>) -> Encoding {
        match kind.map(PositionEncodingKind::as_str) {
            Some("utf-8") => Encoding::Utf8,
            Some("utf-32") => Encoding::Utf32,
            _ => Encoding::Utf16,
        }
    }

    /// The protocol's name for it.
    pub fn kind(self) -> PositionEncodingKind {
        match self {
            Encoding::Utf8 => PositionEncodingKind::UTF8,
            Encoding::Utf16 => PositionEncodingKind::UTF16,
            Encoding::Utf32 => PositionEncodingKind::UTF32,
        }
    }
}

/// The byte offset in `line` at `column`, clamped to the line; a column inside a character means its start.
pub(crate) fn byte_at(line: &str, column: u32, encoding: Encoding) -> usize {
    let column = usize::try_from(column).unwrap_or(usize::MAX);
    match encoding {
        Encoding::Utf8 => {
            let mut byte = column.min(line.len());
            while !line.is_char_boundary(byte) {
                byte -= 1;
            }
            byte
        }
        Encoding::Utf16 => {
            let mut units = 0;
            for (byte, c) in line.char_indices() {
                if units >= column {
                    return byte;
                }
                units += c.len_utf16();
                if units > column {
                    return byte;
                }
            }
            line.len()
        }
        Encoding::Utf32 => line.char_indices().nth(column).map_or(line.len(), |(byte, _)| byte),
    }
}

/// The column of byte offset `byte` in `line`, counted in `encoding`.
pub(crate) fn column_at(line: &str, byte: usize, encoding: Encoding) -> u32 {
    let mut byte = byte.min(line.len());
    while !line.is_char_boundary(byte) {
        byte -= 1;
    }
    let prefix = &line[..byte];
    let count = match encoding {
        Encoding::Utf8 => prefix.len(),
        Encoding::Utf16 => prefix.encode_utf16().count(),
        Encoding::Utf32 => prefix.chars().count(),
    };
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// `column` of `line` counted in `from`, recounted in `to`.
pub(crate) fn convert(line: &str, column: u32, from: Encoding, to: Encoding) -> u32 {
    if from == to {
        return column;
    }
    column_at(line, byte_at(line, column, from), to)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "aä😀b";

    #[test]
    fn columns_are_recounted_between_units() {
        // a = 1 byte, ä = 2 bytes, 😀 = 4 bytes and two UTF-16 units, b = 1 byte.
        assert_eq!(convert(LINE, 4, Encoding::Utf16, Encoding::Utf8), 7);
        assert_eq!(convert(LINE, 7, Encoding::Utf8, Encoding::Utf16), 4);
        assert_eq!(convert(LINE, 3, Encoding::Utf32, Encoding::Utf16), 4);
        assert_eq!(convert(LINE, 5, Encoding::Utf16, Encoding::Utf32), 4);
    }

    #[test]
    fn a_column_inside_a_character_means_its_start_and_past_the_end_means_the_end() {
        assert_eq!(byte_at(LINE, 3, Encoding::Utf16), 3, "between the halves of the emoji");
        assert_eq!(byte_at(LINE, 2, Encoding::Utf8), 1, "inside ä");
        assert_eq!(byte_at(LINE, 99, Encoding::Utf16), LINE.len());
        assert_eq!(byte_at(LINE, 99, Encoding::Utf32), LINE.len());
        assert_eq!(column_at(LINE, 99, Encoding::Utf16), 5);
    }

    #[test]
    fn unknown_encodings_fall_back_to_utf16() {
        assert_eq!(Encoding::from_kind(None), Encoding::Utf16);
        assert_eq!(
            Encoding::from_kind(Some(&PositionEncodingKind::new("utf-7"))),
            Encoding::Utf16
        );
        assert_eq!(Encoding::from_kind(Some(&Encoding::Utf8.kind())), Encoding::Utf8);
    }
}
