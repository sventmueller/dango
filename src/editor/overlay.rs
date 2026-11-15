//! A view over the text for what TextKit does not draw: the placeholders of folds and the carets and selections beyond
//! the first. It also knows where the insets of E81 are, which the layout fragments draw, for their clicks, tooltips
//! and pointer. Clicks pass through to the text, except on an inset that does something.

use std::cell::RefCell;

use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAffineTransformNSAppKitAdditions, NSBezierPath, NSColor, NSCursor, NSEvent, NSFont,
    NSFontAttributeName, NSForegroundColorAttributeName, NSStringDrawing, NSToolTipTag, NSView, NSViewToolTipOwner,
    NSWindowOrderingMode,
};
use objc2_foundation::{NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::view::EditorTextView;
use crate::util::{constant, guarded, ns};

pub(crate) struct OverlayIvars {
    editor: Weak<EditorTextView>,
    /// The insets its tooltip areas and pointer are for: where each is, in the overlay's coordinates, whether it has a
    /// tooltip and whether a click on it does something.
    offered: RefCell<Vec<(NSRect, bool, bool)>>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Overlay` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEEditorOverlay"]
    #[ivars = OverlayIvars]
    pub(crate) struct Overlay;

    unsafe impl NSObjectProtocol for Overlay {}

    unsafe impl NSViewToolTipOwner for Overlay {
        #[unsafe(method_id(view:stringForToolTip:point:userData:))]
        unsafe fn tool_tip(
            &self,
            _view: &NSView,
            _tag: NSToolTipTag,
            point: NSPoint,
            _data: *mut std::ffi::c_void,
        ) -> Retained<NSString> {
            let text = guarded(None, || {
                let editor = self.ivars().editor.load()?;
                super::insets::tooltip(&editor, self.convertPoint_toView(point, Some(&editor)))
            });
            ns(&text.unwrap_or_default())
        }
    }

    impl Overlay {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> Option<Retained<NSView>> {
            let hit = guarded(false, || {
                self.ivars()
                    .editor
                    .load()
                    .is_some_and(|editor| super::insets::hits(&editor, point))
            });
            hit.then(|| self.retain().into_super())
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            guarded((), || {
                if let Some(editor) = self.ivars().editor.load() {
                    let point = editor.convertPoint_fromView(event.locationInWindow(), None);
                    super::insets::click(&editor, point);
                }
            });
        }

        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            guarded((), || self.point_at_insets());
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            guarded((), || self.draw(dirty));
        }
    }
);

/// Puts an overlay over the part of `editor` on screen. It has a layer of its own: without one it would draw into the
/// text view's layer, under the layers TextKit 2 draws the text in. Covering only what is on screen keeps that layer
/// small in a long text.
pub(crate) fn attach(editor: &EditorTextView) {
    let this = Overlay::alloc(editor.mtm()).set_ivars(OverlayIvars {
        editor: Weak::from(editor),
        offered: RefCell::new(Vec::new()),
    });
    // SAFETY: NSView's initializer, called once on the newly allocated object.
    let this: Retained<Overlay> = unsafe { msg_send![super(this), initWithFrame: editor.visibleRect()] };
    this.setWantsLayer(true);
    this.setAccessibilityElement(false);
    editor.addSubview_positioned_relativeTo(&this, NSWindowOrderingMode::Above, None);
    *editor.ivars().around.overlay.borrow_mut() = Some(Weak::from(&*this));
}

/// How often TextKit told an editor it laid out the lines on screen, for the self-test.
#[cfg(feature = "selftest")]
pub(crate) static LAID_OUT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// TextKit laid out the lines on screen: the overlay moves over them and draws again, as what it draws follows the
/// layout.
pub(crate) fn laid_out(editor: &EditorTextView) {
    #[cfg(feature = "selftest")]
    LAID_OUT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if let Some(overlay) = editor.ivars().around.overlay.borrow().as_ref().and_then(Weak::load) {
        let subviews = editor.subviews();
        let on_top = subviews
            .lastObject()
            .is_some_and(|top| std::ptr::eq(&*top, &**overlay as &NSView));
        // TextKit may put views of its own over the overlay as it lays out, which would hide what the overlay draws.
        if !on_top {
            editor.addSubview_positioned_relativeTo(&overlay, NSWindowOrderingMode::Above, None);
        }
    }
    follow(editor);
}

/// Moves the overlay of `editor` and that of its diagnostics over the part on screen and draws it there.
pub(crate) fn follow(editor: &EditorTextView) {
    ide_ui_lsp::follow_squiggles(editor);
    if let Some(overlay) = editor.ivars().around.overlay.borrow().as_ref().and_then(Weak::load) {
        let seen = editor.visibleRect();
        if overlay.frame() != seen {
            overlay.setFrame(seen);
            if let Some(window) = overlay.window() {
                window.invalidateCursorRectsForView(&overlay);
            }
        }
        overlay.setNeedsDisplay(true);
    }
}

/// Draws the overlay of `editor` again.
pub(crate) fn redraw(editor: &EditorTextView) {
    if let Some(overlay) = editor.ivars().around.overlay.borrow().as_ref().and_then(Weak::load) {
        overlay.setNeedsDisplay(true);
    }
}

impl Overlay {
    fn draw(&self, dirty: NSRect) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        // What follows draws in the editor's coordinates, of which the overlay shows the part on screen.
        let origin = self.frame().origin;
        let shift = objc2_foundation::NSAffineTransform::transform();
        shift.translateXBy_yBy(-origin.x, -origin.y);
        shift.concat();
        crate::debug::draw_paused_line(&editor);
        self.draw_placeholders(&editor);
        super::insets::draw(&editor, self.convertRect_toView(dirty, Some(&editor)));
        self.draw_carets(&editor);
        super::vim::draw_block(&editor);
        self.offer_tooltips(&editor);
    }

    /// Gives every inset with a tooltip a tooltip area, which the overlay answers from the inset under the mouse; only
    /// when the insets or their places changed since the last drawing.
    fn offer_tooltips(&self, editor: &EditorTextView) {
        let offers: Vec<(NSRect, bool, bool)> = super::insets::offers(editor)
            .into_iter()
            .map(|(rect, tooltip, action)| (self.convertRect_fromView(rect, Some(editor)), tooltip, action))
            .collect();
        if *self.ivars().offered.borrow() == offers {
            return;
        }
        self.removeAllToolTips();
        for (rect, _, _) in offers.iter().filter(|(_, tooltip, _)| *tooltip) {
            // SAFETY: The overlay owns its tooltip areas and answers them as their owner; no user data is passed.
            unsafe { self.addToolTipRect_owner_userData(*rect, self, std::ptr::null_mut()) };
        }
        *self.ivars().offered.borrow_mut() = offers;
        if let Some(window) = self.window() {
            window.invalidateCursorRectsForView(self);
        }
    }

    /// The pointing hand over every inset a click does something on.
    fn point_at_insets(&self) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        for (rect, _, _) in super::insets::offers(&editor)
            .into_iter()
            .filter(|(_, _, action)| *action)
        {
            self.addCursorRect_cursor(
                self.convertRect_fromView(rect, Some(&editor)),
                &NSCursor::pointingHandCursor(),
            );
        }
    }

    /// A rounded box with an ellipsis after the first line of every fold on screen.
    fn draw_placeholders(&self, editor: &EditorTextView) {
        let size = editor.ivars().font_size.get() * 0.9;
        let attributes = NSDictionary::<NSString, objc2::runtime::AnyObject>::from_retained_objects(
            &[
                constant!(NSFontAttributeName),
                constant!(NSForegroundColorAttributeName),
            ],
            &[
                NSFont::systemFontOfSize(size).into(),
                NSColor::secondaryLabelColor().into(),
            ],
        );
        for (rect, _) in super::fold::placeholders(editor) {
            NSColor::quaternaryLabelColor().setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 4.0, 4.0).fill();
            let dots = ns("…");
            // SAFETY: The attributes hold a font and a colour, as the method expects.
            let width = unsafe { dots.sizeWithAttributes(Some(&attributes)) }.width;
            let at = NSPoint::new(rect.origin.x + (rect.size.width - width) / 2.0, rect.origin.y - 1.0);
            // SAFETY: These attributes too hold only a font and a colour under AppKit's keys.
            unsafe { dots.drawAtPoint_withAttributes(at, Some(&attributes)) };
        }
    }

    /// The selections beyond the first as tinted areas, and their carets as lines.
    fn draw_carets(&self, editor: &EditorTextView) {
        let mut carets = super::multi::extra(editor);
        carets.retain(|range| !super::fold::hidden_by_fold(editor, range.end));
        if carets.is_empty() {
            return;
        }
        for range in &carets {
            if range.is_empty() {
                continue;
            }
            NSColor::selectedTextBackgroundColor()
                .colorWithAlphaComponent(0.4)
                .setFill();
            for rect in super::multi::rects(editor, range.clone()) {
                NSBezierPath::fillRect(rect);
            }
        }
        editor.insertionPointColor().setFill();
        for range in &carets {
            if let Some(rect) = ide_ui_lsp::rect_of(editor, range.end..range.end) {
                NSBezierPath::fillRect(NSRect::new(rect.origin, NSSize::new(1.5, rect.size.height)));
            }
        }
    }
}
