//! Sizes, fonts and colours of the AI tab. System colours throughout; terracotta only as Claude's accent.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAccessibilityHighContrastAqua, NSAppearanceNameAccessibilityHighContrastDarkAqua,
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSColor, NSFont, NSFontWeightMedium, NSFontWeightRegular,
    NSFontWeightSemibold,
};
use objc2_foundation::NSArray;
use splat_markdown::highlight::Token;
use splat_markdown::palette::{self, Appearance, Rgb};

use crate::util::{constant, guarded};

/// The size of running text in the timeline.
pub const BODY: f64 = 13.0;
/// The size of code, commands and output.
pub const CODE: f64 = 12.0;
/// The size of secondary lines: timestamps, status, hints.
pub const SMALL: f64 = 11.0;

pub fn body() -> Retained<NSFont> {
    NSFont::systemFontOfSize(BODY)
}

pub fn bold(size: f64) -> Retained<NSFont> {
    NSFont::systemFontOfSize_weight(size, constant!(NSFontWeightSemibold))
}

pub fn medium(size: f64) -> Retained<NSFont> {
    NSFont::systemFontOfSize_weight(size, constant!(NSFontWeightMedium))
}

pub fn small() -> Retained<NSFont> {
    NSFont::systemFontOfSize(SMALL)
}

pub fn mono(size: f64) -> Retained<NSFont> {
    NSFont::monospacedSystemFontOfSize_weight(size, constant!(NSFontWeightRegular))
}

pub fn appearance_of(appearance: &NSAppearance) -> Appearance {
    let names = NSArray::from_slice(&[
        constant!(NSAppearanceNameAqua),
        constant!(NSAppearanceNameDarkAqua),
        constant!(NSAppearanceNameAccessibilityHighContrastAqua),
        constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua),
    ]);
    let best = appearance
        .bestMatchFromAppearancesWithNames(&names)
        .map(|n| n.to_string())
        .unwrap_or_default();
    if best == constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua).to_string() {
        Appearance::DarkHighContrast
    } else if best == constant!(NSAppearanceNameAccessibilityHighContrastAqua).to_string() {
        Appearance::LightHighContrast
    } else if best == constant!(NSAppearanceNameDarkAqua).to_string() {
        Appearance::Dark
    } else {
        Appearance::Light
    }
}

fn srgb(c: Rgb) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(
        f64::from(c.0) / 255.0,
        f64::from(c.1) / 255.0,
        f64::from(c.2) / 255.0,
        1.0,
    )
}

/// A colour that AppKit resolves anew for each appearance, from `pick`.
fn dynamic(pick: impl Fn(Appearance) -> Rgb + 'static) -> Retained<NSColor> {
    let block = RcBlock::new(move |appearance: NonNull<NSAppearance>| -> NonNull<NSColor> {
        let color = guarded(NSColor::labelColor(), || {
            // SAFETY: `NonNull` rules out null, and AppKit keeps the appearance alive while it asks for its colour.
            let appearance = unsafe { appearance.as_ref() };
            srgb(pick(appearance_of(appearance)))
        });
        let pointer = Retained::autorelease_return(color);
        // SAFETY: `autorelease_return` hands back the pointer of a live object, which is never null.
        unsafe { NonNull::new_unchecked(pointer) }
    });
    // SAFETY: The provider only reads the appearance it is given and returns an autoreleased colour.
    unsafe { NSColor::colorWithName_dynamicProvider(None, &block) }
}

thread_local! {
    static TOKEN_COLORS: RefCell<HashMap<Token, Retained<NSColor>>> = RefCell::new(HashMap::new());
    static ACCENT: RefCell<Option<Retained<NSColor>>> = const { RefCell::new(None) };
}

/// The colour of a code token in the current appearance, splat's palette.
pub fn token_color(token: Token) -> Retained<NSColor> {
    TOKEN_COLORS.with(|cache| {
        cache
            .borrow_mut()
            .entry(token)
            .or_insert_with(|| dynamic(move |appearance| palette::color(token, appearance)))
            .clone()
    })
}

/// Claude's terracotta, darker with Increase Contrast so text and borders keep their contrast.
pub fn terracotta() -> Retained<NSColor> {
    ACCENT.with(|accent| {
        accent
            .borrow_mut()
            .get_or_insert_with(|| {
                dynamic(|appearance| match appearance {
                    Appearance::LightHighContrast => Rgb(0xA8, 0x4A, 0x2E),
                    Appearance::DarkHighContrast => Rgb(0xF0, 0x9A, 0x7E),
                    Appearance::Light | Appearance::Dark => Rgb(0xD9, 0x77, 0x57),
                })
            })
            .clone()
    })
}

/// The background of inline code, code blocks and output boxes.
pub fn code_background() -> Retained<NSColor> {
    NSColor::quaternarySystemFillColor()
}

/// The background of a removed line of a diff.
pub fn removed_background() -> Retained<NSColor> {
    NSColor::systemRedColor().colorWithAlphaComponent(0.16)
}

/// The background of an added line of a diff.
pub fn added_background() -> Retained<NSColor> {
    NSColor::systemGreenColor().colorWithAlphaComponent(0.16)
}
