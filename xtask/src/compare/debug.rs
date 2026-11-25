//! `--debug-frames`: the frames before and after a phase's first input as PNG, whole and cut to the measured region.

use std::path::Path;

use image::{RgbaImage, imageops};

use crate::compare::capture::Snapshot;
use crate::compare::fingerprint::Roi;
use crate::compare::stats::whole;
use crate::files;

fn save(picture: &RgbaImage, path: &Path) -> Result<(), String> {
    picture.save(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes `<name>-<when>.png` and `<name>-<when>-roi.png` into `dir`.
pub(crate) fn write(dir: &Path, name: &str, when: &str, snapshot: &Snapshot, roi: Roi) -> Result<(), String> {
    files::create_dir_all(dir)?;
    let width = u32::try_from(snapshot.width).map_err(|e| e.to_string())?;
    let height = u32::try_from(snapshot.height).map_err(|e| e.to_string())?;
    let picture = RgbaImage::from_raw(width, height, snapshot.rgba.clone())
        .ok_or_else(|| format!("a frame of {width} × {height} without its pixels"))?;
    save(&picture, &dir.join(format!("{name}-{when}.png")))?;
    let at = |fraction: f64, length: u32| u32::try_from(whole(fraction * f64::from(length))).unwrap_or(0);
    let (x, y) = (at(roi.left, width), at(roi.top, height));
    let (w, h) = (
        at(roi.right, width).saturating_sub(x),
        at(roi.bottom, height).saturating_sub(y),
    );
    let cut = imageops::crop_imm(&picture, x, y, w, h).to_image();
    save(&cut, &dir.join(format!("{name}-{when}-roi.png")))
}
