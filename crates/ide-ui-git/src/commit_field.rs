//! The commit message: plain text over several lines, where ⌘↩ commits.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSBezierPath, NSEvent, NSEventModifierFlags, NSTextView};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::panel::GitPanel;
use ide_ui_common::util::guarded;

/// Return and the Return of the number block.
const RETURN_KEYS: [u16; 2] = [36, 76];

/// The corner the button that asks a model for the message covers, which the text flows around.
const BUTTON_CORNER: NSSize = NSSize::new(20.0, 18.0);

define_class!(
    // SAFETY: NSTextView has no subclassing requirements and `CommitTextView` does not implement Drop.
    #[unsafe(super(NSTextView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitCommitTextView"]
    pub(crate) struct CommitTextView;

    unsafe impl NSObjectProtocol for CommitTextView {}

    impl CommitTextView {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let handled = guarded(false, || {
                let command = event.modifierFlags().contains(NSEventModifierFlags::Command);
                if !(command && RETURN_KEYS.contains(&event.keyCode())) {
                    return false;
                }
                if let Some(panel) = self.panel() {
                    panel.commit();
                }
                true
            });
            if !handled {
                // SAFETY: The superclass's implementation, with this method's signature.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: The superclass's implementation, with this method's signature.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            guarded((), || self.keep_corner_free());
        }

        // Tab moves on to the list of changes, as in a text field; ⌥Tab still types a tab.
        #[unsafe(method(insertTab:))]
        fn insert_tab(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(window) = self.window() {
                    window.selectNextKeyView(Some(self));
                }
            });
        }

        #[unsafe(method(insertBacktab:))]
        fn insert_backtab(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(window) = self.window() {
                    window.selectPreviousKeyView(Some(self));
                }
            });
        }
    }
);

impl CommitTextView {
    pub(crate) fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<CommitTextView> {
        // SAFETY: NSTextView's designated initializer, called once on the newly allocated object.
        let view: Retained<CommitTextView> = unsafe { msg_send![CommitTextView::alloc(mtm), initWithFrame: frame] };
        view.setRichText(false);
        view.setAllowsUndo(true);
        view.setAutomaticQuoteSubstitutionEnabled(false);
        view.setAutomaticDashSubstitutionEnabled(false);
        view.setAutomaticTextReplacementEnabled(false);
        view.setVerticallyResizable(true);
        view.setHorizontallyResizable(false);
        view.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
        view.setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable);
        view
    }

    /// Keeps the text out of the top far corner, where the button that asks a model for the message sits.
    fn keep_corner_free(&self) {
        // SAFETY: A text view made by `new` always has its container; nothing here keeps it beyond this call.
        let Some(container) = (unsafe { self.textContainer() }) else {
            return;
        };
        let width = container.size().width;
        let rtl = self.userInterfaceLayoutDirection() == objc2_app_kit::NSUserInterfaceLayoutDirection::RightToLeft;
        let x = if rtl {
            0.0
        } else {
            (width - BUTTON_CORNER.width).max(0.0)
        };
        let corner = NSBezierPath::bezierPathWithRect(NSRect::new(NSPoint::new(x, 0.0), BUTTON_CORNER));
        container.setExclusionPaths(&NSArray::from_retained_slice(&[corner]));
    }

    pub(crate) fn text(&self) -> String {
        self.string().to_string()
    }

    fn panel(&self) -> Option<Retained<GitPanel>> {
        let delegate = self.delegate()?;
        let object: Retained<AnyObject> = delegate.into();
        object.downcast::<GitPanel>().ok()
    }
}
