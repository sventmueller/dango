//! The self-test's hooks of `minimap.rs`.

use super::*;

/// The minimap of `view` as its width, the first row the editor shows and the logical lines of the rows it shows, for
/// the self-test.
pub(crate) fn probe(view: &EditorTextView) -> Option<(f64, usize, Vec<usize>)> {
    let map = of(view)?;
    let (first, lines) = map.shown_rows();
    Some((map.frame().size.width, first, lines))
}

/// Clicks the minimap of `view` at `y` from its top, for the self-test.
pub(crate) fn click(view: &EditorTextView, y: f64) {
    if let Some(map) = of(view) {
        map.jump_to(y);
    }
}

impl Minimap {
    /// The first row the editor shows and the logical lines of the rows the column shows, for the self-test.
    pub(crate) fn shown_rows(&self) -> (usize, Vec<usize>) {
        let Some(editor) = self.ivars().editor.load() else {
            return (0, Vec::new());
        };
        let height = self.bounds().size.height;
        let rows = self.rows(&editor, height);
        let first = index(rows.offset / ROW);
        let count = index((height / ROW).ceil());
        let lines = (first..first + count).map_while(|row| rows.lines.get(row)).collect();
        (rows.visible.start, lines)
    }
}
