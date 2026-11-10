//! Folds by indentation: a line folds the lines below it that are indented deeper.

use crate::text::Text;

/// Lines that fold together: `start` stays visible, the lines after it up to `end` hide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fold {
    /// The line that stays visible.
    pub start: usize,
    /// The last line that hides.
    pub end: usize,
}

/// The folds of `text`, ordered by their first line, with a tab reaching the next multiple of `tab_width`. Blank lines
/// belong to the fold around them, but no fold ends in one.
pub(crate) fn indentation_folds(text: &Text, tab_width: usize) -> Vec<Fold> {
    let tab_width = tab_width.max(1);
    let mut folds = Vec::new();
    let mut open: Vec<(usize, usize)> = Vec::new();
    let mut last_content = 0;
    for (n, line) in text.rope().lines().enumerate() {
        let Some(indent) = indentation(line.chars(), tab_width) else {
            continue;
        };
        while let Some(&(depth, start)) = open.last()
            && depth >= indent
        {
            open.pop();
            if last_content > start {
                folds.push(Fold {
                    start,
                    end: last_content,
                });
            }
        }
        open.push((indent, n));
        last_content = n;
    }
    for (_, start) in open.into_iter().rev() {
        if last_content > start {
            folds.push(Fold {
                start,
                end: last_content,
            });
        }
    }
    folds.sort_by_key(|fold| fold.start);
    folds
}

/// The column the content of a line starts at, or `None` for a line of nothing but blanks.
fn indentation(chars: impl Iterator<Item = char>, tab_width: usize) -> Option<usize> {
    let mut column = 0;
    for ch in chars {
        match ch {
            ' ' => column += 1,
            '\t' => column += tab_width - column % tab_width,
            '\r' | '\n' => return None,
            _ => return Some(column),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folds(text: &str) -> Vec<(usize, usize)> {
        indentation_folds(&Text::new(text), 4)
            .into_iter()
            .map(|f| (f.start, f.end))
            .collect()
    }

    #[test]
    fn deeper_lines_fold_into_the_line_above_them() {
        let text = "fn a() {\n    if x {\n        y();\n    }\n\n    z();\n}\nfn b() {}\n";
        assert_eq!(folds(text), [(0, 5), (1, 2)]);
    }

    #[test]
    fn trailing_blank_lines_stay_outside_and_tabs_reach_the_next_stop() {
        assert_eq!(folds("a:\n\tb\n  \n\nc\n"), [(0, 1)]);
        assert_eq!(
            folds("a\n \tb\n    c\n"),
            [(0, 2)],
            "a blank and a tab reach the column of four blanks"
        );
        assert_eq!(
            folds("x\n  y\n    z"),
            [(0, 2), (1, 2)],
            "folds still open at the end close there"
        );
        assert!(folds("").is_empty() && folds("\n\n").is_empty());
    }
}
