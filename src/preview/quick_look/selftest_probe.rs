//! The self-test's hooks of `quick_look.rs`.

use super::*;

/// Whether `view` is a Quick Look view, for the self-test.
pub fn shows(view: &NSView) -> bool {
    use objc2_foundation::NSObjectProtocol;
    AnyClass::get(c"QLPreviewView").is_some_and(|class| view.isKindOfClass(class))
}
