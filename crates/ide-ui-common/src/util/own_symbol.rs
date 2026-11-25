//! Symbols the app draws itself where SF Symbols has none: template images that take a button's or image view's tint
//! like an SF Symbol does, or images in one colour for places that tint nothing, such as a window tab's title. They
//! are bitmaps, since a window tab shows no image that is drawn only when it is shown.

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSBezierPath, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage, NSLineCapStyle,
    NSLineJoinStyle,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::ns;

/// The name of the factory symbol of the Software Factory.
pub const FACTORY_SYMBOL: &str = "ide.factory";

/// The size of the drawing at 16 points, which is as large as an SF Symbol of that point size.
const BOX: NSSize = NSSize::new(20.0, 18.0);

/// How much of the box the drawing fills, so it looks as large as the SF Symbols beside it, which leave room in theirs.
const INK: f64 = 0.8;

/// Pixels per point of the bitmap, as many as a Retina display has.
const PIXELS_PER_POINT: f64 = 2.0;

/// Whether the app draws the symbol `name` itself.
pub fn is_own_symbol(name: &str) -> bool {
    name == FACTORY_SYMBOL
}

/// The symbol `name` the app draws itself, as large as an SF Symbol of `points`, described for VoiceOver.
pub(crate) fn own_symbol(name: &str, description: &str, points: f64) -> Option<Retained<NSImage>> {
    let image = render(name, description, points, &NSColor::blackColor())?;
    image.setTemplate(true);
    Some(image)
}

/// The symbol `name` the app draws itself at `points`, in `color` as the current appearance shows it.
pub fn own_symbol_in(name: &str, description: &str, points: f64, color: &NSColor) -> Option<Retained<NSImage>> {
    render(name, description, points, color)
}

fn render(name: &str, description: &str, points: f64, color: &NSColor) -> Option<Retained<NSImage>> {
    if name != FACTORY_SYMBOL {
        return None;
    }
    let size = NSSize::new(BOX.width * points / 16.0, BOX.height * points / 16.0);
    #[expect(clippy::cast_possible_truncation, reason = "a symbol's pixels, a few dozen")]
    let (wide, high) = (
        (size.width * PIXELS_PER_POINT).ceil() as isize,
        (size.height * PIXELS_PER_POINT).ceil() as isize,
    );
    // SAFETY: Without plane pointers the image rep allocates its own buffer of the given size.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            wide,
            high,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }?;
    rep.setSize(size);
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    color.set();
    draw_factory(NSRect::new(NSPoint::new(0.0, 0.0), size));
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    let image = NSImage::initWithSize(NSImage::alloc(), size);
    image.addRepresentation(&rep);
    image.setAccessibilityDescription(Some(&ns(description)));
    Some(image)
}

/// A chimney, a sawtooth roof of three bays and a window in each, stroked as SF Symbols' regular weight is, in the
/// colour set.
fn draw_factory(rect: NSRect) {
    let full = rect.size.width / BOX.width;
    let scale = full * INK;
    let x0 = rect.origin.x + rect.size.width * (1.0 - INK) / 2.0;
    let y0 = rect.origin.y + rect.size.height * (1.0 - INK) / 2.0;
    let at = |x: f64, y: f64| NSPoint::new(x0 + x * scale, y0 + y * scale);
    let bay = (19.0 - 4.5) / 3.0;
    let outline = NSBezierPath::bezierPath();
    outline.moveToPoint(at(1.0, 0.75));
    outline.lineToPoint(at(1.0, 17.25));
    outline.lineToPoint(at(4.5, 17.25));
    for step in 0..3 {
        let left = 4.5 + bay * f64::from(step);
        outline.lineToPoint(at(left, 9.5));
        outline.lineToPoint(at(left + bay, 12.5));
    }
    outline.lineToPoint(at(19.0, 0.75));
    outline.closePath();
    outline.setLineWidth(1.4 * full);
    outline.setLineJoinStyle(NSLineJoinStyle::Round);
    outline.setLineCapStyle(NSLineCapStyle::Round);
    outline.stroke();
    for step in 0..3 {
        let middle = 4.5 + bay * (f64::from(step) + 0.5);
        let origin = at(middle - 1.0, 3.5);
        let window = NSRect::new(origin, NSSize::new(2.0 * scale, 2.0 * scale));
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(window, 0.4 * scale, 0.4 * scale).fill();
    }
}
