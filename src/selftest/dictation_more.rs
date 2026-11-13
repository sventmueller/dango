//! The second half of the `dictation` scenario: the menu item as the menu bar chooses it, the microphone button's own
//! click, fn held, double tapped, tapped once and locked with shift, fn while the app goes to the background, the key
//! questions of the settings, a refused microphone and a note that leaves again, the recording limit, a recording too
//! short to transcribe, a field that gives up after the busy microphone's retries, a field that waits while the
//! microphone is still busy, text for a field that closed, the microphone question's answer reaching the main thread,
//! and the dictation settings' key buttons and fn switch.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use lowspeak::audio::recorder::StartError;
use lowspeak::dictation::{Failure, Target};
use lowspeak::models::{ApiProvider, OnlineModel};
use lowspeak::transcription::error::TranscriptionError;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSApplication, NSModalResponse, NSPasteboard,
    NSPasteboardTypeString, NSRunningApplication, NSSecureTextField, NSTextView, NSView, NSWorkspace,
};
use objc2_foundation::{NSDictionary, NSNotificationCenter, NSString};
use splat_i18n::{t, tf};

use super::check;
use super::helpers::{alert_texts, answer_alert, in_any_mode_after, open_beside, tab, wait_until, window_of};
use crate::dictation::selftest::{self as dictation, Answer, Opening, Pressed};
use crate::dictation::{FnState, fn_state};
use crate::util::{constant, main_after, main_on_run_loop, ns};

const QUICK: Duration = Duration::from_millis(200);
/// How long a note stays under a field, with a margin.
const NOTE: Duration = Duration::from_millis(9000);
const SECOND: &str = "zweite.md";

/// Runs the second half for the editor field `target`, then `then`.
pub fn run(target: Target, then: fn()) {
    failure_texts();
    menu_bar(target, then);
}

fn after(millis: u64, f: impl FnOnce() + Send + 'static) {
    main_after(Duration::from_millis(millis), f);
}

fn view(target: Target) -> Option<Retained<NSTextView>> {
    dictation::view_of(target)
}

fn text(target: Target) -> String {
    view(target).map(|v| v.string().to_string()).unwrap_or_default()
}

fn focus(target: Target) {
    if let Some(view) = view(target)
        && let Some(window) = view.window()
    {
        window.makeFirstResponder(Some(&view));
    }
}

fn unfocus(target: Target) {
    if let Some(window) = view(target).and_then(|v| v.window()) {
        window.makeFirstResponder(None);
    }
}

fn idle(target: Target) -> bool {
    let seen = dictation::seen(target);
    !seen.recording && !seen.transcribing
}

fn press(key: Pressed) {
    dictation::fn_key(key);
}

const FN_DOWN: Pressed = Pressed::FnDown { shift: false };

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
            "Lowspeak".to_string(),
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
            after(500, move || {
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
    after(80, move || {
        press(Pressed::FnUp);
        after(80, move || {
            press(FN_DOWN);
            after(80, move || {
                press(Pressed::FnUp);
                after(900, move || {
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
    after(80, move || {
        press(Pressed::FnUp);
        let waits = dictation::seen(target).recording;
        after(900, move || {
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
    after(80, move || {
        press(Pressed::FnUp);
        after(600, move || {
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
    after(700, move || {
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
            after(600, move || fn_settings(target, then));
        },
    );
}

/// Another app starting or quitting leaves fn as it was; switching fn off in the settings and asking for the permission
/// again show in the state the settings read.
fn fn_settings(target: Target, then: fn()) {
    let before = fn_state();
    let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
    let app = NSRunningApplication::currentApplication();
    let value: &AnyObject = &app;
    let key: &NSString = constant!(objc2_app_kit::NSWorkspaceApplicationKey);
    let info = NSDictionary::from_slices(&[key], &[value]);
    // SAFETY: A dictionary's key and value types exist only for the compiler; userInfo takes any dictionary.
    let info: &NSDictionary = unsafe { &*Retained::as_ptr(&info).cast::<NSDictionary>() };
    for name in [
        constant!(objc2_app_kit::NSWorkspaceDidLaunchApplicationNotification),
        constant!(objc2_app_kit::NSWorkspaceDidTerminateApplicationNotification),
    ] {
        // SAFETY: NSWorkspace's notification names with the running application under its documented key.
        unsafe { workspace.postNotificationName_object_userInfo(name, None, Some(info)) };
    }
    check(
        "another app starting and quitting leaves fn as it was",
        fn_state() == before,
        format!("{before:?} → {:?}", fn_state()),
    );
    crate::dictation::fn_setting_changed(false);
    let off = fn_state();
    crate::dictation::fn_setting_changed(true);
    check(
        "switching fn off in the settings turns it off, and on again brings it back",
        off == FnState::Off && fn_state() == before,
        format!("{off:?}, {:?}", fn_state()),
    );
    crate::dictation::ask_fn_permission();
    check(
        "asking for fn's permission again is remembered",
        dictation::fn_asked(),
        "",
    );
    not_editable(target, then);
}

/// A field that cannot be edited takes no dictation.
fn not_editable(target: Target, then: fn()) {
    if let Some(view) = view(target) {
        view.setEditable(false);
    }
    dictation::click(target);
    let recording = dictation::seen(target).recording;
    if let Some(view) = view(target) {
        view.setEditable(true);
    }
    check("a field that cannot be edited starts no dictation", !recording, "");
    refused(target, then);
}

/// A microphone that refuses says why in the app's words, and the note leaves after a while.
fn refused(target: Target, then: fn()) {
    dictation::open([Opening::Refused(StartError::PermissionDenied)]);
    dictation::click(target);
    let expected = dictation::failure_text(&Failure::Microphone(StartError::PermissionDenied));
    let wanted = expected.clone();
    wait_until(
        move || dictation::seen(target).line == wanted,
        move |ok| {
            check(
                "a microphone without permission says so under the field and ends the recording",
                ok && !dictation::seen(target).recording,
                format!("{:?}", dictation::seen(target)),
            );
            main_after(NOTE, move || {
                let seen = dictation::seen(target);
                check(
                    "a note under the field leaves after a while",
                    seen.line.is_empty(),
                    format!("{seen:?}"),
                );
                limit(target, then);
            });
        },
    );
}

/// A recording that reaches its limit is transcribed and says so.
fn limit(target: Target, then: fn()) {
    dictation::limit(Duration::from_secs(1));
    dictation::answer([Answer {
        after: QUICK,
        result: Ok(" bis zur Grenze".into()),
    }]);
    dictation::click(target);
    wait_until(
        move || text(target).contains("bis zur Grenze"),
        move |ok| {
            let seen = dictation::seen(target);
            check(
                "a recording that reaches its limit ends by itself, goes in and says so",
                ok && !seen.recording && seen.line == t("Die Aufnahme hat ihre Höchstdauer erreicht."),
                format!("{seen:?}"),
            );
            dictation::limit(Duration::from_secs(300));
            too_short(target, then);
        },
    );
}

/// A recording too short to transcribe ends without text or note.
fn too_short(target: Target, then: fn()) {
    let before = text(target);
    dictation::too_short();
    dictation::click(target);
    wait_until(
        move || dictation::seen(target).listening,
        move |_| {
            dictation::click(target);
            after(1000, move || {
                let seen = dictation::seen(target);
                check(
                    "a recording too short to transcribe ends without text",
                    idle(target) && text(target) == before,
                    format!("{seen:?}"),
                );
                keys(then, target);
            });
        },
    );
}

/// The secure field of the alert in front.
fn secure_field(view: &NSView) -> Option<Retained<NSSecureTextField>> {
    if let Some(field) = view.downcast_ref::<NSSecureTextField>() {
        return Some(field.retain());
    }
    view.subviews().iter().find_map(|child| secure_field(&child))
}

/// Answers the key alert in front `millis` from now with `response`, after typing `typed` into its field and noting
/// whether it says `says`.
fn key_alert(millis: u64, typed: &'static str, says: String, response: NSModalResponse, saw: Arc<Mutex<Vec<bool>>>) {
    in_any_mode_after(Duration::from_millis(millis), move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let alert = NSApplication::sharedApplication(mtm).modalWindow();
        let said = alert_texts().iter().any(|text| text.contains(&says));
        if let Some(field) = alert.and_then(|a| a.contentView()).as_deref().and_then(secure_field) {
            field.setStringValue(&ns(typed));
        }
        saw.lock().unwrap_or_else(PoisonError::into_inner).push(said);
        answer_alert(response);
    });
}

fn seen_all(saw: &Arc<Mutex<Vec<bool>>>, count: usize) -> bool {
    let saw = saw.lock().unwrap_or_else(PoisonError::into_inner);
    saw.len() == count && saw.iter().all(|said| *said)
}

/// The settings' key questions: entering one instead of taking Lowspeak's over, a wrong one asked again, Lowspeak's
/// key macOS did not hand out, and removing a key.
fn keys(then: fn(), target: Target) {
    let saw = Arc::new(Mutex::new(Vec::new()));
    let groq = ApiProvider::Groq;
    let question = tf(
        "Lowspeak hat einen Schlüssel für {}. Für {} übernehmen?",
        &[groq.display_name(), ide_product::NAME],
    );
    key_alert(800, "", question, NSAlertSecondButtonReturn, Arc::clone(&saw));
    key_alert(
        1800,
        "gsk_selbst_eingegeben",
        tf("API-Schlüssel für {}", &[groq.display_name()]),
        NSAlertFirstButtonReturn,
        Arc::clone(&saw),
    );
    main_on_run_loop(move || {
        crate::dictation::take_over_key(groq);
        check(
            "instead of Lowspeak's key the settings take one the user enters",
            seen_all(&saw, 2) && crate::dictation::has_key(groq),
            format!("{saw:?}"),
        );
        wrong_key(then, target);
    });
}

fn wrong_key(then: fn(), target: Target) {
    let saw = Arc::new(Mutex::new(Vec::new()));
    let openai = ApiProvider::OpenAi;
    key_alert(
        800,
        "kein Schlüssel",
        tf("API-Schlüssel für {}", &[openai.display_name()]),
        NSAlertFirstButtonReturn,
        Arc::clone(&saw),
    );
    let problem = t("Das ist kein Schlüssel von OpenAI (sk-…) oder Groq (gsk_…).").to_string();
    key_alert(1800, "", problem, NSAlertSecondButtonReturn, Arc::clone(&saw));
    main_on_run_loop(move || {
        crate::dictation::enter_key(openai);
        check(
            "a key of no known provider is asked for again, saying why",
            seen_all(&saw, 2),
            format!("{saw:?}"),
        );
        unreadable_key(then, target);
    });
}

fn unreadable_key(then: fn(), target: Target) {
    let saw = Arc::new(Mutex::new(Vec::new()));
    let openai = ApiProvider::OpenAi;
    dictation::lowspeak_key_readable(false);
    let question = tf(
        "Lowspeak hat einen Schlüssel für {}. Für {} übernehmen?",
        &[openai.display_name(), ide_product::NAME],
    );
    key_alert(800, "", question, NSAlertFirstButtonReturn, Arc::clone(&saw));
    let problem = t("Lowspeaks Schlüssel ließ sich nicht lesen. Gib den Schlüssel selbst ein.").to_string();
    key_alert(1800, "", problem, NSAlertSecondButtonReturn, Arc::clone(&saw));
    main_on_run_loop(move || {
        crate::dictation::take_over_key(openai);
        dictation::lowspeak_key_readable(true);
        check(
            "when macOS does not hand Lowspeak's key out, the user is asked to enter one, saying why",
            seen_all(&saw, 2),
            format!("{saw:?}"),
        );
        let removed = crate::dictation::remove_key(ApiProvider::Groq);
        check(
            "a key removed in the settings is gone",
            removed.is_ok() && !crate::dictation::has_key(ApiProvider::Groq),
            format!("{removed:?}"),
        );
        microphone(then, target);
    });
}

/// AVFoundation's answer to the microphone question reaches the main thread once, from whatever thread it comes.
fn microphone(then: fn(), target: Target) {
    let answers = Arc::new(Mutex::new(Vec::new()));
    let heard = Arc::clone(&answers);
    dictation::answer_microphone_twice(true, move |granted| {
        let on_main = MainThreadMarker::new().is_some();
        heard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((granted, on_main));
    });
    after(600, move || {
        let answers = answers.lock().unwrap_or_else(PoisonError::into_inner).clone();
        check(
            "the microphone question's answer reaches the main thread once",
            answers == [(true, true)],
            format!("{answers:?}"),
        );
        if !dictation::microphone_decided() {
            return busy(then, target);
        }
        let answers = Arc::new(Mutex::new(Vec::new()));
        let heard = Arc::clone(&answers);
        dictation::ask_microphone(move |_| {
            heard
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(MainThreadMarker::new().is_some());
        });
        let arrived = Arc::clone(&answers);
        wait_until(
            move || !arrived.lock().unwrap_or_else(PoisonError::into_inner).is_empty(),
            move |ok| {
                let answers = answers.lock().unwrap_or_else(PoisonError::into_inner).clone();
                check(
                    "once macOS has its answer, asking again hears it on the main thread without a question",
                    ok && answers == [true],
                    format!("{answers:?}"),
                );
                busy(then, target);
            },
        );
    });
}

fn second_root() -> PathBuf {
    let folder = ide_core::paths::home().join("dictation-second");
    let _ = std::fs::create_dir_all(&folder);
    let _ = std::fs::write(folder.join(SECOND), "Zweites Feld\n");
    folder.canonicalize().unwrap_or(folder)
}

fn second_target(root: &Path) -> Option<Target> {
    let controller = window_of(root)?;
    let view = crate::editor::editor_for(&controller, &root.join(SECOND))?;
    dictation::target_of(&view)
}

/// A field whose start the busy microphone refuses waits, a click ends the wait, and once the microphone is free the
/// waiting field records.
fn busy(then: fn(), target: Target) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = second_root();
    crate::project::trust(&root);
    open_beside(mtm, &root);
    if let Some(controller) = window_of(&root) {
        controller.open_file(&root.join(SECOND));
    }
    let found = root.clone();
    wait_until(
        move || second_target(&found).is_some(),
        move |ok| {
            let Some(other) = second_target(&root).filter(|_| ok) else {
                check("a second window has a field with a microphone", false, "");
                return then();
            };
            gives_up(target, other, move || busy_then_free(then, target, other, root));
        },
    );
}

/// A field that waits longer than the busy microphone's retries allow gives up, while the field that holds the
/// microphone goes on.
fn gives_up(target: Target, other: Target, next: impl FnOnce() + Send + 'static) {
    dictation::open([Opening::After(Duration::from_secs(8))]);
    dictation::click(target);
    after(200, move || {
        dictation::click(other);
        let waits = dictation::seen(other);
        wait_until(
            move || !dictation::seen(other).recording,
            move |gave_up| {
                let holder = dictation::seen(target);
                check(
                    "a field that waits longer than the busy microphone's retries gives up",
                    waits.recording && !waits.listening && gave_up && holder.recording,
                    format!("{waits:?}, {holder:?}"),
                );
                wait_until(
                    move || dictation::seen(target).listening,
                    move |_| {
                        dictation::click(target);
                        wait_until(move || idle(target), move |_| next());
                    },
                );
            },
        );
    });
}

/// The first field holds the microphone while it opens, the second waits and records once it is free.
fn busy_then_free(then: fn(), target: Target, other: Target, root: PathBuf) {
    dictation::open([Opening::After(Duration::from_millis(1500))]);
    dictation::click(target);
    after(200, move || {
        dictation::click(target);
        after(200, move || {
            dictation::click(other);
            let waits = dictation::seen(other);
            after(200, move || {
                dictation::click(other);
                let no_longer = dictation::seen(other);
                dictation::click(other);
                check(
                    "a field waits while the microphone is still busy, and a click ends the wait",
                    waits.recording && !waits.listening && !no_longer.recording,
                    format!("{waits:?}, {no_longer:?}"),
                );
                wait_until(
                    move || dictation::seen(other).listening,
                    move |ok| {
                        check("the waiting field records once the microphone is free", ok, "");
                        closed_field(then, other, root);
                    },
                );
            });
        });
    });
}

/// Text for a field that closed while it was transcribed goes to the clipboard.
fn closed_field(then: fn(), other: Target, root: PathBuf) {
    const LATE: &str = "für ein geschlossenes Feld";
    dictation::answer([Answer {
        after: Duration::from_millis(1200),
        result: Ok(LATE.into()),
    }]);
    dictation::click(other);
    if let Some(controller) = window_of(&root)
        && let Some(id) = tab(&controller, SECOND)
    {
        controller.close_tab(id);
    }
    wait_until(
        || {
            NSPasteboard::generalPasteboard()
                .stringForType(constant!(NSPasteboardTypeString))
                .is_some_and(|s| s.to_string() == LATE)
        },
        move |ok| {
            check(
                "the text of a field that closed meanwhile goes to the clipboard",
                ok,
                "",
            );
            if let Some(window) = window_of(&root).and_then(|c| c.window()) {
                window.close();
            }
            save();
            then();
        },
    );
}

/// The notes as the second half left them, saved, so quitting asks nothing.
fn save() {
    let root = ide_core::paths::home().join("dictation-project");
    let root = root.canonicalize().unwrap_or(root);
    let editor = crate::window::for_project(&root)
        .and_then(|controller| crate::editor::editor_for(&controller, &root.join("notes.md")));
    if let Some(editor) = editor {
        crate::editor::save_view(&editor);
    }
}

/// The buttons of the dictation settings: a key only Lowspeak keeps is taken over, the app's own is removed, which
/// leaves Lowspeak's, and the fn switch turns fn off and on; then the scenario ends.
pub fn settings_buttons() {
    let saw = Arc::new(Mutex::new(Vec::new()));
    let groq = ApiProvider::Groq;
    let question = tf(
        "Lowspeak hat einen Schlüssel für {}. Für {} übernehmen?",
        &[groq.display_name(), ide_product::NAME],
    );
    key_alert(800, "", question, NSAlertFirstButtonReturn, Arc::clone(&saw));
    main_on_run_loop(move || {
        let key_button = |tag| super::settings_pages::control(objc2::sel!(ideDictationKey:), Some(tag));
        let took = key_button(1).is_some_and(|b| super::settings_pages::act(&b));
        let taken = crate::settings::dictation_keys();
        let removed = key_button(0).is_some_and(|b| super::settings_pages::act(&b));
        let lines = crate::settings::dictation_keys();
        check(
            "the settings' buttons take Lowspeak's key over and remove the app's own, which leaves Lowspeak's",
            took && removed
                && seen_all(&saw, 1)
                && taken == ["OpenAI: im Schlüsselbund", "Groq: im Schlüsselbund"]
                && lines == ["OpenAI: in Lowspeak", "Groq: im Schlüsselbund"],
            format!("{} → {}", taken.join(" | "), lines.join(" | ")),
        );
        fn_switch();
    });
}

fn fn_switch() {
    let switch = super::settings_pages::control(objc2::sel!(ideDictationWithFn:), None)
        .and_then(|c| c.downcast::<objc2_app_kit::NSButton>().ok());
    let before = fn_state();
    let flip = |on: bool| {
        switch.as_ref().is_some_and(|b| {
            b.setState(if on {
                objc2_app_kit::NSControlStateValueOn
            } else {
                objc2_app_kit::NSControlStateValueOff
            });
            super::settings_pages::act(b)
        })
    };
    let off = flip(false) && fn_state() == FnState::Off;
    let on = flip(true) && fn_state() == before;
    check(
        "the fn switch turns fn off and on again",
        off && on,
        format!("{before:?} → {:?}", fn_state()),
    );
    super::finish("dictation");
}
