//! The self-test's hooks of `pdf.rs`.

use super::*;

/// The number of pages the PDF tab `view` shows, for the self-test.
pub fn page_count(view: &NSView) -> usize {
    // SAFETY: Only sent to a PDFView, whose document answers its page count.
    unsafe {
        let document: Option<Retained<AnyObject>> = msg_send![view, document];
        document.map_or(0, |document| msg_send![&*document, pageCount])
    }
}
