//! Fonts and the colours of code, which follow Light, Dark and Increase Contrast by themselves.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAccessibilityHighContrastAqua, NSAppearanceNameAccessibilityHighContrastDarkAqua,
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSColor, NSFont, NSFontManager, NSFontTraitMask, NSFontWeightBold,
    NSFontWeightRegular,
};
use objc2_foundation::NSArray;
use splat_markdown::code::CodeToken;
use splat_markdown::highlight::Token;
use splat_markdown::palette::{self, Appearance, Rgb};

use crate::util::constant;

pub fn code_font(size: f64) -> Retained<NSFont> {
    NSFont::monospacedSystemFontOfSize_weight(size, constant!(NSFontWeightRegular))
}

thread_local! {
    static STYLED_FONTS: RefCell<HashMap<(u64, bool, bool), Retained<NSFont>>> = RefCell::new(HashMap::new());
}

/// The code font at `size`, bold and italic as Markdown's source asks; the same width as the plain one.
pub fn styled_code_font(size: f64, bold: bool, italic: bool) -> Retained<NSFont> {
    STYLED_FONTS.with(|cache| {
        cache
            .borrow_mut()
            .entry((size.to_bits(), bold, italic))
            .or_insert_with(|| {
                let weight = if bold {
                    constant!(NSFontWeightBold)
                } else {
                    constant!(NSFontWeightRegular)
                };
                let font = NSFont::monospacedSystemFontOfSize_weight(size, weight);
                match objc2::MainThreadMarker::new().filter(|_| italic) {
                    Some(mtm) => NSFontManager::sharedFontManager(mtm)
                        .convertFont_toHaveTrait(&font, NSFontTraitMask::ItalicFontMask),
                    None => font,
                }
            })
            .clone()
    })
}

fn appearance_of(appearance: &NSAppearance) -> Appearance {
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

thread_local! {
    static TOKEN_COLORS: RefCell<HashMap<Token, Retained<NSColor>>> = RefCell::new(HashMap::new());
}

/// The colour of `token`, resolved per appearance with splat's palette (7:1 contrast).
pub fn token_color(token: CodeToken) -> Retained<NSColor> {
    palette_color(Token::from(token))
}

/// The colour of any of splat's tokens, as `token_color` resolves it.
pub fn palette_color(token: Token) -> Retained<NSColor> {
    TOKEN_COLORS.with(|cache| {
        cache
            .borrow_mut()
            .entry(token)
            .or_insert_with(|| {
                let block = RcBlock::new(move |appearance: NonNull<NSAppearance>| -> NonNull<NSColor> {
                    let color = crate::util::guarded(NSColor::labelColor(), || {
                        // SAFETY: `NonNull` rules out null, and AppKit keeps the appearance alive while it asks.
                        let appearance = unsafe { appearance.as_ref() };
                        srgb(palette::color(token, appearance_of(appearance)))
                    });
                    NonNull::new(Retained::autorelease_return(color)).expect("a color")
                });
                // SAFETY: The provider only reads the appearance it is given and returns a color.
                unsafe { NSColor::colorWithName_dynamicProvider(None, &block) }
            })
            .clone()
    })
}
