//! A real screen capture of the editor's window, read in the editor's coordinates: how many pixels of a rectangle
//! stand out from the editor's ground.

use std::path::Path;

use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::editor::EditorTextView;

/// A capture of one window, without its shadow.
pub(crate) struct Picture {
    image: image::RgbaImage,
    /// Pixels per point.
    scale: f64,
    /// The window's height in points, as window coordinates count from its bottom.
    height: f64,
}

/// How many pixels of a rectangle differ from the ground, out of how many.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Count {
    pub(crate) marked: usize,
    pub(crate) all: usize,
}

impl Count {
    pub(crate) fn share(self) -> f64 {
        if self.all == 0 {
            0.0
        } else {
            self.marked as f64 / self.all as f64
        }
    }
}

impl Picture {
    /// Reads the capture at `path` of the window that shows `view`.
    pub(crate) fn load(path: &Path, view: &EditorTextView) -> Result<Picture, String> {
        let window = view.window().ok_or("no window")?;
        let frame = window.frame();
        let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
        let scale = f64::from(image.width()) / frame.size.width;
        let tall = f64::from(image.height()) / scale;
        if (tall - frame.size.height).abs() > 2.0 {
            return Err(format!(
                "a capture of {}×{} pixels does not fit a window of {:?}",
                image.width(),
                image.height(),
                frame.size
            ));
        }
        Ok(Picture {
            image,
            scale,
            height: frame.size.height,
        })
    }

    /// The pixels `rect` of `view` covers, as columns and rows of the picture.
    fn pixels(&self, view: &EditorTextView, rect: NSRect) -> (std::ops::Range<u32>, std::ops::Range<u32>) {
        let r = view.convertRect_toView(rect, None);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "pixels of the picture, kept inside it"
        )]
        let clamp = |v: f64, most: u32| v.clamp(0.0, f64::from(most)) as u32;
        let (w, h) = (self.image.width(), self.image.height());
        let x =
            clamp((r.origin.x * self.scale).round(), w)..clamp(((r.origin.x + r.size.width) * self.scale).round(), w);
        let top = self.height - (r.origin.y + r.size.height);
        let y = clamp((top * self.scale).round(), h)..clamp(((top + r.size.height) * self.scale).round(), h);
        (x, y)
    }

    fn at(&self, x: u32, y: u32) -> [u8; 3] {
        let p = self.image.get_pixel(x, y).0;
        [p[0], p[1], p[2]]
    }

    /// The ground of `view` on the row of `point`: near the right edge of what shows, past the end of every line of
    /// the scenario's text, left of where an overlay scroller appears.
    pub(crate) fn ground(&self, view: &EditorTextView, y: f64) -> [u8; 3] {
        let seen = view.visibleRect();
        let (x, y) = self.pixels(
            view,
            NSRect::new(
                NSPoint::new(seen.origin.x + seen.size.width - 40.0, y),
                NSSize::new(1.0, 1.0),
            ),
        );
        self.at(
            x.start.min(self.image.width() - 1),
            y.start.min(self.image.height() - 1),
        )
    }

    /// The pixels of `rect` of `view` that differ from the ground by more than `threshold` in some channel.
    pub(crate) fn ink(&self, view: &EditorTextView, rect: NSRect, threshold: u8) -> Count {
        let ground = self.ground(view, rect.origin.y + rect.size.height / 2.0);
        self.count(view, rect, |p| {
            p.iter().zip(ground).any(|(a, b)| a.abs_diff(b) > threshold)
        })
    }

    /// The pixels of `rect` of `view` tinted blue, as a selection is.
    pub(crate) fn tint(&self, view: &EditorTextView, rect: NSRect) -> Count {
        self.count(view, rect, |p| i16::from(p[2]) - i16::from(p[0]) > 8)
    }

    fn count(&self, view: &EditorTextView, rect: NSRect, marked: impl Fn([u8; 3]) -> bool) -> Count {
        let (xs, ys) = self.pixels(view, rect);
        let mut count = Count { marked: 0, all: 0 };
        for y in ys {
            for x in xs.clone() {
                count.all += 1;
                if marked(self.at(x, y)) {
                    count.marked += 1;
                }
            }
        }
        count
    }
}
