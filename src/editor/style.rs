//! Fonts and the colours of code, which follow Light, Dark and Increase Contrast by themselves.

use std::cell::RefCell;
use std::collections::HashMap;

use ide_ui_common::style::appearance_of;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSColor, NSFont, NSFontManager, NSFontTraitMask, NSFontWeightBold, NSFontWeightRegular,
};
use splat_markdown::code::CodeToken;
use splat_markdown::highlight::Token;
use splat_markdown::palette::Appearance;

pub(crate) use ide_ui_common::style::{mono as code_font, token_color as palette_color};

use crate::util::constant;

thread_local! {
    static STYLED_FONTS: RefCell<HashMap<(u64, bool, bool), Retained<NSFont>>> = RefCell::new(HashMap::new());
}

/// The code font at `size`, bold and italic as Markdown's source asks; the same width as the plain one.
pub(crate) fn styled_code_font(size: f64, bold: bool, italic: bool) -> Retained<NSFont> {
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

/// Whether `appearance` is one of increased contrast.
pub(super) fn increased_contrast(appearance: &NSAppearance) -> bool {
    matches!(
        appearance_of(appearance),
        Appearance::LightHighContrast | Appearance::DarkHighContrast
    )
}

/// The colour of `token`, resolved per appearance with splat's palette (7:1 contrast).
pub(crate) fn token_color(token: CodeToken) -> Retained<NSColor> {
    palette_color(Token::from(token))
}
