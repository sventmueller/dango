//! The self-test's hooks of `line_diagnostics.rs`.

use super::*;

/// The lines of `view` edited within the last second, for the self-test.
pub fn busy_lines(view: &EditorTextView) -> Vec<usize> {
    let now = Instant::now();
    view.ivars()
        .line_edits
        .borrow()
        .0
        .iter()
        .filter(|(_, at)| now.duration_since(*at) < REST)
        .map(|(line, _)| *line)
        .collect()
}
