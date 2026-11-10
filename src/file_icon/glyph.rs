//! Drawing Seti's glyphs natively: the vendored font, which Core Text reads from memory as it is (WOFF), a template
//! image per glyph that AppKit draws anew for each scale and tints, and a colour per icon that follows the appearance.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::{NSAppearance, NSColor, NSGraphicsContext, NSImage};
use objc2_core_foundation::{CFData, CFIndex, CFRetained, CGAffineTransform, CGPoint};
use objc2_core_graphics::{CGContext, CGGlyph, CGPath};
use objc2_core_text::{CTFont, CTFontManagerCreateFontDescriptorFromData};
use objc2_foundation::{NSRect, NSSize};
use splat_markdown::palette::Appearance;

use super::theme::{Icon, Rgb};
use crate::util::guarded;

/// The Seti font as VS Code ships it, see `assets/seti/README.md`.
const SETI: &[u8] = include_bytes!("../../assets/seti/seti.woff");

/// The side of an icon's square, in points.
pub const SIDE: f64 = 16.0;

/// The glyphs' size: VS Code draws them at 150 % of the 13-pixel font of its lists (the theme's `fonts`).
const SIZE: f64 = 19.5;

thread_local! {
    static FONT: OnceCell<Option<CFRetained<CTFont>>> = const { OnceCell::new() };
    static IMAGES: RefCell<HashMap<char, Retained<NSImage>>> = RefCell::new(HashMap::new());
    static COLORS: RefCell<HashMap<(Rgb, Rgb), Retained<NSColor>>> = RefCell::new(HashMap::new());
}

/// The font at the glyphs' size.
pub fn font() -> Option<CFRetained<CTFont>> {
    FONT.with(|font| font.get_or_init(load).clone())
}

fn load() -> Option<CFRetained<CTFont>> {
    let data = CFData::from_bytes(SETI);
    // SAFETY: The data are the bytes of a font file, which Core Text reads or refuses.
    let descriptor = unsafe { CTFontManagerCreateFontDescriptorFromData(&data) }?;
    // SAFETY: A null matrix asks for the identity.
    Some(unsafe { CTFont::with_font_descriptor(&descriptor, SIZE, std::ptr::null()) })
}

/// The font's glyph for `character`, if it has one.
pub fn glyph_of(font: &CTFont, character: char) -> Option<CGGlyph> {
    let mut buffer = [0u16; 2];
    let units = character.encode_utf16(&mut buffer);
    let mut glyphs: [CGGlyph; 2] = [0; 2];
    let count = CFIndex::try_from(units.len()).ok()?;
    // SAFETY: Both buffers hold at least `units.len()` entries, which is one or two.
    let mapped =
        unsafe { font.glyphs_for_characters(NonNull::from(&*units).cast(), NonNull::from(&mut glyphs).cast(), count) };
    (mapped && glyphs[0] != 0).then_some(glyphs[0])
}

/// A template image of `character` centred in an icon's square; AppKit tints it and draws it anew at each scale.
pub fn image(character: char) -> Option<Retained<NSImage>> {
    if let Some(image) = IMAGES.with(|images| images.borrow().get(&character).cloned()) {
        return Some(image);
    }
    let font = font()?;
    let glyph = glyph_of(&font, character)?;
    // The bounds Core Text reports for these glyphs start at the origin whatever the outline; the outline's are right.
    // SAFETY: A null matrix asks for the outline as it is.
    let outline = unsafe { font.path_for_glyph(glyph, std::ptr::null()) };
    let bounds = CGPath::path_bounding_box(outline.as_deref());
    let handler = RcBlock::new(move |rect: NSRect| -> Bool {
        guarded(Bool::NO, || {
            let Some(context) = NSGraphicsContext::currentContext() else {
                return Bool::NO;
            };
            let ctx = context.CGContext();
            CGContext::set_rgb_fill_color(Some(&ctx), 0.0, 0.0, 0.0, 1.0);
            let identity = CGAffineTransform {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: 0.0,
                ty: 0.0,
            };
            CGContext::set_text_matrix(Some(&ctx), identity);
            let mut glyph = glyph;
            let mut position = CGPoint {
                x: rect.origin.x + (rect.size.width - bounds.size.width) / 2.0 - bounds.origin.x,
                y: rect.origin.y + (rect.size.height - bounds.size.height) / 2.0 - bounds.origin.y,
            };
            // SAFETY: One glyph and one position, both on the stack for the length of the call.
            unsafe {
                font.draw_glyphs(NonNull::from(&mut glyph), NonNull::from(&mut position), 1, &ctx);
            }
            Bool::YES
        })
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(SIDE, SIDE), false, &handler);
    image.setTemplate(true);
    IMAGES.with(|images| images.borrow_mut().insert(character, image.clone()));
    Some(image)
}

/// The icon's colour, the theme's dark one in a dark appearance and its light one in a light appearance, chosen
/// anew whenever the appearance changes.
pub fn color(icon: &Icon) -> Retained<NSColor> {
    let (Some(dark), Some(light)) = (icon.dark, icon.light) else {
        return NSColor::labelColor();
    };
    COLORS.with(|colors| {
        colors
            .borrow_mut()
            .entry((dark, light))
            .or_insert_with(|| dynamic(dark, light))
            .clone()
    })
}

fn dynamic(dark: Rgb, light: Rgb) -> Retained<NSColor> {
    let block = RcBlock::new(move |appearance: NonNull<NSAppearance>| -> NonNull<NSColor> {
        let color = guarded(NSColor::secondaryLabelColor(), || {
            // SAFETY: `NonNull` rules out null, and AppKit keeps the appearance alive while it asks for its colour.
            let appearance = unsafe { appearance.as_ref() };
            let rgb = match crate::agent::style::appearance_of(appearance) {
                Appearance::Dark | Appearance::DarkHighContrast => dark,
                Appearance::Light | Appearance::LightHighContrast => light,
            };
            NSColor::colorWithSRGBRed_green_blue_alpha(
                f64::from(rgb.0) / 255.0,
                f64::from(rgb.1) / 255.0,
                f64::from(rgb.2) / 255.0,
                1.0,
            )
        });
        let pointer = Retained::autorelease_return(color);
        // SAFETY: `autorelease_return` hands back the pointer of a live object, which is never null.
        unsafe { NonNull::new_unchecked(pointer) }
    });
    // SAFETY: The provider only reads the appearance it is given and returns an autoreleased colour.
    unsafe { NSColor::colorWithName_dynamicProvider(None, &block) }
}
