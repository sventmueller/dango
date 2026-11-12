//! A view over the text for what TextKit does not draw: the placeholders of folds, the carets and selections beyond
//! the first, and the insets of E81. Clicks pass through to the text, except on an inset that does something.

use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSCursor, NSEvent, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSStringDrawing, NSToolTipTag, NSView, NSViewToolTipOwner, NSWindowOrderingMode,
};
use objc2_foundation::{NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::view::EditorTextView;
use crate::util::{constant, guarded, ns};

pub struct OverlayIvars {
    editor: Weak<EditorTextView>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Overlay` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEEditorOverlay"]
    #[ivars = OverlayIvars]
    pub struct Overlay;

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

/// Puts an overlay over `editor`, following its size.
pub fn attach(editor: &EditorTextView) {
    let this = Overlay::alloc(editor.mtm()).set_ivars(OverlayIvars {
        editor: Weak::from(editor),
    });
    // SAFETY: NSView's initializer, called once on the newly allocated object.
    let this: Retained<Overlay> = unsafe { msg_send![super(this), initWithFrame: editor.bounds()] };
    this.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    this.setAccessibilityElement(false);
    editor.addSubview_positioned_relativeTo(&this, NSWindowOrderingMode::Above, None);
    *editor.ivars().overlay.borrow_mut() = Some(Weak::from(&*this));
}

/// How often TextKit told an editor it laid out the lines on screen, for the self-test.
#[cfg(feature = "selftest")]
pub static LAID_OUT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// TextKit laid out the lines on screen: draws the part of the overlay there again. Scrolling draws the overlay for
/// lines that come into view before TextKit lays them out, and an inset whose line has no layout yet is left out.
pub fn laid_out(editor: &EditorTextView) {
    #[cfg(feature = "selftest")]
    LAID_OUT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if let Some(overlay) = editor.ivars().overlay.borrow().as_ref().and_then(Weak::load) {
        overlay.setNeedsDisplayInRect(overlay.convertRect_fromView(editor.visibleRect(), Some(editor)));
    }
}

/// Draws the overlay of `editor` again.
pub fn redraw(editor: &EditorTextView) {
    if let Some(overlay) = editor.ivars().overlay.borrow().as_ref().and_then(Weak::load) {
        overlay.setNeedsDisplay(true);
    }
}

impl Overlay {
    fn draw(&self, dirty: NSRect) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        self.draw_placeholders(&editor);
        super::insets::draw(&editor, self.convertRect_toView(dirty, Some(&editor)));
        self.draw_carets(&editor);
        self.offer_tooltips(&editor);
    }

    /// Gives every inset with a tooltip a tooltip area, which the overlay answers from the inset under the mouse.
    fn offer_tooltips(&self, editor: &EditorTextView) {
        self.removeAllToolTips();
        let shown = super::insets::shown(editor);
        for inset in &shown {
            if super::insets::tooltip(
                editor,
                NSPoint::new(inset.rect.origin.x + 1.0, inset.rect.origin.y + 1.0),
            )
            .is_some()
            {
                // SAFETY: The overlay owns its tooltip areas and answers them as their owner; no user data is passed.
                unsafe { self.addToolTipRect_owner_userData(inset.rect, self, std::ptr::null_mut()) };
            }
        }
        if let Some(window) = self.window() {
            window.invalidateCursorRectsForView(self);
        }
    }

    /// The pointing hand over every inset a click does something on.
    fn point_at_insets(&self) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        for inset in super::insets::shown(&editor) {
            let middle = NSPoint::new(inset.rect.origin.x + 1.0, inset.rect.origin.y + 1.0);
            if super::insets::hits(&editor, middle) {
                self.addCursorRect_cursor(inset.rect, &NSCursor::pointingHandCursor());
            }
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
            // SAFETY: As above.
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
            if let Some(rect) = crate::lsp::rect_of(editor, range.end..range.end) {
                NSBezierPath::fillRect(NSRect::new(rect.origin, NSSize::new(1.5, rect.size.height)));
            }
        }
    }
}
