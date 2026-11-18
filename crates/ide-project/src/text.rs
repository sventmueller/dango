//! Positions in text as AppKit counts them, in UTF-16 code units.

use std::ops::Range;

/// The UTF-16 columns of ascending byte `offsets` into `line`, which may hold invalid UTF-8: each run of invalid bytes
/// counts as the one replacement character a lossy conversion puts there.
pub(crate) fn columns(line: &[u8], offsets: &[usize]) -> Vec<usize> {
    let mut found = Vec::with_capacity(offsets.len());
    let mut next = 0;
    let mut byte = 0;
    let mut column = 0;
    let mut unit = |len: usize, width: usize, found: &mut Vec<usize>| {
        while next < offsets.len() && offsets[next] < byte + len {
            found.push(column);
            next += 1;
        }
        byte += len;
        column += width;
    };
    for chunk in line.utf8_chunks() {
        for c in chunk.valid().chars() {
            unit(c.len_utf8(), c.len_utf16(), &mut found);
        }
        if !chunk.invalid().is_empty() {
            unit(chunk.invalid().len(), 1, &mut found);
        }
    }
    found.resize(offsets.len(), column);
    found
}

/// The UTF-16 ranges covering the characters at the ascending `positions` of `text`, neighbours merged.
pub(crate) fn char_ranges(text: &str, positions: &[usize]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut wanted = positions.iter().peekable();
    let mut column = 0;
    for (index, c) in text.chars().enumerate() {
        let Some(&&position) = wanted.peek() else { break };
        let width = c.len_utf16();
        if position == index {
            wanted.next();
            match ranges.last_mut() {
                Some(last) if last.end == column => last.end += width,
                _ => ranges.push(column..column + width),
            }
        }
        column += width;
    }
    ranges
}

/// The byte index in `text` where the UTF-16 `column` falls, moved back to the start of a character it splits.
pub(crate) fn byte_at(text: &str, column: usize) -> usize {
    let mut seen = 0;
    for (index, c) in text.char_indices() {
        if seen + c.len_utf16() > column {
            return index;
        }
        seen += c.len_utf16();
    }
    text.len()
}

/// The length of `text` in UTF-16 code units.
pub(crate) fn width(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_count_astral_characters_twice_and_invalid_runs_once() {
        let line = "a😀b".as_bytes();
        assert_eq!(columns(line, &[0, 1, 5, 6]), [0, 1, 3, 4]);
        let broken = b"x\xe2\x82y";
        assert_eq!(columns(broken, &[0, 1, 3, 4]), [0, 1, 2, 3]);
        let shown = String::from_utf8_lossy(broken);
        assert_eq!(width(&shown), 3);
    }

    #[test]
    fn neighbouring_characters_merge_into_one_range() {
        assert_eq!(char_ranges("ab😀cd", &[0, 1, 2, 4]), [0..4, 5..6]);
    }

    #[test]
    fn a_column_inside_a_surrogate_pair_falls_back_to_the_character() {
        let text = "a😀b";
        assert_eq!(byte_at(text, 2), 1);
        assert_eq!(byte_at(text, 3), 5);
        assert_eq!(byte_at(text, 9), text.len());
    }
}
