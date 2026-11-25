//! What a captured frame shows, reduced to the brightness of small cells, and which cells two frames differ in.

/// The part of a frame the comparison looks at, as fractions of its width and height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Roi {
    pub(crate) left: f64,
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
}

/// The content of both apps' windows below the title bar: the file tree sits left in this app and right in Zed, and
/// whatever an input changes, the status bar's caret position too, is the app's answer to it.
pub(crate) const EDITOR: Roi = Roi {
    left: 0.0,
    top: 0.06,
    right: 1.0,
    bottom: 0.97,
};

/// The side of a square cell, in pixels of the capture, which has one pixel per point.
pub(crate) const CELL: usize = 4;

/// The least difference of a cell's brightness that counts as a change; it ignores rounding in the scaler.
pub(crate) const THRESHOLD: u32 = 64;

/// A BGRA picture as ScreenCaptureKit hands it over.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Image<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// Bytes from one row to the next, at least four per pixel.
    pub(crate) stride: usize,
}

/// The summed brightness of every cell of a frame's region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fingerprint {
    cols: usize,
    rows: usize,
    cells: Vec<u32>,
}

impl Fingerprint {
    /// How far apart its brightest and its darkest cell are: about nothing for an empty background, much for text.
    pub(crate) fn spread(&self) -> u32 {
        let max = self.cells.iter().copied().max().unwrap_or(0);
        let min = self.cells.iter().copied().min().unwrap_or(0);
        max - min
    }

    /// The cells that differ from `earlier` by more than `threshold`; every cell when the sizes differ.
    pub(crate) fn changes_from(&self, earlier: &Fingerprint, threshold: u32) -> Changes {
        let mut changes = Changes::none(self.cols, self.rows);
        let same_size = (self.cols, self.rows) == (earlier.cols, earlier.rows);
        for (i, cell) in self.cells.iter().enumerate() {
            if !same_size || cell.abs_diff(earlier.cells[i]) > threshold {
                changes.set(i);
            }
        }
        changes
    }
}

/// The fingerprint of `roi` in `image`, in cells of `CELL` pixels; a cell the image does not hold counts as black.
pub(crate) fn fingerprint(image: &Image<'_>, roi: Roi) -> Fingerprint {
    let x0 = at(roi.left, image.width);
    let y0 = at(roi.top, image.height);
    let cols = at(roi.right, image.width).saturating_sub(x0) / CELL;
    let rows = at(roi.bottom, image.height).saturating_sub(y0) / CELL;
    let mut cells = vec![0u32; cols * rows];
    for (i, cell) in cells.iter_mut().enumerate() {
        let (col, row) = (i % cols, i / cols);
        for y in 0..CELL {
            let start = (y0 + row * CELL + y) * image.stride + (x0 + col * CELL) * 4;
            let Some(line) = image.bytes.get(start..start + CELL * 4) else {
                continue;
            };
            *cell += line
                .as_chunks::<4>()
                .0
                .iter()
                .map(|px| u32::from(px[0]) + 2 * u32::from(px[1]) + u32::from(px[2]))
                .sum::<u32>();
        }
    }
    Fingerprint { cols, rows, cells }
}

/// The pixel at `fraction` of `length`.
fn at(fraction: f64, length: usize) -> usize {
    crate::compare::stats::whole(fraction.clamp(0.0, 1.0) * length as f64).min(length)
}

/// A set of cells of a fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Changes {
    cols: usize,
    rows: usize,
    bits: Vec<u64>,
}

/// The cells a set covers: its first and last column and row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) cols: (usize, usize),
    pub(crate) rows: (usize, usize),
}

impl Span {
    pub(crate) fn width(self) -> usize {
        self.cols.1 - self.cols.0 + 1
    }

    pub(crate) fn height(self) -> usize {
        self.rows.1 - self.rows.0 + 1
    }
}

impl Changes {
    pub(crate) fn none(cols: usize, rows: usize) -> Changes {
        Changes {
            cols,
            rows,
            bits: vec![0; (cols * rows).div_ceil(64)],
        }
    }

    fn set(&mut self, i: usize) {
        self.bits[i / 64] |= 1 << (i % 64);
    }

    fn has(&self, i: usize) -> bool {
        self.bits.get(i / 64).is_some_and(|word| word & (1 << (i % 64)) != 0)
    }

    pub(crate) fn count(&self) -> usize {
        self.bits.iter().map(|word| word.count_ones() as usize).sum()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bits.iter().all(|word| *word == 0)
    }

    /// Adds the cells of `other`; a set of another size makes this one every cell.
    pub(crate) fn add(&mut self, other: &Changes) {
        if (self.cols, self.rows) != (other.cols, other.rows) {
            *self = Changes::all(self.cols, self.rows);
            return;
        }
        for (word, more) in self.bits.iter_mut().zip(&other.bits) {
            *word |= more;
        }
    }

    /// These cells without those of `mask`; a mask of another size takes nothing away.
    pub(crate) fn without(&self, mask: &Changes) -> Changes {
        if (self.cols, self.rows) != (mask.cols, mask.rows) {
            return self.clone();
        }
        let bits = self
            .bits
            .iter()
            .zip(&mask.bits)
            .map(|(word, masked)| word & !masked)
            .collect();
        Changes { bits, ..*self }
    }

    fn all(cols: usize, rows: usize) -> Changes {
        let mut all = Changes::none(cols, rows);
        (0..cols * rows).for_each(|i| all.set(i));
        all
    }

    /// The runs of consecutive rows with a change, each with the columns and rows it covers, from the top.
    pub(crate) fn bands(&self) -> Vec<Span> {
        let mut bands: Vec<Span> = Vec::new();
        for row in 0..self.rows {
            let cols: Vec<usize> = (0..self.cols).filter(|col| self.has(row * self.cols + col)).collect();
            let (Some(&first), Some(&last)) = (cols.first(), cols.last()) else {
                continue;
            };
            match bands.last_mut() {
                Some(band) if band.rows.1 + 1 == row => {
                    band.rows.1 = row;
                    band.cols = (band.cols.0.min(first), band.cols.1.max(last));
                }
                _ => bands.push(Span {
                    cols: (first, last),
                    rows: (row, row),
                }),
            }
        }
        bands
    }

    /// The columns and rows the set covers, none for an empty set.
    #[cfg(test)]
    pub(crate) fn span(&self) -> Option<Span> {
        let mut span: Option<Span> = None;
        for i in (0..self.cols * self.rows).filter(|i| self.has(*i)) {
            let (col, row) = (i % self.cols, i / self.cols);
            span = Some(match span {
                None => Span {
                    cols: (col, col),
                    rows: (row, row),
                },
                Some(s) => Span {
                    cols: (s.cols.0.min(col), s.cols.1.max(col)),
                    rows: (s.rows.0.min(row), s.rows.1.max(row)),
                },
            });
        }
        span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHOLE: Roi = Roi {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    };

    /// A grey picture of `width` by `height` with a stride of padding, and white pixels at `marks`.
    fn picture(width: usize, height: usize, marks: &[(usize, usize)]) -> Vec<u8> {
        let stride = width * 4 + 16;
        let mut bytes = vec![100u8; stride * height];
        for (x, y) in marks {
            bytes[y * stride + x * 4..y * stride + x * 4 + 4].fill(255);
        }
        bytes
    }

    fn print(bytes: &[u8], width: usize, height: usize, roi: Roi) -> Fingerprint {
        let image = Image {
            bytes,
            width,
            height,
            stride: width * 4 + 16,
        };
        fingerprint(&image, roi)
    }

    #[test]
    fn the_same_picture_has_no_changes_and_a_mark_changes_its_cell_only() {
        let plain = picture(32, 16, &[]);
        let marked = picture(32, 16, &[(9, 5)]);
        let before = print(&plain, 32, 16, WHOLE);
        assert!(before.changes_from(&print(&plain, 32, 16, WHOLE), THRESHOLD).is_empty());
        let changes = print(&marked, 32, 16, WHOLE).changes_from(&before, THRESHOLD);
        assert_eq!(changes.count(), 1);
        assert_eq!(
            changes.span(),
            Some(Span {
                cols: (2, 2),
                rows: (1, 1)
            })
        );
    }

    #[test]
    fn a_change_below_the_threshold_is_none() {
        let mut faint = picture(8, 8, &[]);
        faint[0] = 110;
        let changes = print(&faint, 8, 8, WHOLE).changes_from(&print(&picture(8, 8, &[]), 8, 8, WHOLE), THRESHOLD);
        assert!(changes.is_empty());
    }

    #[test]
    fn the_region_leaves_out_what_lies_outside() {
        let left_half = Roi { right: 0.5, ..WHOLE };
        let marked = picture(32, 16, &[(30, 3)]);
        let changes =
            print(&marked, 32, 16, left_half).changes_from(&print(&picture(32, 16, &[]), 32, 16, left_half), 0);
        assert!(changes.is_empty());
    }

    #[test]
    fn text_spreads_the_brightness_and_a_plain_background_does_not() {
        assert_eq!(print(&picture(16, 16, &[]), 16, 16, WHOLE).spread(), 0);
        assert!(print(&picture(16, 16, &[(1, 1), (2, 1)]), 16, 16, WHOLE).spread() > 500);
    }

    #[test]
    fn a_mask_takes_its_cells_away_and_sets_add_up() {
        let before = print(&picture(32, 16, &[]), 32, 16, WHOLE);
        let blink = print(&picture(32, 16, &[(0, 0)]), 32, 16, WHOLE).changes_from(&before, 0);
        let typed = print(&picture(32, 16, &[(0, 0), (12, 0), (20, 0)]), 32, 16, WHOLE).changes_from(&before, 0);
        let rest = typed.without(&blink);
        assert_eq!((typed.count(), rest.count()), (3, 2));
        assert_eq!(rest.span().unwrap().width(), 3);
        assert!(blink.without(&blink).is_empty());
        assert_eq!(typed.bands().len(), 1);
        let mut both = blink.clone();
        both.add(&rest);
        assert_eq!(both, typed);
    }

    #[test]
    fn frames_of_another_size_differ_everywhere() {
        let small = print(&picture(16, 16, &[]), 16, 16, WHOLE);
        let large = print(&picture(32, 16, &[]), 32, 16, WHOLE);
        assert_eq!(large.changes_from(&small, THRESHOLD).count(), 8 * 4);
    }

    #[test]
    fn a_short_buffer_reads_as_black_instead_of_panicking() {
        let image = Image {
            bytes: &[255; 16],
            width: 8,
            height: 8,
            stride: 32,
        };
        let print = fingerprint(&image, WHOLE);
        assert_eq!(print.cells.len(), 4);
        assert_eq!(print.spread(), 4 * 4 * 255);
    }
}
