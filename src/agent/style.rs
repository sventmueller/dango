//! Sizes, fonts and colours of the AI tab. System colours throughout, as Apple's guidelines ask; an agent's own colour
//! only tells its tabs apart in the tab bar, made to keep its contrast in every appearance.

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

/// The accents made so far, by their colour.
type Accents = HashMap<(u8, u8, u8), Retained<NSColor>>;

thread_local! {
    static TOKEN_COLORS: RefCell<HashMap<Token, Retained<NSColor>>> = RefCell::new(HashMap::new());
    static ACCENT: RefCell<Option<Retained<NSColor>>> = const { RefCell::new(None) };
    static ACCENTS: RefCell<Accents> = RefCell::new(HashMap::new());
}

/// Claude's terracotta as it is in light and dark.
pub const TERRACOTTA: Rgb = Rgb(0xD9, 0x77, 0x57);
/// The text background an accent stands on, in light and in dark.
const LIGHT_GROUND: Rgb = Rgb(0xFF, 0xFF, 0xFF);
const DARK_GROUND: Rgb = Rgb(0x1E, 0x1E, 0x1E);
/// The contrast an accent keeps with its ground, WCAG's for symbols and borders, and with Increase Contrast the one
/// for text.
const ACCENT_CONTRAST: f64 = 3.0;
const HIGH_CONTRAST: f64 = 4.5;
/// In how many steps an accent moves towards black or white.
const MIX_STEPS: i32 = 40;

fn luminance(c: Rgb) -> f64 {
    let channel = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.0) + 0.7152 * channel(c.1) + 0.0722 * channel(c.2)
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `base` mixed towards black or white, whichever is away from `ground`, until it has `wanted` contrast with it.
fn against(base: Rgb, ground: Rgb, wanted: f64) -> Rgb {
    let towards: u8 = if luminance(ground) > 0.5 { 0 } else { u8::MAX };
    let mix = |v: u8, step: i32| {
        let moved = i32::from(v) + (i32::from(towards) - i32::from(v)) * step / MIX_STEPS;
        u8::try_from(moved).unwrap_or(towards)
    };
    (0..=MIX_STEPS)
        .map(|step| Rgb(mix(base.0, step), mix(base.1, step), mix(base.2, step)))
        .find(|candidate| contrast(*candidate, ground) >= wanted)
        .unwrap_or(Rgb(towards, towards, towards))
}

/// An agent's colour `base` as its accent: as it is where it stands out enough, else darker on light and lighter on
/// dark grounds, more so with Increase Contrast.
pub fn accent(base: Rgb) -> Retained<NSColor> {
    cached(base, move |appearance| match appearance {
        Appearance::Light => against(base, LIGHT_GROUND, ACCENT_CONTRAST),
        Appearance::Dark => against(base, DARK_GROUND, ACCENT_CONTRAST),
        Appearance::LightHighContrast => against(base, LIGHT_GROUND, HIGH_CONTRAST),
        Appearance::DarkHighContrast => against(base, DARK_GROUND, HIGH_CONTRAST),
    })
}

fn cached(base: Rgb, pick: impl Fn(Appearance) -> Rgb + 'static) -> Retained<NSColor> {
    ACCENTS.with(|cache| {
        cache
            .borrow_mut()
            .entry((base.0, base.1, base.2))
            .or_insert_with(|| dynamic(pick))
            .clone()
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Claude's terracotta, Gemini's blue, a light yellow and a near black.
    const COLOURS: [Rgb; 4] = [
        TERRACOTTA,
        Rgb(0x47, 0x96, 0xE3),
        Rgb(0xF5, 0xC5, 0x18),
        Rgb(0x22, 0x22, 0x22),
    ];

    #[test]
    fn an_accent_keeps_its_contrast_with_the_ground_in_every_appearance() {
        for colour in COLOURS {
            for (ground, wanted) in [
                (LIGHT_GROUND, ACCENT_CONTRAST),
                (DARK_GROUND, ACCENT_CONTRAST),
                (LIGHT_GROUND, HIGH_CONTRAST),
                (DARK_GROUND, HIGH_CONTRAST),
            ] {
                let made = against(colour, ground, wanted);
                assert!(contrast(made, ground) >= wanted, "{colour:?} on {ground:?}");
            }
        }
    }

    #[test]
    fn a_colour_that_stands_out_enough_stays_as_it_is() {
        let blue = Rgb(0x47, 0x96, 0xE3);
        assert_eq!(against(blue, LIGHT_GROUND, ACCENT_CONTRAST), blue);
        assert_eq!(against(TERRACOTTA, DARK_GROUND, ACCENT_CONTRAST), TERRACOTTA);
    }
}
