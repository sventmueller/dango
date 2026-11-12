//! The editor's layout fragments: TextKit 2 lays out and draws each paragraph as one, and these draw the insets of
//! E81 that fall in their paragraph right after its text. Drawn with the text, the insets scroll and render with it;
//! a view of their own over the text fell behind TextKit's scrolling and showed the gaps without them.

use objc2::rc::{Retained, Weak};
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSTextElement, NSTextLayoutFragment, NSTextLayoutManager, NSTextLayoutManagerDelegate, NSTextLocation,
};
use objc2_core_graphics::CGContext;
use objc2_foundation::{NSObjectProtocol, NSRect};

use super::view::EditorTextView;
use crate::util::guarded;

pub struct FragmentIvars {
    editor: Weak<EditorTextView>,
}

define_class!(
    // SAFETY: NSTextLayoutFragment may be subclassed to draw more than its text; `InsetFragment` does not implement
    // Drop.
    #[unsafe(super(NSTextLayoutFragment, NSObject))]
    #[name = "IDEInsetFragment"]
    #[ivars = FragmentIvars]
    pub struct InsetFragment;

    unsafe impl NSObjectProtocol for InsetFragment {}

    impl InsetFragment {
        #[unsafe(method(drawAtPoint:inContext:))]
        fn draw_at(&self, point: objc2_foundation::NSPoint, context: &CGContext) {
            // SAFETY: The superclass's implementation, with this method's signature.
            let _: () = unsafe { msg_send![super(self), drawAtPoint: point, inContext: context] };
            if MainThreadMarker::new().is_some() {
                guarded((), || {
                    if let Some(editor) = self.ivars().editor.load() {
                        super::insets::draw_fragment(&editor, self, point, context);
                    }
                });
            }
        }

        #[unsafe(method(renderingSurfaceBounds))]
        fn rendering_surface_bounds(&self) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let bounds: NSRect = unsafe { msg_send![super(self), renderingSurfaceBounds] };
            if MainThreadMarker::new().is_none() {
                return bounds;
            }
            let extra = guarded(0.0, || {
                self.ivars()
                    .editor
                    .load()
                    .map_or(0.0, |editor| super::insets::width_after_text(&editor, self))
            });
            NSRect::new(
                bounds.origin,
                objc2_foundation::NSSize::new(bounds.size.width + extra, bounds.size.height),
            )
        }
    }
);

pub struct LayoutIvars {
    editor: Weak<EditorTextView>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `InsetLayout` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEInsetLayout"]
    #[ivars = LayoutIvars]
    pub struct InsetLayout;

    unsafe impl NSObjectProtocol for InsetLayout {}

    unsafe impl NSTextLayoutManagerDelegate for InsetLayout {
        #[unsafe(method_id(textLayoutManager:textLayoutFragmentForLocation:inTextElement:))]
        fn fragment_for(
            &self,
            _manager: &NSTextLayoutManager,
            _location: &ProtocolObject<dyn NSTextLocation>,
            element: &NSTextElement,
        ) -> Retained<NSTextLayoutFragment> {
            let this = InsetFragment::alloc().set_ivars(FragmentIvars {
                editor: self.ivars().editor.clone(),
            });
            // SAFETY: NSTextLayoutFragment's initializer for the whole element, called once on the newly allocated
            // object.
            let fragment: Retained<InsetFragment> =
                unsafe { msg_send![super(this), initWithTextElement: element, range: None::<&NSObject>] };
            Retained::into_super(fragment)
        }
    }
);

/// Has the text layout of `editor` make its fragments as `InsetFragment`s. The layout manager holds its delegate
/// weakly, so the delegate is returned for the editor to keep.
pub fn attach(editor: &EditorTextView) -> Option<Retained<InsetLayout>> {
    let layout = editor.textLayoutManager()?;
    let delegate = InsetLayout::alloc(editor.mtm()).set_ivars(LayoutIvars {
        editor: Weak::from(editor),
    });
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let delegate: Retained<InsetLayout> = unsafe { msg_send![super(delegate), init] };
    layout.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    Some(delegate)
}
