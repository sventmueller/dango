//! Entry points for the fuzz targets in `fuzz/`.

use crate::diff::{Change, Hunk, LineKind};

/// Reads `bytes` as the output of `git diff`. The line numbers of every hunk count up from its starts without
/// passing its counts, and the patches staging writes of a file's hunks, forwards and backwards, read back as them.
pub fn diff(bytes: &[u8]) {
    let Ok(files) = crate::diff::parse(bytes) else {
        return;
    };
    for file in &files {
        for hunk in &file.hunks {
            numbered(hunk);
        }
        if file.binary || file.change == Change::Unmerged || file.hunks.is_empty() {
            continue;
        }
        let hunks: Vec<&Hunk> = file.hunks.iter().collect();
        for reverse in [false, true] {
            let Ok(patch) = crate::staging::patch(file, &hunks, reverse) else {
                continue;
            };
            let read = crate::diff::parse(&patch).expect("a patch staging wrote reads");
            let [again] = read.as_slice() else {
                panic!("{} files in the patch of one", read.len());
            };
            assert_eq!(again.path(), file.path());
            assert_eq!(again.hunks.len(), file.hunks.len());
            for (hunk, read) in file.hunks.iter().zip(&again.hunks) {
                assert_eq!(
                    same_lines(hunk, reverse),
                    same_lines(read, reverse),
                    "reverse: {reverse}"
                );
            }
        }
    }
}

/// Checks that each line has the numbers of the sides it is on, counted on from the hunk's starts.
fn numbered(hunk: &Hunk) {
    let (mut old, mut new) = (hunk.old_start, hunk.new_start);
    let (mut old_lines, mut new_lines) = (0, 0);
    for line in &hunk.lines {
        let (on_old, on_new) = match line.kind {
            LineKind::Context => (true, true),
            LineKind::Removed => (true, false),
            LineKind::Added => (false, true),
        };
        assert_eq!(line.old_number, on_old.then_some(old));
        assert_eq!(line.new_number, on_new.then_some(new));
        if on_old {
            old = old.wrapping_add(1);
            old_lines += 1;
        }
        if on_new {
            new = new.wrapping_add(1);
            new_lines += 1;
        }
    }
    assert!(old_lines <= hunk.old_count && new_lines <= hunk.new_count, "{hunk:?}");
}

/// What a patch keeps of `hunk`: everything but the start of the side it renumbers and that side's line numbers.
fn same_lines(hunk: &Hunk, reverse: bool) -> impl PartialEq + std::fmt::Debug {
    let kept_start = if reverse { hunk.new_start } else { hunk.old_start };
    let lines: Vec<_> = hunk
        .lines
        .iter()
        .map(|line| {
            let kept = if reverse { line.new_number } else { line.old_number };
            (line.kind, kept, line.content.clone(), line.missing_newline)
        })
        .collect();
    (kept_start, hunk.old_count, hunk.new_count, hunk.section.clone(), lines)
}
