//! Where a line of the editor lies: positions TextKit 2 gives only for text it has laid out, so the paragraphs asked
//! about are laid out first.

use objc2::DefinedClass;
use objc2_foundation::{NSPoint, NSRect};

use super::{EditorTextView, highlight, insets, line_rect};

/// The byte offset where the first line on screen starts.
pub fn top_byte(view: &EditorTextView) -> Option<usize> {
    let line = highlight::line_at(view, NSPoint::new(0.0, view.visibleRect().origin.y))?;
    view.document().text().offset(line, 0, ide_document::Unit::Utf16).ok()
}

/// The top of the line holding byte `byte`, in the view's coordinates.
pub fn byte_y(view: &EditorTextView, byte: usize) -> Option<f64> {
    lay_out_whole(view, &*view.textLayoutManager()?, &*view.textContentStorage()?);
    byte_y_near(view, byte)
}

/// `byte_y` for a line beside those on screen, which TextKit 2 places as it placed them without laying out all text.
pub fn byte_y_near(view: &EditorTextView, byte: usize) -> Option<f64> {
    let at = view.document().text().byte_to_utf16(byte).ok()?;
    let layout = view.textLayoutManager()?;
    let content = view.textContentStorage()?;
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let location = content.locationFromLocation_withOffset(&start, isize::try_from(at).unwrap_or(isize::MAX))?;
    ensure_paragraph(&layout, &location);
    let fragment = layout.textLayoutFragmentForLocation(&location)?;
    Some(fragment.layoutFragmentFrame().origin.y + view.textContainerOrigin().y)
}

/// Lays out the whole text of `view`, once per revision and size of the view. TextKit 2 places a fragment by estimated
/// heights until all text is laid out, and a jump would land beside it.
fn lay_out_whole(
    view: &EditorTextView,
    layout: &objc2_app_kit::NSTextLayoutManager,
    content: &objc2_app_kit::NSTextContentStorage,
) {
    let revision = view.document().revision();
    if view.ivars().laid_out.get() == Some((revision, view.frame().size)) {
        return;
    }
    layout.ensureLayoutForRange(&objc2_app_kit::NSTextElementProvider::documentRange(content));
    view.ivars().laid_out.set(Some((revision, view.frame().size)));
}

/// Lays out the paragraph at `location`. A paragraph TextKit has not laid out yet answers with a fragment of its
/// estimate, without line fragments, which a later edit trips over as one of no text
/// ("NSTextLayoutManagerBreakOnNilContentManager").
pub(super) fn ensure_paragraph(
    layout: &objc2_app_kit::NSTextLayoutManager,
    location: &objc2::runtime::ProtocolObject<dyn objc2_app_kit::NSTextLocation>,
) {
    if let Some(range) = objc2_app_kit::NSTextRange::initWithLocation_endLocation(
        <objc2_app_kit::NSTextRange as objc2::AllocAnyThread>::alloc(),
        location,
        Some(location),
    ) {
        layout.ensureLayoutForRange(&range);
    }
}

/// `line_rect` of a line that may lie outside what TextKit laid out, such as the paused line of a debugger, laying
/// its paragraph out first.
pub fn laid_out_line_rect(editor: &EditorTextView, line: usize) -> Option<NSRect> {
    let at = editor.line_start(line)?;
    let (layout, content) = (editor.textLayoutManager()?, editor.textContentStorage()?);
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let location = content.locationFromLocation_withOffset(&start, isize::try_from(at).ok()?)?;
    let fragment = layout.textLayoutFragmentForLocation(&location)?;
    if insets::line_fragments(&fragment).is_none() {
        ensure_paragraph(&layout, &location);
    }
    line_rect(editor, line)
}
