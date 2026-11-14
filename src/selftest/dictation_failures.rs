//! What goes wrong in the `dictation` scenario and how the settings change it, after `dictation_fn`: fn switched off
//! and asked for again, a field that cannot be edited, a refused microphone and a note that leaves again, the
//! recording limit, a recording too short to transcribe, the key questions of the settings, the microphone question's
//! answer reaching the main thread, a field that gives up after the busy microphone's retries, a field that waits
//! while the microphone is still busy, text for a field that closed, and the settings' key buttons and fn switch.

use ide_core::LockIgnoringPoison;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lowspeak::audio::recorder::StartError;
use lowspeak::dictation::{Failure, Target};
use lowspeak::models::ApiProvider;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSApplication, NSModalResponse, NSPasteboard,
    NSPasteboardTypeString, NSRunningApplication, NSSecureTextField, NSView, NSWorkspace,
};
use objc2_foundation::{NSDictionary, NSString};
use splat_i18n::{t, tf};

use super::check;
use super::helpers::{after, alert_texts, answer_alert, in_any_mode_after, open_beside, tab, wait_until, window_of};
use crate::util::{constant, main_on_run_loop, ns};
use ide_ui_dictation::selftest::{self as dictation, Answer, Opening};
use ide_ui_dictation::{FnState, fn_state};

use super::dictation_fn::*;

/// Another app starting or quitting leaves fn as it was; switching fn off in the settings and asking for the permission
/// again show in the state the settings read.
pub(super) fn fn_settings(target: Target, then: fn()) {
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
    ide_ui_dictation::fn_setting_changed(false);
    let off = fn_state();
    ide_ui_dictation::fn_setting_changed(true);
    check(
        "switching fn off in the settings turns it off, and on again brings it back",
        off == FnState::Off && fn_state() == before,
        format!("{off:?}, {:?}", fn_state()),
    );
    ide_ui_dictation::ask_fn_permission();
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
            after(NOTE, move || {
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
            after(NO_TEXT, move || {
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
        saw.lock_ignoring_poison().push(said);
        answer_alert(response);
    });
}

fn seen_all(saw: &Arc<Mutex<Vec<bool>>>, count: usize) -> bool {
    let saw = saw.lock_ignoring_poison();
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
        ide_ui_dictation::take_over_key(groq);
        check(
            "instead of Lowspeak's key the settings take one the user enters",
            seen_all(&saw, 2) && ide_ui_dictation::has_key(groq),
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
        ide_ui_dictation::enter_key(openai);
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
        ide_ui_dictation::take_over_key(openai);
        dictation::lowspeak_key_readable(true);
        check(
            "when macOS does not hand Lowspeak's key out, the user is asked to enter one, saying why",
            seen_all(&saw, 2),
            format!("{saw:?}"),
        );
        let removed = ide_ui_dictation::remove_key(ApiProvider::Groq);
        check(
            "a key removed in the settings is gone",
            removed.is_ok() && !ide_ui_dictation::has_key(ApiProvider::Groq),
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
        heard.lock_ignoring_poison().push((granted, on_main));
    });
    after(ANSWER_ONCE, move || {
        let answers = answers.lock_ignoring_poison().clone();
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
            heard.lock_ignoring_poison().push(MainThreadMarker::new().is_some());
        });
        let arrived = Arc::clone(&answers);
        wait_until(
            move || !arrived.lock_ignoring_poison().is_empty(),
            move |ok| {
                let answers = answers.lock_ignoring_poison().clone();
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
    let name = "dictation-second";
    super::helpers::make_project(name, &[(SECOND, "Zweites Feld\n")]).unwrap_or_else(|_| super::helpers::root(name))
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
fn gives_up(target: Target, other: Target, next: impl FnOnce() + 'static) {
    dictation::open([Opening::After(Duration::from_secs(8))]);
    dictation::click(target);
    wait_until(
        move || dictation::seen(target).recording,
        move |_| {
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
        },
    );
}

/// The first field holds the microphone while it opens, the second waits and records once it is free.
fn busy_then_free(then: fn(), target: Target, other: Target, root: PathBuf) {
    dictation::open([Opening::After(Duration::from_millis(1500))]);
    dictation::click(target);
    after(CLICK_GAP, move || {
        dictation::click(target);
        after(CLICK_GAP, move || {
            dictation::click(other);
            let waits = dictation::seen(other);
            after(CLICK_GAP, move || {
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
    let root = super::helpers::root("dictation-project");
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
        let removed = super::settings_pages::control(objc2::sel!(ideDictationRemoveKey:), Some(0))
            .is_some_and(|b| super::settings_pages::act(&b));
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
    super::dictation_settings::run();
}
