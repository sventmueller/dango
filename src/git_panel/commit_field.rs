//! The commit message: plain text over several lines, where ⌘↩ commits.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSTextView};
use objc2_foundation::{NSObjectProtocol, NSRect, NSSize};

use super::panel::GitPanel;
use crate::util::guarded;

/// Return and the Return of the number block.
const RETURN_KEYS: [u16; 2] = [36, 76];

define_class!(
    // SAFETY: NSTextView has no subclassing requirements and `CommitTextView` does not implement Drop.
    #[unsafe(super(NSTextView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitCommitTextView"]
    pub struct CommitTextView;

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
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<CommitTextView> {
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

    pub fn text(&self) -> String {
        self.string().to_string()
    }

    fn panel(&self) -> Option<Retained<GitPanel>> {
        let delegate = self.delegate()?;
        let object: Retained<AnyObject> = delegate.into();
        object.downcast::<GitPanel>().ok()
    }
}
