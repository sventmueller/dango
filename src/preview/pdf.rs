//! PDF files in a tab of their own, shown by PDFKit, which parses them in the app as Preview.app does.

use std::path::Path;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{MainThreadMarker, MainThreadOnly, msg_send};
use objc2_app_kit::{NSAccessibility, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSURL};
use splat_i18n::{t, tf};

use crate::util::ns;
use crate::window::{TabKind, WindowController};

// PDFKit has no crate of its own here; its classes are looked up by name once the framework is loaded.
#[link(name = "PDFKit", kind = "framework")]
unsafe extern "C" {}

pub fn is_pdf(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Opens `path` in a PDF tab of `controller`, or in its preview tab, or selects the tab that shows it.
pub fn open(controller: &WindowController, path: &Path, preview: bool) {
    let key = path.to_string_lossy().into_owned();
    if let Some(id) = controller.tab_with_key(&key) {
        if !preview {
            controller.keep_tab(id);
        }
        controller.select_tab_by_id(id);
        return;
    }
    let name = crate::editor::display_name(path);
    let Some(view) = pdf_view(controller.mtm(), path, &name) else {
        crate::editor::show_alert(
            controller.mtm(),
            &tf("„{}“ lässt sich nicht öffnen.", &[&name]),
            t("Die Datei ist kein lesbares PDF."),
        );
        return;
    };
    if preview {
        controller.add_preview_tab(TabKind::Preview, Some(key), &name, view);
    } else {
        controller.add_tab(TabKind::Preview, Some(key), &name, view);
    }
}

/// A `PDFView` showing the file, scaled to its width; `None` when PDFKit cannot read it.
fn pdf_view(_mtm: MainThreadMarker, path: &Path, name: &str) -> Option<Retained<NSView>> {
    let document_class = AnyClass::get(c"PDFDocument")?;
    let view_class = AnyClass::get(c"PDFView")?;
    let url = NSURL::fileURLWithPath(&ns(&path.to_string_lossy()));
    // SAFETY: PDFDocument's class answers `alloc`, and `initWithURL:` takes a file URL and answers nil for a file
    // that is no PDF.
    let document: Option<Retained<AnyObject>> = unsafe {
        let allocated: Allocated<AnyObject> = msg_send![document_class, alloc];
        msg_send![allocated, initWithURL: &*url]
    };
    let document = document?;
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
    // SAFETY: PDFView is an NSView, made on the main thread, which `_mtm` proves, with NSView's initializer.
    let view: Retained<NSView> = unsafe {
        let allocated: Allocated<NSView> = msg_send![view_class, alloc];
        msg_send![allocated, initWithFrame: frame]
    };
    // SAFETY: PDFView's own setters, with a flag and a PDFDocument.
    unsafe {
        let _: () = msg_send![&*view, setAutoScales: true];
        let _: () = msg_send![&*view, setDocument: &*document];
    }
    view.setAccessibilityLabel(Some(&ns(&tf("PDF: {}", &[name]))));
    Some(view)
}

/// The number of pages the PDF tab `view` shows, for the self-test.
#[cfg(feature = "selftest")]
pub fn page_count(view: &NSView) -> usize {
    // SAFETY: Only sent to a PDFView, whose document answers its page count.
    unsafe {
        let document: Option<Retained<AnyObject>> = msg_send![view, document];
        document.map_or(0, |document| msg_send![&*document, pageCount])
    }
}
