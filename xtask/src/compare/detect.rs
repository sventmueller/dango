//! When the recorded frames changed: after an input, while idle, and in the shape of a typed line.

use crate::compare::fingerprint::Changes;

/// A frame ScreenCaptureKit reported as complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    /// When it reached the display, on the host clock in nanoseconds.
    pub(crate) at_ns: u64,
    /// The cells it differs in from the frame before; none for no difference.
    pub(crate) changes: Option<Changes>,
    /// `Fingerprint::spread` of the frame.
    pub(crate) spread: u32,
}

/// A spread above this means text is drawn; an empty editor or a window still blank stays far below it.
pub(crate) const CONTENT_SPREAD: u32 = 800;

/// A typed character moves the rest of its line: a change at most this many cells high, which a text line of 13 pt
/// at four points a cell spans with room to spare.
const LINE_ROWS: usize = 7;
/// And at least this many cells wide, wider than a cursor.
const LINE_COLS: usize = 3;

/// The changes of `frame` that `mask` does not cover, if any.
fn visible(frame: &Frame, mask: Option<&Changes>) -> Option<Changes> {
    let changes = frame.changes.as_ref()?;
    let left = mask.map_or_else(|| changes.clone(), |mask| changes.without(mask));
    (!left.is_empty()).then_some(left)
}

/// Every cell that changed in a frame after `from_ns` up to and with `to_ns`: what changes on its own, like a blinking
/// cursor, which the measurement then ignores.
pub(crate) fn mask(frames: &[Frame], from_ns: u64, to_ns: u64) -> Option<Changes> {
    let mut mask: Option<Changes> = None;
    for changes in frames
        .iter()
        .filter(|f| f.at_ns > from_ns && f.at_ns <= to_ns)
        .filter_map(|f| f.changes.as_ref())
    {
        match &mut mask {
            Some(mask) => mask.add(changes),
            None => mask = Some(changes.clone()),
        }
    }
    mask
}

/// When the first frame after `after_ns` showed a change outside `mask`.
pub(crate) fn first_change(frames: &[Frame], after_ns: u64, mask: Option<&Changes>) -> Option<u64> {
    frames
        .iter()
        .filter(|f| f.at_ns > after_ns)
        .find(|f| visible(f, mask).is_some())
        .map(|f| f.at_ns)
}

/// When each frame from `from_ns` up to `to_ns` showed a change outside `mask`, in order.
pub(crate) fn changes_between(frames: &[Frame], from_ns: u64, to_ns: u64, mask: Option<&Changes>) -> Vec<u64> {
    frames
        .iter()
        .filter(|f| f.at_ns > from_ns && f.at_ns <= to_ns)
        .filter(|f| visible(f, mask).is_some())
        .map(|f| f.at_ns)
        .collect()
}

/// When the last frame with any change reached the display.
pub(crate) fn last_change(frames: &[Frame]) -> Option<u64> {
    frames.iter().rev().find(|f| f.changes.is_some()).map(|f| f.at_ns)
}

/// When the first frame after `after_ns` showed text.
pub(crate) fn first_content(frames: &[Frame], after_ns: u64) -> Option<u64> {
    frames
        .iter()
        .find(|f| f.at_ns > after_ns && f.spread > CONTENT_SPREAD)
        .map(|f| f.at_ns)
}

/// When the first frame after `after_ns` changed like a line a character was typed into: one or two low bands, the
/// line and perhaps the status bar's caret position, one of them wide; unlike the loading of a file, its colouring or
/// a blinking cursor.
pub(crate) fn first_typed_line(frames: &[Frame], after_ns: u64) -> Option<u64> {
    frames
        .iter()
        .filter(|f| f.at_ns > after_ns)
        .find(|f| {
            visible(f, None).is_some_and(|changes| {
                let bands = changes.bands();
                bands.len() <= 2
                    && bands.iter().all(|band| band.height() <= LINE_ROWS)
                    && bands.iter().any(|band| band.width() >= LINE_COLS)
            })
        })
        .map(|f| f.at_ns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::fingerprint::{Image, Roi, THRESHOLD, fingerprint};

    const WHOLE: Roi = Roi {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    };

    /// The changes of a 64 by 64 picture once white rectangles `(x, y, width, height)` are drawn into it.
    fn drawn(rects: &[(usize, usize, usize, usize)]) -> Changes {
        let plain = vec![0u8; 64 * 64 * 4];
        let mut marked = plain.clone();
        for &(x, y, w, h) in rects {
            for row in y..y + h {
                marked[(row * 64 + x) * 4..(row * 64 + x + w) * 4].fill(255);
            }
        }
        let print = |bytes: &[u8]| {
            fingerprint(
                &Image {
                    bytes,
                    width: 64,
                    height: 64,
                    stride: 256,
                },
                WHOLE,
            )
        };
        print(&marked).changes_from(&print(&plain), THRESHOLD)
    }

    fn frame(at_ns: u64, changes: Option<Changes>) -> Frame {
        Frame {
            at_ns,
            changes,
            spread: 0,
        }
    }

    #[test]
    fn the_first_change_after_the_input_counts_and_a_masked_blink_does_not() {
        let cursor = drawn(&[(0, 0, 2, 16)]);
        let typed = drawn(&[(0, 0, 40, 16)]);
        let frames = vec![
            frame(100, Some(cursor.clone())),
            frame(200, None),
            frame(300, Some(cursor.clone())),
            frame(400, Some(cursor.clone())),
            frame(500, Some(typed)),
        ];
        let blink = mask(&frames, 0, 300);
        assert_eq!(blink.as_ref().map(Changes::count), Some(cursor.count()));
        assert_eq!(first_change(&frames, 350, blink.as_ref()), Some(500));
        assert_eq!(first_change(&frames, 350, None), Some(400));
        assert_eq!(first_change(&frames, 500, None), None);
        assert_eq!(changes_between(&frames, 0, 500, blink.as_ref()), [500]);
        assert_eq!(last_change(&frames), Some(500));
    }

    #[test]
    fn nothing_changed_is_no_mask() {
        let frames = vec![frame(10, None), frame(20, None)];
        assert!(mask(&frames, 0, 100).is_none());
        assert_eq!(last_change(&frames), None);
    }

    #[test]
    fn content_is_the_first_frame_with_text() {
        let mut frames = vec![frame(10, None), frame(20, None), frame(30, None)];
        frames[1].spread = CONTENT_SPREAD / 2;
        frames[2].spread = CONTENT_SPREAD * 4;
        assert_eq!(first_content(&frames, 0), Some(30));
        assert_eq!(first_content(&frames, 30), None);
    }

    #[test]
    fn a_typed_line_is_low_and_wide_unlike_a_cursor_or_a_page() {
        let frames = vec![
            frame(10, Some(drawn(&[(0, 0, 2, 16)]))),
            frame(20, Some(drawn(&[(0, 0, 64, 64)]))),
            frame(30, Some(drawn(&[(8, 20, 48, 14)]))),
            frame(40, Some(drawn(&[(8, 20, 48, 14), (40, 56, 16, 6)]))),
            frame(50, Some(drawn(&[(8, 0, 48, 6), (8, 20, 48, 6), (8, 40, 48, 6)]))),
        ];
        assert_eq!(first_typed_line(&frames, 0), Some(30));
        assert_eq!(
            first_typed_line(&frames, 30),
            Some(40),
            "the line with the status bar's caret position"
        );
        assert_eq!(first_typed_line(&frames, 40), None, "three bands are more than a line");
    }
}
