//! A Markdown document as PDF: the preview's text set for paper in the light appearance. The layout runs on the
//! export's thread, the pictures load as the preview loads them, the text is set on the main thread a few blocks at a
//! time, and the pages are laid out and drawn into the file on the export's thread again.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use block2::StackBlock;
use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSGraphicsContext, NSLayoutManager, NSPrintInfo, NSTextContainer, NSTextStorage,
};
use objc2_core_foundation::{CFRetained, CFString, CFURL, CFURLPathStyle, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGContext, CGPDFContextBeginPage, CGPDFContextClose, CGPDFContextCreateWithURL, CGPDFContextEndPage,
};
use objc2_foundation::{NSDictionary, NSMutableAttributedString, NSPoint, NSRange, NSSize};
use splat_i18n::t;
use splat_preview::layout::{Atom, Layout, ParaKind, RunMap, layout};
use splat_render::images::{ImageFile, ImageStore};
use splat_render::js::{self, Key, Ready};
use splat_render::picture::Rendered;
use splat_render::remote;
use splat_render::render::{Drawn, RenderCtx};

use super::pictures::{PRINT_SCALE, may_carry_along};
use super::{Job, Outcome, find, finish};
use ide_ui_common::constant;
use ide_ui_common::util::{main_after, main_soon};

/// The page margins, in points.
const MARGIN: f64 = 56.0;

/// Blocks set as text per turn of the main thread, so a long document does not hold it up.
const BLOCKS_PER_TURN: usize = 20;

/// How long the export waits for the pictures of the document's folder; one not read by then is left out.
const PICTURE_WAIT: Duration = Duration::from_secs(15);

/// What a PDF export holds on the main thread while it sets the text.
pub struct Work {
    job: Job,
    laid: Layout,
    macros: Arc<str>,
    images: RefCell<ImageStore>,
    /// The picture files asked for and not read yet.
    pending: RefCell<HashSet<PathBuf>>,
    text: Retained<NSMutableAttributedString>,
    next_block: std::cell::Cell<usize>,
    paper: NSSize,
}

impl Work {
    /// A picture file the export asked for was read.
    pub fn picture_read(&self, path: PathBuf, file: ImageFile) {
        self.pending.borrow_mut().remove(&path);
        self.images.borrow_mut().finish(path, file);
    }
}

/// Lays the document out on the export's thread, then goes on on the main thread.
pub fn start(number: u64, job: Job) {
    let text = job.source.text.clone();
    let spawned = std::thread::Builder::new().name("export".into()).spawn(move || {
        let doc = splat_markdown::reader::parse(&text);
        let macros = js::wanted(&doc, false).macros;
        let ready = Ready {
            dark: false,
            macros: macros.clone(),
        };
        let laid = layout(&doc, &ready, None);
        main_soon(move || laid_out(number, job, laid, macros));
    });
    if spawned.is_err() {
        finish(number, failed());
    }
}

fn failed() -> Outcome {
    Outcome::Failed(t("Das PDF ließ sich nicht schreiben.").to_string())
}

/// Asks for the pictures of the document's folder, which arrive through the export's viewer.
fn laid_out(number: u64, job: Job, laid: Layout, macros: Arc<str>) {
    let Some(run) = find(number) else { return };
    // SAFETY: An empty dictionary, which gives the defaults of the user's paper.
    let info = unsafe { NSPrintInfo::initWithDictionary(NSPrintInfo::alloc(), &NSDictionary::new()) };
    let work = Work {
        laid,
        macros,
        images: RefCell::new(ImageStore::default()),
        pending: RefCell::new(HashSet::new()),
        text: NSMutableAttributedString::new(),
        next_block: std::cell::Cell::new(0),
        paper: info.paperSize(),
        job,
    };
    if let Some(viewer) = run.viewer.get() {
        let dir = work.job.source.doc_dir.as_deref();
        for url in local_pictures(&work.laid) {
            let Some(path) = splat_render::images::resolve_path(&url, dir).filter(|p| may_carry_along(p, dir)) else {
                continue;
            };
            if work.images.borrow_mut().file(&url, dir, false, Some(viewer)).is_none() {
                work.pending.borrow_mut().insert(path);
            }
        }
    }
    *run.pdf.borrow_mut() = Some(work);
    wait_for_pictures(number, Instant::now() + PICTURE_WAIT);
}

fn local_pictures(laid: &Layout) -> Vec<String> {
    laid.blocks
        .iter()
        .flat_map(|block| block.paras.iter())
        .flat_map(|para| para.runs.iter())
        .filter_map(|run| match &run.map {
            RunMap::Atom {
                atom: Atom::Image(image),
                ..
            } if !remote::is_remote(&image.url) => Some(image.url.to_string()),
            _ => None,
        })
        .collect()
}

fn wait_for_pictures(number: u64, until: Instant) {
    let Some(run) = find(number) else { return };
    let loading = run
        .pdf
        .borrow()
        .as_ref()
        .is_some_and(|work| !work.pending.borrow().is_empty());
    if loading && Instant::now() < until {
        main_after(Duration::from_millis(50), move || wait_for_pictures(number, until));
    } else {
        main_soon(move || set_text(number));
    }
}

/// Sets the next few blocks as text; once all are set, the pages are drawn on the export's thread.
fn set_text(number: u64) {
    let Some(run) = find(number) else { return };
    let Some(mtm) = MainThreadMarker::new() else { return };
    let done = {
        let work = run.pdf.borrow();
        let Some(work) = work.as_ref() else { return };
        let Some(viewer) = run.viewer.get() else { return };
        let width = work.paper.width - 2.0 * MARGIN;
        let drawn = Paper {
            trusted: work.job.source.trusted,
        };
        let ctx = RenderCtx {
            mtm,
            zoom: PRINT_SCALE,
            doc_dir: work.job.source.doc_dir.as_deref(),
            images: &work.images,
            max_image_width: width - 20.0,
            dark: false,
            requester: Some(viewer),
            leaves_the_mac: Some(may_carry_along),
            macros: work.macros.clone(),
            drawn: &drawn,
        };
        let from = work.next_block.get();
        let to = (from + BLOCKS_PER_TURN).min(work.laid.blocks.len());
        // The summary of the metadata is there to open its form, and paper has no use for it, as in splat.
        let on_paper = work.laid.blocks[from..to].iter().filter(|block| {
            !block
                .paras
                .iter()
                .all(|para| matches!(para.style.kind, ParaKind::FrontMatter))
        });
        for block in on_paper {
            work.text.appendAttributedString(&ctx.render_block(block));
        }
        work.next_block.set(to);
        to == work.laid.blocks.len()
    };
    if !done {
        main_soon(move || set_text(number));
        return;
    }
    let Some(work) = run.pdf.borrow_mut().take() else {
        return;
    };
    let Work { job, text, paper, .. } = work;
    let handed = Handed(text);
    let spawned = std::thread::Builder::new().name("export".into()).spawn(move || {
        let outcome = draw(handed, paper, &job);
        main_soon(move || finish(number, outcome));
    });
    if spawned.is_err() {
        finish(number, failed());
    }
}

/// The text of the pages, handed from the main thread to the export's thread.
struct Handed(Retained<NSMutableAttributedString>);

// SAFETY: The main thread gives up its only reference when it hands the text over and never touches it again; from
// then on only the export's thread reads it, and the pictures in it are only drawn, which AppKit allows anywhere.
unsafe impl Send for Handed {}

/// Lays the text out page by page and draws the pages into the export's file; on the export's thread.
fn draw(text: Handed, paper: NSSize, job: &Job) -> Outcome {
    let size = NSSize::new(paper.width - 2.0 * MARGIN, paper.height - 2.0 * MARGIN);
    let storage = NSTextStorage::new();
    storage.setAttributedString(&text.0);
    drop(text);
    let layout = NSLayoutManager::new();
    storage.addLayoutManager(&layout);
    let mut pages: Vec<(Retained<NSTextContainer>, NSRange)> = Vec::new();
    loop {
        if job.cancelled.load(Ordering::SeqCst) {
            return Outcome::Cancelled;
        }
        let container = NSTextContainer::initWithSize(NSTextContainer::alloc(), size);
        container.setLineFragmentPadding(0.0);
        layout.addTextContainer(&container);
        let glyphs = layout.glyphRangeForTextContainer(&container);
        let end = glyphs.location + glyphs.length;
        pages.push((container, glyphs));
        // A piece too large for any page ends the document rather than asking for pages without end.
        if end >= layout.numberOfGlyphs() || glyphs.length == 0 {
            break;
        }
    }
    let path = CFString::from_str(&job.path.to_string_lossy());
    let Some(url) = CFURL::with_file_system_path(None, Some(&path), CFURLPathStyle::CFURLPOSIXPathStyle, false) else {
        return failed();
    };
    let media = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(paper.width, paper.height));
    // SAFETY: A file URL, a rectangle that lives across the call and no auxiliary information.
    let Some(pdf) = (unsafe { CGPDFContextCreateWithURL(Some(&url), &media, None) }) else {
        return failed();
    };
    let outcome = draw_pages(&pdf, &layout, &pages, paper, job);
    CGPDFContextClose(Some(&pdf));
    if outcome != Outcome::Written {
        let _ = std::fs::remove_file(&job.path);
    }
    outcome
}

fn draw_pages(
    pdf: &CFRetained<CGContext>,
    layout: &NSLayoutManager,
    pages: &[(Retained<NSTextContainer>, NSRange)],
    paper: NSSize,
    job: &Job,
) -> Outcome {
    let Some(aqua) = NSAppearance::appearanceNamed(constant!(NSAppearanceNameAqua)) else {
        return failed();
    };
    let graphics = NSGraphicsContext::graphicsContextWithCGContext_flipped(pdf, true);
    let origin = NSPoint::new(MARGIN, MARGIN);
    for (_, glyphs) in pages {
        if job.cancelled.load(Ordering::SeqCst) {
            return Outcome::Cancelled;
        }
        // SAFETY: A context the export made, between the opening and closing of its file, with no page information.
        unsafe { CGPDFContextBeginPage(Some(pdf), None) };
        CGContext::save_g_state(Some(pdf));
        // Text lays out from the top down, PDF pages count from the bottom up.
        CGContext::translate_ctm(Some(pdf), 0.0, paper.height);
        CGContext::scale_ctm(Some(pdf), 1.0, -1.0);
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&graphics));
        let page = StackBlock::new(|| {
            ide_ui_common::util::guarded((), || {
                layout.drawBackgroundForGlyphRange_atPoint(*glyphs, origin);
                layout.drawGlyphsForGlyphRange_atPoint(*glyphs, origin);
            })
        });
        // The colours of the text follow the light appearance, whatever the app shows.
        aqua.performAsCurrentDrawingAppearance(&page);
        NSGraphicsContext::restoreGraphicsState_class();
        CGContext::restore_g_state(Some(pdf));
        CGPDFContextEndPage(Some(pdf));
    }
    Outcome::Written
}

/// What paper gets from others: what the renderer drew, and pictures from the web already loaded, only where the
/// folder is trusted (E45).
struct Paper {
    trusted: bool,
}

impl Drawn for Paper {
    fn web_picture(&self, url: &str) -> Option<Rendered> {
        if self.trusted {
            remote::get(url, None).flatten()
        } else {
            None
        }
    }

    fn drawing(&self, key: &Key) -> Option<Rendered> {
        js::cached(key)
    }
}
