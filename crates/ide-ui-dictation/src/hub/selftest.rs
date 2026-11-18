//! What the self-test does with dictation and reads back from it.

use std::time::Duration;

use lowspeak::dictation::Target;
pub use lowspeak::hotkey::Key as Pressed;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{NSAccessibility, NSApplication, NSTextView};

use super::desk::Phase;
pub use super::engine::fake::{Answer, Opening, answer, lowspeak_key_readable, open, too_short};
use super::{Via, with_hub};

/// What the controls of a field show.
#[derive(Debug, Clone, Default)]
pub struct Seen {
    pub recording: bool,
    pub listening: bool,
    pub transcribing: bool,
    pub spinning: bool,
    /// The line beside the controls.
    pub line: String,
    /// The button's label for VoiceOver.
    pub label: String,
    pub value: String,
}

/// The field number of `view`.
pub fn target_of(view: &NSTextView) -> Option<Target> {
    with_hub(|hub| hub.target_of(view)).flatten()
}

/// A click on the microphone of `target`.
pub fn click(target: Target) {
    super::press(target, Via::Button);
}

/// The menu item, or its shortcut.
pub fn menu(mtm: MainThreadMarker) {
    super::toggle_focused(mtm, Via::Menu);
}

pub fn seen(target: Target) -> Seen {
    with_hub(|hub| {
        let phase = hub.desk.phase(target);
        let place = hub.places.get(&target)?;
        Some(Seen {
            recording: phase != Phase::Idle || hub.desk.is_waiting(target),
            listening: phase == Phase::Listening,
            transcribing: hub.desk.transcribing(target) > 0,
            spinning: place.mic.spinning(),
            line: place
                .line
                .load()
                .filter(|line| !line.isHidden())
                .map(|line| line.stringValue().to_string())
                .unwrap_or_default(),
            label: place
                .mic
                .button
                .accessibilityLabel()
                .map(|l| l.to_string())
                .unwrap_or_default(),
            value: place
                .mic
                .button
                .accessibilityValue()
                .and_then(|v| v.downcast::<objc2_foundation::NSString>().ok())
                .map(|v| v.to_string())
                .unwrap_or_default(),
        })
    })
    .flatten()
    .unwrap_or_default()
}

/// The title the menu item shows now, and whether it can be chosen.
pub fn menu_item(mtm: MainThreadMarker) -> (String, bool) {
    let item = super::menu_item(mtm, "");
    let enabled = super::validate(mtm, &item);
    (item.title().to_string(), enabled)
}

/// How a failure reads in the interface language, the way a field shows it.
pub fn failure_text(failure: &lowspeak::dictation::Failure) -> String {
    super::describe(failure)
}

/// A click on the microphone button of `target` itself, which sends its action as AppKit does.
pub fn click_button(target: Target) {
    let button = with_hub(|hub| hub.places.get(&target).map(|place| place.mic.button.clone())).flatten();
    if let Some(button) = button {
        // SAFETY: The button is in a window of this app, on the main thread, and its target outlives it.
        unsafe { button.performClick(None) };
    }
}

/// The menu item chosen as the menu bar does: AppKit asks its target whether it applies, then sends its action;
/// whether it applied.
pub fn choose_menu_item(mtm: MainThreadMarker) -> bool {
    let item = super::menu_item(mtm, "");
    let target = super::target(mtm);
    // SAFETY: `validateMenuItem:` takes a menu item and answers a boolean, as the target defines it.
    let enabled: bool = unsafe { msg_send![&*target, validateMenuItem: &*item] };
    let sender: &AnyObject = &item;
    if let Some(action) = item.action() {
        let to: &AnyObject = &target;
        // SAFETY: The action is the item's own, which the target implements with an optional sender.
        unsafe { NSApplication::sharedApplication(mtm).sendAction_to_from(action, Some(to), Some(sender)) };
    }
    enabled
}

/// fn, or shift with it, as the event tap reports it.
pub fn fn_key(key: Pressed) {
    super::fn_key::pressed(key);
}

/// Whether the app asked macOS for the Accessibility permission that fn needs.
pub fn fn_asked() -> bool {
    super::fn_key::asked()
}

/// Dictations from now on stop recording after `limit`.
pub fn limit(limit: Duration) {
    super::engine::fake::limit(limit);
    with_hub(|hub| hub.dictation = None);
}

/// The text view of `target`.
pub fn view_of(target: Target) -> Option<Retained<NSTextView>> {
    with_hub(|hub| hub.text_of(target)).flatten()
}

/// Whether macOS has an answer about the microphone for this app, whatever the self-test plays.
pub fn microphone_decided() -> bool {
    !super::microphone::macos_undecided()
}

/// macOS's microphone question, for a scenario that made sure macOS has its answer and shows nothing.
pub fn ask_microphone(answered: impl FnOnce(bool) + Send + 'static) {
    super::microphone::ask(answered);
}

/// The handler AVFoundation gets for its answer, called twice from a thread of its own, the first time with `granted`.
pub fn answer_microphone_twice(granted: bool, answered: impl FnOnce(bool) + Send + 'static) {
    let handler = super::microphone::once_on_main(answered);
    std::thread::spawn(move || {
        handler(objc2::runtime::Bool::new(granted));
        handler(objc2::runtime::Bool::new(!granted));
    });
}

/// The choices the engine starts its next dictation with.
pub fn engine_choices() -> lowspeak::dictation::Choices {
    with_hub(|hub| hub.engine().choices()).unwrap_or_else(super::choices::defaults)
}
