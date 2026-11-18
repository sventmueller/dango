//! The size of the AI tab's text, which the user chooses in the settings; the timeline, the composer and the status
//! line take every size of their text from it.

use std::cell::Cell;
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use ide_core::settings::{Key, Settings, Sharing};
use ide_ui_common::util::{guarded, ns};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol};

use crate::AgentTab;
use crate::style::BODY;

/// The size of running text in the AI tabs, in points; the other sizes of their text follow it.
pub const CHAT_FONT_SIZE: Key<f64> = Key::user("chatFontSize", Sharing::Shared, || BODY);

/// The smallest size the settings offer.
pub const CHAT_FONT_MIN: f64 = 10.0;
/// The largest size the settings offer.
pub const CHAT_FONT_MAX: f64 = 24.0;

/// The notification that tells the open AI tabs that the text size changed.
const CHANGED: &str = "IDEAgentTextSizeChanged";

thread_local! {
    static SIZE: Cell<f64> = const { Cell::new(BODY) };
}

/// The text size of the AI tabs; the default until the app sets the user's, as in unit tests.
pub(crate) fn current() -> f64 {
    SIZE.with(Cell::get)
}

/// Shows the AI tabs' text at `size` points, as the settings have it; the app calls it as it starts.
pub fn set_text_size(size: f64) {
    let size = valid(size);
    if SIZE.with(|shown| shown.replace(size)) != size {
        // SAFETY: The app's own notification, posted without an object.
        unsafe { NSNotificationCenter::defaultCenter().postNotificationName_object(&ns(CHANGED), None) };
    }
}

/// Has `tab` take every change of the text size while it lives; the tab removes the observer as it goes.
pub(crate) fn follow(tab: &Rc<AgentTab>) -> Retained<ProtocolObject<dyn NSObjectProtocol>> {
    let tab = Rc::downgrade(tab);
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        guarded((), || {
            if let Some(tab) = tab.upgrade() {
                tab.text_size_changed();
            }
        });
    });
    // SAFETY: The block holds the tab weakly and does nothing once it is gone; it runs on the main thread, where the
    // app posts the notification.
    unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(&ns(CHANGED)),
            None,
            None,
            &block,
        )
    }
}

/// Takes the text size from the settings when the settings `names` that changed include it.
pub fn settings_changed(names: &[String]) {
    if names.iter().any(|name| name == CHAT_FONT_SIZE.declared().name) {
        set_text_size(Settings::load_user().get(&CHAT_FONT_SIZE));
    }
}

/// `size`, or the default where a hand-edited settings file holds something the AI tab cannot show.
fn valid(size: f64) -> f64 {
    if size.is_finite() {
        size.clamp(CHAT_FONT_MIN, CHAT_FONT_MAX)
    } else {
        BODY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_outside_the_settings_range_is_brought_into_it() {
        assert_eq!(valid(14.0), 14.0);
        assert_eq!(valid(2.0), CHAT_FONT_MIN);
        assert_eq!(valid(400.0), CHAT_FONT_MAX);
        assert_eq!(valid(f64::NAN), BODY);
    }
}
