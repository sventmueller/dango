//! What the self-test does with dictation and reads back from it.

use lowspeak::dictation::Target;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAccessibility, NSTextView};

use super::desk::Phase;
pub use super::engine::fake::{Answer, answer};
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
