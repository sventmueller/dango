//! A view over the text for what TextKit does not draw: the placeholders of folds and the carets and selections
//! beyond the first. Clicks pass through to the text.

use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSStringDrawing, NSView, NSWindowOrderingMode,
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

    impl Overlay {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> Option<Retained<NSView>> {
            None
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            guarded((), || self.draw());
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

/// Draws the overlay of `editor` again.
pub fn redraw(editor: &EditorTextView) {
    if let Some(overlay) = editor.ivars().overlay.borrow().as_ref().and_then(Weak::load) {
        overlay.setNeedsDisplay(true);
    }
}

impl Overlay {
    fn draw(&self) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        self.draw_placeholders(&editor);
        self.draw_carets(&editor);
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
