//! What the self-test reads from and does with the parts of `mod.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

pub(crate) use super::highlight::colours_painted;
pub(crate) use super::overlay::LAID_OUT;

mod probe;

pub(crate) use probe::*;

/// Scrolls `view` so that the line holding byte `byte` is the first on screen.
pub(crate) fn scroll_to_byte(view: &EditorTextView, byte: usize) {
    if let Some(y) = byte_y(view, byte) {
        view.scrollPoint(NSPoint::new(view.visibleRect().origin.x, y));
    }
}
