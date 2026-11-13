//! What the self-test reads from and does with the parts of `mod.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

pub use super::draw::drawn_in;

pub use super::rotor::rotor_items;

pub use super::rotor::rotor_step;

/// Whether editors draw as with increased contrast, for the self-test.
#[derive(Default)]
pub struct ForcedContrast {
    forced_contrast: std::cell::Cell<bool>,
}

pub(super) fn with_forced_contrast<R>(f: impl FnOnce(&std::cell::Cell<bool>) -> R) -> R {
    f(&crate::context::app().editor.forced_contrast.forced_contrast)
}

/// Draws as with increased contrast, which AppKit gives a window only through the system setting, for the self-test.
pub fn force_increased_contrast(on: bool) {
    with_forced_contrast(|forced| forced.set(on));
}

pub(super) fn forced_contrast() -> bool {
    with_forced_contrast(std::cell::Cell::get)
}

pub(super) fn kern_at(storage: &NSTextStorage, at: usize) -> Option<f64> {
    if at >= storage.length() {
        return None;
    }
    let key = constant!(NSKernAttributeName);
    // SAFETY: The index lies inside the text; no effective range is asked for.
    let value = unsafe { storage.attribute_atIndex_effectiveRange(key, at, std::ptr::null_mut()) }?;
    value.downcast::<NSNumber>().ok().map(|n| n.doubleValue())
}

/// The contrast of an inset's text against its box on the editor's ground, in the appearance `view` has now, for the
/// self-test.
pub fn contrast(view: &EditorTextView) -> Option<f64> {
    use objc2_app_kit::{NSAppearanceCustomization, NSColorSpace};
    let rgba = |color: &NSColor| {
        let found = std::cell::Cell::new(None);
        view.effectiveAppearance()
            .performAsCurrentDrawingAppearance(&block2::RcBlock::new(|| {
                crate::util::guarded((), || {
                    found.set(color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()).map(|c| {
                        [
                            c.redComponent(),
                            c.greenComponent(),
                            c.blueComponent(),
                            c.alphaComponent(),
                        ]
                    }));
                })
            }));
        found.get()
    };
    let over =
        |top: [f64; 4], below: [f64; 3]| -> [f64; 3] { [0, 1, 2].map(|i| top[i] * top[3] + below[i] * (1.0 - top[3])) };
    let ground = rgba(&NSColor::textBackgroundColor())?;
    let ground = [ground[0], ground[1], ground[2]];
    let back = over(rgba(&fill())?, ground);
    let front = over(rgba(&text_color(view))?, back);
    let luminance = |c: [f64; 3]| {
        let linear = |v: f64| {
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
    };
    let (a, b) = (luminance(back), luminance(front));
    Some((a.max(b) + 0.05) / (a.min(b) + 0.05))
}

/// The kern attributes in the text of `view`, as ranges with their values, for the self-test.
pub fn kerns(view: &EditorTextView) -> Vec<(Range<usize>, f64)> {
    let Some(storage) = view.storage() else {
        return Vec::new();
    };
    kerned_in(&storage, 0..storage.length())
        .into_iter()
        .map(|range| {
            let width = kern_at(&storage, range.start).unwrap_or(0.0);
            (range, width)
        })
        .collect()
}
