//! The editor view's own helpers beside its AppKit methods: tracking the mouse, lines, revealed matches and calls
//! later.

#[allow(
    unused_imports,
    reason = "the helpers moved here use only some of their parent's names"
)]
use super::*;

impl EditorTextView {
    /// Hears the mouse move over the visible text while the window is key, for hover texts.
    pub(super) fn track_mouse(&self) {
        let options = NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        let ours = self.trackingAreas().iter().any(|area| {
            area.options().contains(options) && area.owner().is_some_and(|o| std::ptr::eq(&*o, self.as_ref()))
        });
        if ours {
            return;
        }
        // SAFETY: The view owns the area and outlives it; AppKit holds the owner weakly.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                options,
                Some(self),
                None,
            )
        };
        self.addTrackingArea(&area);
    }

    /// The UTF-16 offset of the start of `line`.
    pub fn line_start(&self, line: usize) -> Option<usize> {
        let document = self.document();
        let text = document.text();
        text.offset(line, 0, Unit::Utf16)
            .ok()
            .and_then(|byte| text.byte_to_utf16(byte).ok())
    }

    /// Marks `columns`, in UTF-16 code units, of line `line`, both counted from 0 and kept within the text, in the
    /// find colour and scrolls them into view; with `focus` it selects them and takes the keyboard focus, else the
    /// caret goes before them.
    pub fn select_columns(&self, line: usize, columns: std::ops::Range<usize>, focus: bool) {
        let range = {
            let document = self.document();
            let text = document.text();
            let line = u32::try_from(line.min(text.line_count().saturating_sub(1))).unwrap_or(u32::MAX);
            let at = |column: usize| {
                let column = u32::try_from(column).unwrap_or(u32::MAX);
                text.byte_to_utf16(text.lsp_offset(line, column, Unit::Utf16))
                    .unwrap_or(0)
            };
            let start = at(columns.start);
            NSRange::new(start, at(columns.end).saturating_sub(start))
        };
        self.setSelectedRange(if focus { range } else { NSRange::new(range.location, 0) });
        *self.ivars().found.borrow_mut() = Some(range.location..range.end());
        self.paint_found();
        self.scrollRangeToVisible(range);
        if focus && let Some(window) = self.window() {
            window.makeFirstResponder(Some(self));
        }
    }

    /// Draws the revealed match as the search panel's list does.
    pub(crate) fn paint_found(&self) {
        let Some(found) = self.ivars().found.borrow().clone() else {
            return;
        };
        let (Some(layout), Some(content)) = (self.textLayoutManager(), self.textContentStorage()) else {
            return;
        };
        let Some(range) = ide_ui_lsp::text_range(&content, &found) else {
            return;
        };
        let marker: Retained<AnyObject> = objc2_app_kit::NSColor::findHighlightColor().into();
        let color: Retained<AnyObject> = objc2_app_kit::NSColor::blackColor().into();
        let (marker_key, color_key) = (
            crate::util::constant!(objc2_app_kit::NSBackgroundColorAttributeName),
            crate::util::constant!(objc2_app_kit::NSForegroundColorAttributeName),
        );
        // SAFETY: Both values are colours, as the background and foreground colour keys expect.
        unsafe {
            layout.addRenderingAttribute_value_forTextRange(marker_key, Some(&marker), &range);
            layout.addRenderingAttribute_value_forTextRange(color_key, Some(&color), &range);
        }
    }

    pub(crate) fn clear_found(&self) {
        let Some(found) = self.ivars().found.take() else { return };
        let (Some(layout), Some(content)) = (self.textLayoutManager(), self.textContentStorage()) else {
            return;
        };
        let Some(range) = ide_ui_lsp::text_range(&content, &found) else {
            return;
        };
        layout.removeRenderingAttribute_forTextRange(
            crate::util::constant!(objc2_app_kit::NSBackgroundColorAttributeName),
            &range,
        );
        layout.removeRenderingAttribute_forTextRange(
            crate::util::constant!(objc2_app_kit::NSForegroundColorAttributeName),
            &range,
        );
        if self.ivars().lsp.try_borrow_mut().is_ok() {
            ide_ui_lsp::paint(self);
        } else {
            self.later(objc2::sel!(ideDecorate), 0.0);
        }
    }

    /// Runs `selector` on this view after `delay` seconds, replacing a request for it that has not run yet.
    pub(crate) fn later(&self, selector: Sel, delay: f64) {
        use objc2_foundation::NSObjectNSDelayedPerforming;
        let object: &objc2::runtime::NSObject = self;
        // SAFETY: A selector without arguments that this class answers; the run loop retains the target until the call.
        unsafe {
            objc2::runtime::NSObject::cancelPreviousPerformRequestsWithTarget_selector_object(object, selector, None);
            let modes = objc2_foundation::NSArray::from_slice(&[objc2_foundation::NSRunLoopCommonModes]);
            object.performSelector_withObject_afterDelay_inModes(selector, None, delay, &modes);
        }
    }
}
