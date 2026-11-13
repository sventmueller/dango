//! The split view under a window tab's documents, which says when the user has dragged its divider.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSSplitView};

use crate::util::guarded;

pub struct AreaIvars {
    dragged: RefCell<Option<Box<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSSplitView has no subclassing requirements and `AreaIvars` does not implement Drop.
    #[unsafe(super(NSSplitView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEDocumentArea"]
    #[ivars = AreaIvars]
    pub struct DocumentArea;

    unsafe impl NSObjectProtocol for DocumentArea {}

    impl DocumentArea {
        /// A press that reaches the split view itself is on its divider, which it tracks until the button comes up.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let () = unsafe { msg_send![super(self), mouseDown: event] };
            guarded((), || {
                if let Some(dragged) = self.ivars().dragged.borrow().as_ref() {
                    dragged();
                }
            });
        }
    }
);

impl DocumentArea {
    pub fn new(mtm: MainThreadMarker) -> Retained<DocumentArea> {
        let this = Self::alloc(mtm).set_ivars(AreaIvars {
            dragged: RefCell::new(None),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// Calls `dragged` each time the user lets go of the divider.
    pub fn on_drag(&self, dragged: impl Fn() + 'static) {
        *self.ivars().dragged.borrow_mut() = Some(Box::new(dragged));
    }
}
