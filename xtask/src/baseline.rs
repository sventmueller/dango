//! Compares window pictures of a selftest run with accepted ones in `tests/snapshots/<scenario>/<appearance>/`, and
//! accepts new ones with `cargo xtask selftest-accept`. See docs/development.md.

use std::path::{Path, PathBuf};

use image::{Rgba, RgbaImage, imageops};

use crate::*;

/// How far one channel of a pixel may stray before the pixel counts as changed; font smoothing and the desktop
/// showing through glass shift edges by less.
const CHANNEL_TOLERANCE: u8 = 48;
/// How many pixels an edge may move before its pixels count as changed; text lands on half points differently from
/// run to run.
const SHIFT: u32 = 1;
/// The share of changed pixels a picture may have before it counts as different.
const CHANGED_SHARE: f64 = 0.01;

/// The folder of the accepted pictures of `scenario` in `appearance`.
fn folder(scenario: &str, appearance: &str) -> PathBuf {
    root().join("tests/snapshots").join(scenario).join(appearance)
}

/// The appearance the report in `reports` of `scenario` names, `light` or `dark`, with `-dictation` from a build with
/// dictation.
fn appearance(scenario: &str, reports: &Path) -> Result<String, String> {
    let written = std::fs::read(reports.join(format!("{scenario}.json"))).ok();
    let report = parse_report(scenario, written.as_deref())?;
    report
        .appearance
        .ok_or_else(|| "the report names no appearance".to_string())
}

/// Compares every accepted picture of `scenario` with the one the run left in `reports`; the error names those that
/// differ, each with a picture of the difference beside it.
pub(crate) fn compare(scenario: &str, reports: &Path) -> Result<(), String> {
    let accepted = root().join("tests/snapshots").join(scenario);
    if !accepted.exists() {
        return Ok(());
    }
    let appearance = appearance(scenario, reports)?;
    let Ok(entries) = std::fs::read_dir(folder(scenario, &appearance)) else {
        println!("  no accepted pictures in {appearance}; `cargo xtask selftest-accept {scenario}` takes this run's");
        return Ok(());
    };
    let mut problems = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "png") {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match differs(&path, &reports.join(&name)) {
            Ok(None) => println!("  ✓ {name} matches its accepted picture"),
            Ok(Some((share, diff))) => {
                let diff_path = reports.join(name.replace(".png", ".diff.png"));
                diff.save(&diff_path)
                    .map_err(|e| format!("{}: {e}", diff_path.display()))?;
                problems.push(format!(
                    "{name} differs from its accepted picture in {:.1} % of its pixels, see {}",
                    share * 100.0,
                    diff_path.display()
                ));
            }
            Err(e) => problems.push(e),
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// None if the picture at `taken` matches the one at `accepted`, else the share of changed pixels and a picture of
/// them: the accepted picture faded, with changed pixels red.
fn differs(accepted: &Path, taken: &Path) -> Result<Option<(f64, RgbaImage)>, String> {
    let open = |path: &Path| {
        image::open(path)
            .map(|picture| picture.to_rgba8())
            .map_err(|e| format!("{}: {e}", path.display()))
    };
    let (accepted, taken) = (open(accepted)?, open(taken)?);
    Ok(difference(&accepted, &taken))
}

pub(crate) fn difference(accepted: &RgbaImage, taken: &RgbaImage) -> Option<(f64, RgbaImage)> {
    let (width, height) = accepted.dimensions();
    let taken = if taken.dimensions() == (width, height) {
        taken.clone()
    } else {
        // A Mac with another display scale takes the same window at another pixel size.
        imageops::resize(taken, width, height, imageops::FilterType::Triangle)
    };
    let mut diff = RgbaImage::new(width, height);
    let mut changed = 0u64;
    for (x, y, a) in accepted.enumerate_pixels() {
        let far = !near(a, &taken, x, y);
        if far {
            changed += 1;
            diff.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        } else {
            let grey = u8::try_from((u16::from(a[0]) + u16::from(a[1]) + u16::from(a[2])) / 3).unwrap_or(0);
            diff.put_pixel(x, y, Rgba([grey, grey, grey, 64]));
        }
    }
    let share = changed as f64 / (f64::from(width) * f64::from(height)).max(1.0);
    (share > CHANGED_SHARE).then_some((share, diff))
}

/// Whether `taken` has a pixel within `SHIFT` of (`x`, `y`) that matches `a` within `CHANNEL_TOLERANCE`.
fn near(a: &Rgba<u8>, taken: &RgbaImage, x: u32, y: u32) -> bool {
    let (width, height) = taken.dimensions();
    let xs = x.saturating_sub(SHIFT)..=(x + SHIFT).min(width - 1);
    xs.into_iter().any(|nx| {
        (y.saturating_sub(SHIFT)..=(y + SHIFT).min(height - 1)).any(|ny| {
            let b = taken.get_pixel(nx, ny);
            a.0.iter()
                .zip(b.0.iter())
                .all(|(a, b)| a.abs_diff(*b) <= CHANNEL_TOLERANCE)
        })
    })
}

/// Takes the pictures `pictures` of the last run of `scenario` as its accepted ones, in the appearance that run had;
/// without names, every picture that already has an accepted one.
pub(crate) fn accept(scenario: &str, pictures: &[String]) -> Result<(), String> {
    let reports = target_dir().join("selftest");
    let appearance = appearance(scenario, &reports)?;
    let target = folder(scenario, &appearance);
    let mut names: Vec<String> = pictures
        .iter()
        .map(|p| format!("{}.png", p.trim_end_matches(".png")))
        .collect();
    if names.is_empty() {
        names = std::fs::read_dir(&target)
            .map_err(|_| {
                format!(
                    "{} has no accepted pictures yet; name the pictures to accept",
                    target.display()
                )
            })?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".png"))
            .collect();
    }
    files::create_dir_all(&target)?;
    for name in names {
        let from = reports.join(&name);
        std::fs::copy(&from, target.join(&name)).map_err(|e| format!("{}: {e}", from.display()))?;
        println!("accepted {}", target.join(&name).display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(width: u32, height: u32, value: u8) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba([value, value, value, 255]))
    }

    #[test]
    fn small_shifts_and_another_scale_match() {
        let accepted = plain(40, 20, 100);
        assert!(difference(&accepted, &plain(40, 20, 130)).is_none());
        assert!(difference(&accepted, &plain(80, 40, 100)).is_none());
        let mut few = accepted.clone();
        few.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        assert!(difference(&accepted, &few).is_none());
    }

    #[test]
    fn an_edge_moved_by_one_pixel_is_the_same_picture() {
        let accepted = plain(40, 20, 100);
        let mut taken = accepted.clone();
        for y in 0..20 {
            taken.put_pixel(5, y, Rgba([255, 255, 255, 255]));
        }
        let mut shifted = accepted.clone();
        for y in 0..20 {
            shifted.put_pixel(6, y, Rgba([255, 255, 255, 255]));
        }
        assert!(difference(&taken, &shifted).is_none());
    }

    #[test]
    fn a_changed_region_differs_and_shows_red() {
        let accepted = plain(40, 20, 100);
        let mut taken = accepted.clone();
        for x in 0..10 {
            for y in 0..5 {
                taken.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        let (share, diff) = difference(&accepted, &taken).unwrap();
        // Pixels next to an unchanged one pass as a shifted edge: 9 × 4 of the 10 × 5 block stay.
        assert!((share - 36.0 / 800.0).abs() < 1e-9);
        assert_eq!(diff.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(diff.get_pixel(20, 10).0[3], 64);
    }
}
