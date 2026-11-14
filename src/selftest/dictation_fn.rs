//! How the `dictation` scenario starts and ends a dictation, after its first part: the menu item as the menu bar
//! chooses it, the microphone button's own click, fn held, double tapped, tapped once and locked with shift, and fn
//! while the app goes to the background. What goes wrong follows in `dictation_failures`.

use std::time::Duration;

use lowspeak::audio::recorder::StartError;
use lowspeak::dictation::{Failure, Target};
use lowspeak::models::{ApiProvider, OnlineModel};
use lowspeak::transcription::error::TranscriptionError;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSTextView};
use objc2_foundation::NSNotificationCenter;
use splat_i18n::t;

use super::check;
use super::helpers::{after, wait_until};
use crate::util::constant;
use ide_ui_dictation::selftest::{self as dictation, Answer, Pressed};

pub(super) const QUICK: Duration = Duration::from_millis(200);
/// How long a note stays under a field, with a margin.
pub(super) const NOTE: Duration = Duration::from_millis(9000);
/// How long fn stays down for a held dictation.
pub(super) const FN_HELD: Duration = Duration::from_millis(500);
/// How long a quick tap of fn or the gap between two taps lasts.
pub(super) const TAP: Duration = Duration::from_millis(80);
/// Longer than the window in which a second tap of fn would make a double tap.
pub(super) const PAST_DOUBLE_TAP: Duration = Duration::from_millis(900);
/// How long a hands-free recording must go on after fn came up.
pub(super) const STAYS_LOCKED: Duration = Duration::from_millis(600);
/// How long fn coming up must start nothing after a recording ended.
pub(super) const NOTHING_STARTS: Duration = Duration::from_millis(700);
/// How long a recording too short to transcribe must bring no text.
pub(super) const NO_TEXT: Duration = Duration::from_millis(1000);
/// How long a second answer to the microphone question would take to arrive.
pub(super) const ANSWER_ONCE: Duration = Duration::from_millis(600);
/// The gap between clicks that all fall into the microphone's 1.5 s opening.
pub(super) const CLICK_GAP: Duration = Duration::from_millis(200);
pub(super) const SECOND: &str = "zweite.md";

/// Runs these steps and those of `dictation_failures` for the editor field `target`, then `then`.
pub fn run(target: Target, then: fn()) {
    failure_texts();
    menu_bar(target, then);
}

pub(super) fn view(target: Target) -> Option<Retained<NSTextView>> {
    dictation::view_of(target)
}

pub(super) fn text(target: Target) -> String {
    view(target).map(|v| v.string().to_string()).unwrap_or_default()
}

pub(super) fn focus(target: Target) {
    if let Some(view) = view(target)
        && let Some(window) = view.window()
    {
        window.makeFirstResponder(Some(&view));
    }
}

pub(super) fn unfocus(target: Target) {
    if let Some(window) = view(target).and_then(|v| v.window()) {
        window.makeFirstResponder(None);
    }
}

pub(super) fn idle(target: Target) -> bool {
    let seen = dictation::seen(target);
    !seen.recording && !seen.transcribing
}

pub(super) fn press(key: Pressed) {
    dictation::fn_key(key);
}

pub(super) const FN_DOWN: Pressed = Pressed::FnDown { shift: false };

/// Where the app's texts stand in for lowspeak's, which name lowspeak's own menu and app.
fn failure_texts() {
    let language = objc2_foundation::NSBundle::mainBundle()
        .preferredLocalizations()
        .firstObject()
        .map_or_else(|| "en".to_string(), |l| l.to_string());
    let lowspeak = lowspeak::dictation::texts_for_locale(&language);
    let model = OnlineModel::GroqWhisperLargeV3;
    let cases = [
        (
            Failure::Microphone(StartError::PermissionDenied),
            ide_product::NAME.to_string(),
        ),
        (
            Failure::Transcription(TranscriptionError::NoApiKey(ApiProvider::Groq)),
            ApiProvider::Groq.display_name().to_string(),
        ),
        (
            Failure::Transcription(TranscriptionError::ModelNotAvailable),
            t("Diktat").to_string(),
        ),
        (
            Failure::Transcription(TranscriptionError::ModelRetired(model)),
            model.name().to_string(),
        ),
    ];
    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|(failure, named)| {
            let shown = dictation::failure_text(failure);
            (!shown.contains(named.as_str()) || shown == failure.describe(lowspeak)).then_some(shown)
        })
        .collect();
    check(
        "a refused microphone, a missing key, a missing model and a retired one read in the app's own words",
        wrong.is_empty(),
        wrong.join(" | "),
    );
}

/// The menu item as the menu bar chooses it: with the focus in a field it starts there, without one it ends the
/// recording that runs, and with neither it does not apply.
fn menu_bar(target: Target, then: fn()) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    focus(target);
    let enabled = dictation::choose_menu_item(mtm);
    wait_until(
        move || dictation::seen(target).listening,
        move |ok| {
            let Some(mtm) = MainThreadMarker::new() else { return };
            check(
                "the menu bar's item starts a dictation in the field with the focus",
                ok && enabled,
                "",
            );
            unfocus(target);
            let enabled = dictation::choose_menu_item(mtm);
            wait_until(
                move || idle(target) && !dictation::seen(target).line.is_empty(),
                move |ok| {
                    let Some(mtm) = MainThreadMarker::new() else { return };
                    let seen = dictation::seen(target);
                    check(
                        "without a field with the focus, the menu item ends the recording that runs",
                        enabled && ok,
                        format!("{seen:?}"),
                    );
                    check(
                        "a recording without speech says so under the field",
                        seen.line == t("Keine Sprache erkannt"),
                        format!("{seen:?}"),
                    );
                    let enabled = dictation::choose_menu_item(mtm);
                    check(
                        "with no field and no recording the menu item does not apply and starts nothing",
                        !enabled && idle(target),
                        format!("{:?}", dictation::seen(target)),
                    );
                    button(target, then);
                },
            );
        },
    );
}

/// The microphone button's own click starts a dictation and gives its field the focus.
fn button(target: Target, then: fn()) {
    dictation::click_button(target);
    wait_until(
        move || dictation::seen(target).listening,
        move |ok| {
            let focused = view(target).is_some_and(|v| {
                v.window()
                    .and_then(|w| w.firstResponder())
                    .is_some_and(|r| std::ptr::eq(Retained::as_ptr(&r).cast::<u8>(), Retained::as_ptr(&v).cast::<u8>()))
            });
            check(
                "a click on the microphone starts a dictation and puts the focus in its field",
                ok && focused,
                "",
            );
            dictation::click_button(target);
            wait_until(move || idle(target), move |_| fn_held(target, then));
        },
    );
}

/// fn held dictates until it comes up.
fn fn_held(target: Target, then: fn()) {
    focus(target);
    dictation::answer([Answer {
        after: QUICK,
        result: Ok("per fn".into()),
    }]);
    press(FN_DOWN);
    wait_until(
        move || dictation::seen(target).listening,
        move |listening| {
            after(FN_HELD, move || {
                let still = dictation::seen(target).listening;
                press(Pressed::FnUp);
                wait_until(
                    move || text(target).contains("per fn"),
                    move |ok| {
                        check(
                            "fn held dictates into the field with the focus until it comes up",
                            listening && still && ok,
                            text(target),
                        );
                        fn_double_tap(target, then);
                    },
                );
            });
        },
    );
}

/// Two quick taps of fn lock the recording until fn goes down again.
fn fn_double_tap(target: Target, then: fn()) {
    press(FN_DOWN);
    after(TAP, move || {
        press(Pressed::FnUp);
        after(TAP, move || {
            press(FN_DOWN);
            after(TAP, move || {
                press(Pressed::FnUp);
                after(PAST_DOUBLE_TAP, move || {
                    let locked = dictation::seen(target).recording;
                    press(FN_DOWN);
                    press(Pressed::FnUp);
                    wait_until(
                        move || idle(target),
                        move |stopped| {
                            check(
                                "a double tap of fn records hands-free until fn goes down again",
                                locked && stopped,
                                format!("{:?}", dictation::seen(target)),
                            );
                            fn_single_tap(target, then);
                        },
                    );
                });
            });
        });
    });
}

/// A single short tap of fn ends its recording once no second tap follows.
fn fn_single_tap(target: Target, then: fn()) {
    press(FN_DOWN);
    let started = dictation::seen(target).recording;
    after(TAP, move || {
        press(Pressed::FnUp);
        let waits = dictation::seen(target).recording;
        after(PAST_DOUBLE_TAP, move || {
            check(
                "a single tap of fn ends its recording when no second tap follows",
                started && waits && !dictation::seen(target).recording,
                format!("{:?}", dictation::seen(target)),
            );
            wait_until(move || idle(target), move |_| fn_shift(target, then));
        });
    });
}

/// fn with shift records hands-free, and shift alone ends it.
fn fn_shift(target: Target, then: fn()) {
    press(Pressed::FnDown { shift: true });
    after(TAP, move || {
        press(Pressed::FnUp);
        after(STAYS_LOCKED, move || {
            let locked = dictation::seen(target).recording;
            press(Pressed::ShiftDown);
            wait_until(
                move || idle(target),
                move |stopped| {
                    check(
                        "fn with shift records hands-free after fn comes up, and shift ends it",
                        locked && stopped,
                        format!("{:?}", dictation::seen(target)),
                    );
                    fn_then_button(target, then);
                },
            );
        });
    });
}

/// A recording fn started and the button ended is over for fn too.
fn fn_then_button(target: Target, then: fn()) {
    press(FN_DOWN);
    let started = dictation::seen(target).recording;
    dictation::click(target);
    press(Pressed::FnUp);
    after(NOTHING_STARTS, move || {
        check(
            "a recording fn started ends with a click, and fn coming up then starts nothing",
            started && !dictation::seen(target).recording,
            format!("{:?}", dictation::seen(target)),
        );
        wait_until(move || idle(target), move |_| fn_elsewhere(target, then));
    });
}

/// fn outside a field that takes dictation does nothing.
fn fn_elsewhere(target: Target, then: fn()) {
    unfocus(target);
    press(FN_DOWN);
    let recording = dictation::seen(target).recording;
    press(Pressed::FnUp);
    check("fn outside a field with a microphone starts nothing", !recording, "");
    focus(target);
    press(FN_DOWN);
    let started = dictation::seen(target).recording;
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: AppKit's own notification names, posted with the application as their object as AppKit does.
    unsafe {
        center.postNotificationName_object(
            constant!(objc2_app_kit::NSApplicationDidResignActiveNotification),
            Some(&app),
        );
    }
    // SAFETY: As above.
    unsafe {
        center.postNotificationName_object(
            constant!(objc2_app_kit::NSApplicationDidBecomeActiveNotification),
            Some(&app),
        );
    }
    wait_until(
        move || idle(target),
        move |ended| {
            press(Pressed::FnUp);
            check(
                "a recording fn holds ends when the app goes to the background",
                started && ended,
                format!("{:?}", dictation::seen(target)),
            );
            after(NOTHING_STARTS, move || {
                super::dictation_failures::fn_settings(target, then)
            });
        },
    );
}
