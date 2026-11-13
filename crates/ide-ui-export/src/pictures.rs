//! The pictures of an export: drawn formulas and diagrams, and the image files a document names, as PNG or JPEG. What
//! reads AppKit's pictures runs on the main thread, in short steps; image files are read and converted on the export's
//! thread.

use std::path::{Component, Path, PathBuf};

use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use splat_decoder::{Decoded, EXPORT_SIDE, MAX_FILE_BYTES};
use splat_markdown::doc::Figure;
use splat_markdown::docx::Picture;
use splat_markdown::html::{data_url, escape, web_image_type};
use splat_render::images::{native_only, resolve_path};
use splat_render::js::{self, Key};
use splat_render::picture::Sizing;
use splat_render::remote;

/// Pictures on paper are three quarters of their size on screen, as the text is.
pub const PRINT_SCALE: f64 = 0.75;

/// What a page says instead of a formula or diagram.
pub fn alt_of(figure: &Figure) -> String {
    match figure {
        Figure::Math { tex, .. } => tex.to_string(),
        Figure::Diagram { .. } => splat_i18n::t("Diagramm").to_string(),
    }
}

/// A drawn formula or diagram as `<img>` with the picture inside it, at twice its size for sharp display; main thread.
pub fn figure_html(key: &Key, alt: &str) -> Option<String> {
    let rendered = js::cached(key)?;
    // One ex is about half the 16 px body text of the page.
    let (size, attributes) = match rendered.sizing {
        Sizing::Ex { width, height, valign } => (
            NSSize::new(width * 8.0, height * 8.0),
            format!(" class=\"math\" style=\"height: {height:.3}ex; vertical-align: {valign:.3}ex\""),
        ),
        Sizing::Points { size, .. } => (size, format!(" width=\"{:.0}\"", size.width)),
    };
    let png = png_of_image(&rendered.image, size, 2.0)?;
    Some(format!(
        "<img src=\"{}\" alt=\"{}\"{attributes}>",
        data_url("image/png", &png),
        escape(alt)
    ))
}

/// A drawn formula or diagram for Word; main thread.
pub fn word_figure(key: &Key) -> Option<Picture> {
    let rendered = js::cached(key)?;
    // One ex is about half the 11 pt body text.
    let size = match rendered.sizing {
        Sizing::Ex { width, height, .. } => NSSize::new(width * 5.5, height * 5.5),
        Sizing::Points { size, .. } => size,
    };
    picture_of(&rendered.image, size)
}

/// A picture from the web that is already loaded, for Word; main thread.
pub fn remote_picture(url: &str) -> Option<Picture> {
    let rendered = remote::get(url, None).flatten()?;
    picture_of(&rendered.image, rendered.image.size())
}

/// `picture` at its size on paper.
pub fn on_paper(picture: Picture) -> Picture {
    Picture {
        width: picture.width * PRINT_SCALE,
        height: picture.height * PRINT_SCALE,
        ..picture
    }
}

fn picture_of(image: &NSImage, size: NSSize) -> Option<Picture> {
    Some(Picture {
        data: png_of_image(image, size, 2.0)?,
        jpeg: false,
        width: size.width,
        height: size.height,
    })
}

/// A local image for Word: PNG and JPEG as they are, other formats converted to PNG.
pub fn local_picture(url: &str, doc_dir: Option<&Path>) -> Option<Picture> {
    let path = &resolve_path(url, doc_dir)?;
    if !may_carry_along(path, doc_dir) {
        return None;
    }
    let bytes = splat_render::files::read_regular_file(path, MAX_FILE_BYTES as u64)?;
    let Decoded::Rgba {
        width,
        height,
        natural,
        pixels,
    } = decoded_file(path, &bytes)?
    else {
        return None;
    };
    // A vector picture has more pixels than points, and it is the points that say how large it is.
    let (w, h) = if natural.0 < width && natural.1 < height {
        natural
    } else {
        (width, height)
    };
    if let Some(mime @ ("image/png" | "image/jpeg")) = web_image_type(&bytes) {
        return Some(Picture {
            data: bytes,
            jpeg: mime == "image/jpeg",
            width: f64::from(w),
            height: f64::from(h),
        });
    }
    Some(Picture {
        data: png_of_pixels(width, height, pixels)?,
        jpeg: false,
        width: f64::from(w),
        height: f64::from(h),
    })
}

/// A local image as `data:` URL: web formats as they are, others converted to PNG. Images from the web stay links.
pub fn embedded(url: &str, doc_dir: Option<&Path>) -> Option<String> {
    if url.contains("://") && splat_markdown::url::behind_file_scheme(url).is_none() {
        return None;
    }
    let path = resolve_path(url, doc_dir)?;
    if !may_carry_along(&path, doc_dir) {
        return None;
    }
    let bytes = splat_render::files::read_regular_file(&path, MAX_FILE_BYTES as u64)?;
    if let Some(mime) = web_image_type(&bytes) {
        return Some(data_url(mime, &bytes));
    }
    // Scripts in an SVG do not run when a page shows it as image.
    if std::str::from_utf8(&bytes).is_ok_and(|text| text.contains("<svg")) {
        return Some(data_url("image/svg+xml", &bytes));
    }
    let Decoded::Rgba {
        width, height, pixels, ..
    } = decoded_file(&path, &bytes)?
    else {
        return None;
    };
    Some(data_url("image/png", &png_of_pixels(width, height, pixels)?))
}

/// Decoded pixels as PNG, without AppKit, so any thread may convert them.
fn png_of_pixels(width: u32, height: u32, pixels: Vec<u8>) -> Option<Vec<u8>> {
    let image = image::RgbaImage::from_raw(width, height, pixels)?;
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

/// Pixels from the decoding service, or from its helper for what only the system reads.
fn decoded_file(path: &Path, bytes: &[u8]) -> Option<Decoded> {
    splat_decoder::decode(bytes, EXPORT_SIDE).or_else(|| {
        native_only(path, bytes)
            .then(|| splat_decoder::decode_native(bytes, EXPORT_SIDE))
            .flatten()
    })
}

/// Whether a file that leaves the Mac with the document may take `path` along: what lies in the document's folder or
/// its repository does, so a document from someone else cannot pull the user's files into what gets sent on. The rule
/// is splat's.
pub fn may_carry_along(path: &Path, doc_dir: Option<&Path>) -> bool {
    let Some(doc_dir) = doc_dir else { return false };
    let home = splat_markdown::url::home();
    let root = doc_dir
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .filter(|root| !home.as_deref().is_some_and(|home| home.starts_with(root)))
        .unwrap_or(doc_dir);
    match (path.canonicalize(), root.canonicalize()) {
        (Ok(file), Ok(root)) => file.starts_with(root),
        _ => false,
    }
}

/// A relative link of the document as the exported page in `export_dir` reaches the same file.
pub fn rebased(link: &str, doc_dir: Option<&Path>, export_dir: Option<&Path>) -> String {
    let (Some(doc_dir), Some(export_dir)) = (doc_dir, export_dir) else {
        return link.to_string();
    };
    if link.is_empty() || link.starts_with('#') || link.contains("://") || link.starts_with('/') {
        return link.to_string();
    }
    let split = link.find(['#', '?']).unwrap_or(link.len());
    let (path, suffix) = link.split_at(split);
    let target = doc_dir.join(splat_markdown::url::percent_decode(path));
    match relative_path(export_dir, &target) {
        Some(relative) => format!("{}{suffix}", in_an_address(&relative.to_string_lossy())),
        // Without a folder in common the only way there is an absolute path, which would show the user's folders.
        None => link.to_string(),
    }
}

/// `target` as seen from `from`, both absolute; `None` when they share no folder but the root.
fn relative_path(from: &Path, target: &Path) -> Option<PathBuf> {
    let (from, target) = (normal(from), normal(target));
    let common = from.iter().zip(&target).take_while(|(a, b)| a == b).count();
    if common <= 1 {
        return None;
    }
    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for part in &target[common..] {
        relative.push(part.as_os_str());
    }
    Some(relative)
}

/// The parts of `path` with `.` and `..` resolved.
fn normal(path: &Path) -> Vec<Component<'_>> {
    let mut parts = Vec::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                parts.pop();
            }
            Component::CurDir => {}
            part => parts.push(part),
        }
    }
    parts
}

/// A path as a link can name it: the characters that would end the path or start an escape are escaped themselves.
fn in_an_address(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            '%' => out.push_str("%25"),
            ' ' => out.push_str("%20"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            c => out.push(c),
        }
    }
    out
}

/// Draws an image into a bitmap of `size` points at `scale` and encodes it as PNG.
fn png_of_image(image: &NSImage, size: NSSize, scale: f64) -> Option<Vec<u8>> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "pixels of a picture, and checked just below"
    )]
    let (w, h) = (
        (size.width * scale).ceil() as isize,
        (size.height * scale).ceil() as isize,
    );
    if w <= 0 || h <= 0 || w * h > 64_000_000 {
        return None;
    }
    // SAFETY: Without plane pointers the image rep allocates its own buffer of the given size.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w,
            h,
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
    image.drawInRect(NSRect::new(NSPoint::new(0.0, 0.0), size));
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    // SAFETY: An empty property dictionary, which every file type accepts.
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }?;
    Some(png.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_leads_to_the_same_file_from_the_export_folder() {
        let doc = Path::new("/p/docs");
        assert_eq!(
            rebased("img/a b.png", Some(doc), Some(Path::new("/p/out"))),
            "../docs/img/a%20b.png"
        );
        assert_eq!(rebased("x.md#part", Some(doc), Some(doc)), "x.md#part");
        assert_eq!(
            rebased("https://a.org/x", Some(doc), Some(Path::new("/p/out"))),
            "https://a.org/x"
        );
        assert_eq!(rebased("#top", Some(doc), Some(Path::new("/q"))), "#top");
        assert_eq!(
            rebased("a.png", Some(doc), Some(Path::new("/q"))),
            "a.png",
            "no folder in common"
        );
    }

    #[test]
    fn only_what_lies_with_the_document_goes_along() {
        let dir = ide_core::testing::scratch("carry");
        let docs = dir.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("in.png"), b"x").unwrap();
        std::fs::write(dir.join("out.png"), b"x").unwrap();
        let inside = may_carry_along(&docs.join("in.png"), Some(&docs));
        let outside = may_carry_along(&dir.join("out.png"), Some(&docs));
        assert!(inside && !outside);
        assert!(!may_carry_along(Path::new("/etc/hosts"), None));
    }

    #[test]
    fn a_link_with_dots_and_signs_still_names_its_file() {
        let doc = Path::new("/p/docs/./notes");
        assert_eq!(
            rebased("../img/50%25%231%3F.png", Some(doc), Some(Path::new("/p/out"))),
            "../docs/img/50%25%231%3F.png"
        );
        assert_eq!(rebased("a.png", None, Some(doc)), "a.png");
    }

    #[test]
    fn a_page_says_the_formula_or_that_a_diagram_stands_there() {
        assert_eq!(
            alt_of(&Figure::Math {
                tex: "x^2",
                display: true
            }),
            "x^2"
        );
        let diagram = Figure::Diagram {
            source: "graph TD; A-->B",
            kind: splat_markdown::doc::DiagramKind::Mermaid,
        };
        assert_eq!(alt_of(&diagram), splat_i18n::t("Diagramm"));
    }

    #[test]
    fn decoded_pixels_become_a_png_of_the_same_size() {
        let pixels = [255, 0, 0, 255].repeat(6);
        let png = png_of_pixels(3, 2, pixels).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (3, 2));
        assert_eq!(image.get_pixel(2, 1).0, [255, 0, 0, 255]);
        assert!(png_of_pixels(3, 2, vec![0; 5]).is_none(), "too few pixels");
    }

    #[test]
    fn a_drawn_picture_becomes_a_png_at_its_scale_and_a_huge_one_none() {
        let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(3.0, 2.0));
        let png = png_of_image(&image, NSSize::new(3.0, 2.0), 2.0).unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (6, 4));
        assert!(png_of_image(&image, NSSize::new(0.0, 2.0), 2.0).is_none());
        assert!(png_of_image(&image, NSSize::new(9_000.0, 9_000.0), 1.0).is_none());
    }

    #[test]
    fn an_svg_goes_in_as_it_is_and_web_or_foreign_pictures_stay_out() {
        let dir = ide_core::testing::scratch("embed");
        let docs = dir.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"/>";
        std::fs::write(docs.join("kreis.svg"), svg).unwrap();
        std::fs::write(dir.join("fremd.svg"), svg).unwrap();
        let inside = embedded("kreis.svg", Some(&docs));
        let outside = embedded("../fremd.svg", Some(&docs));
        let missing = embedded("fehlt.svg", Some(&docs));
        assert_eq!(inside, Some(data_url("image/svg+xml", svg.as_bytes())));
        assert_eq!(outside, None, "outside the document's folder");
        assert_eq!(missing, None);
        assert_eq!(embedded("https://example.org/a.png", Some(&docs)), None);
    }
}
