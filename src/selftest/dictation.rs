//! The `dictation` scenario, built with dictation and with a microphone and an engine of the self-test behind
//! lowspeak's seam: the key question and taking Lowspeak's key over, text at the cursor of the editor and of the
//! composer as one undo step, a dictation that keeps its field while the focus moves, the menu item, Esc while
//! recording and while transcribing, a failure in the interface language, the states of the controls and the settings
//! tab, in light and dark.

use std::cell::Cell;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::time::Duration;

use block2::RcBlock;
use lowspeak::dictation::{Failure, Target};
use lowspeak::models::ApiProvider;
use lowspeak::transcription::error::TranscriptionError;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
    NSModalResponse, NSStandardKeyBindingResponding, NSTextView,
};
use objc2_foundation::{NSRange, NSRunLoop, NSRunLoopCommonModes, NSTimer};
use splat_i18n::t;

use super::helpers::wait_until;
use super::{check, finish, helpers, snapshot};
use crate::util::{constant, main_on_run_loop};
use crate::window::{self, WindowController};
use ide_ui_agent::AgentKey;
use ide_ui_dictation::selftest::{self as dictation, Answer};

const SCENARIO: &str = "dictation";
const NOTES: &str = "notes.md";
const TEXT: &str = "Notiz: \n";
/// Long enough for a picture of the spinner.
const SLOW: Duration = Duration::from_millis(1500);
const QUICK: Duration = Duration::from_millis(200);

const PROJECT: &str = "dictation-project";
/// How long a changed field or window takes to draw before its picture, which it does without a state to wait for.
const DRAW: Duration = Duration::from_millis(300);
/// How long the recording Esc ended takes to wind down, which the field no longer shows.
const WIND_DOWN: Duration = Duration::from_millis(600);
/// How long the dropped transcription would take to arrive, with a margin.
const DROPPED_TEXT: Duration = SLOW.saturating_add(Duration::from_millis(800));

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn controller() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn editor() -> Option<Retained<crate::editor::EditorTextView>> {
    crate::editor::editor_for(&*controller()?, &root().join(NOTES))
}

fn editor_target() -> Option<Target> {
    let view = editor()?;
    dictation::target_of(&view)
}

fn picture(name: &str) {
    if let Some(window) = controller().and_then(|c| c.window()) {
        window.displayIfNeeded();
        snapshot(&window, name);
    }
}

/// The text view that has the keyboard focus in the project's window.
fn focused_text() -> Option<Retained<NSTextView>> {
    let responder = controller()?.window()?.firstResponder()?;
    responder.downcast::<NSTextView>().ok()
}

/// Esc in the field with the focus, as the key's binding sends it.
fn escape() {
    if let Some(text) = focused_text() {
        // SAFETY: `cancelOperation:` is a command of the standard key bindings, which take the sender only.
        unsafe { text.doCommandBySelector(objc2::sel!(cancelOperation:)) };
    }
}

/// Answers the alert in front with `response` after `delay`, from a timer of the run loop, which runs while the alert
/// does, and takes a picture of it first.
fn answer_alert(delay: Duration, picture: &'static str, response: NSModalResponse) {
    let once = Cell::new(true);
    let block = RcBlock::new(move |_timer: NonNull<NSTimer>| {
        let Some(mtm) = MainThreadMarker::new().filter(|_| once.replace(false)) else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        let Some(alert) = app.modalWindow() else { return };
        let texts = super::helpers::alert_texts();
        let question = splat_i18n::tf(
            "Lowspeak hat einen Schlüssel für {}. Für {} übernehmen?",
            &[ApiProvider::OpenAi.display_name(), ide_product::NAME],
        );
        check(
            "a dictation without the app's key offers to take Lowspeak's over",
            texts.contains(&question),
            texts.join(" | "),
        );
        alert.display();
        snapshot(&alert, picture);
        app.stopModalWithCode(response);
    });
    // SAFETY: The block holds only a flag and a number, which may go to any thread; the main run loop calls it.
    let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(delay.as_secs_f64(), false, &block) };
    // SAFETY: The main run loop takes the timer in its common modes, which include the one modal alerts run in.
    unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
}

pub fn run() {
    helpers::open_project(SCENARIO, PROJECT, &[(NOTES, TEXT)], |controller| {
        controller.open_file(&root().join(NOTES));
        wait_for_editor_target();
    });
}

fn wait_for_editor_target() {
    wait_until(
        || editor_target().is_some(),
        |ok| {
            check("an editor has a microphone in its status bar", ok, "");
            if !ok {
                return finish(SCENARIO);
            }
            first_dictation();
        },
    );
}

/// The first dictation asks for the missing key: Lowspeak has one, and taking it over keeps a copy of the app's own.
fn first_dictation() {
    let (Some(view), Some(target)) = (editor(), editor_target()) else {
        return finish(SCENARIO);
    };
    let seen = dictation::seen(target);
    check(
        "the microphone starts a dictation",
        !seen.recording && seen.label == t("Diktat starten"),
        format!("{seen:?}"),
    );
    view.setSelectedRange(NSRange::new(TEXT.encode_utf16().count() - 1, 0));
    dictation::answer([Answer {
        after: SLOW,
        result: Ok("mit Diktat".into()),
    }]);
    answer_alert(
        Duration::from_millis(1200),
        "dictation-key-question",
        NSAlertFirstButtonReturn,
    );
    main_on_run_loop(move || dictation::click(target));
    wait_until(
        move || dictation::seen(target).listening,
        move |ok| editor_listening(ok, target),
    );
}

fn editor_listening(ok: bool, target: Target) {
    check(
        "the key Lowspeak keeps is taken over once and kept as the app's own",
        ide_ui_dictation::has_key(ApiProvider::OpenAi),
        "",
    );
    let seen = dictation::seen(target);
    check(
        "a listening editor says so, with a button that ends the dictation and tells VoiceOver",
        ok && seen.line == t("Höre zu …") && seen.label == t("Diktat beenden") && seen.value == seen.line,
        format!("{seen:?}"),
    );
    helpers::after(DRAW, move || {
        picture("dictation-editor-listening");
        if let Some(window) = controller().and_then(|c| c.window()) {
            window.makeFirstResponder(None);
        }
        dictation::click(target);
        wait_until(
            move || dictation::seen(target).spinning,
            move |ok| editor_transcribing(ok, target),
        );
    });
}

fn editor_transcribing(ok: bool, target: Target) {
    let seen = dictation::seen(target);
    check(
        "while it transcribes, the editor shows a spinner and says so",
        ok && seen.transcribing && seen.line == t("Transkribiere …"),
        format!("{seen:?}"),
    );
    picture("dictation-editor-transcribing");
    let expected = TEXT.replace(": ", ": mit Diktat");
    wait_until(
        move || editor().is_some_and(|v| v.document().text().to_string() == TEXT.replace(": ", ": mit Diktat")),
        move |ok| {
            let text = editor().map(|v| v.document().text().to_string()).unwrap_or_default();
            check(
                "the text goes in at the editor's cursor, though the focus left it",
                ok,
                format!("{text:?}, expected {expected:?}"),
            );
            undone();
        },
    );
}

fn undone() {
    let Some(view) = editor() else { return finish(SCENARIO) };
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    let text = view.document().text().to_string();
    check("one undo takes the dictation back", text == TEXT, format!("{text:?}"));
    composer();
}

/// The menu item dictates into the composer that has the focus.
fn composer() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(controller) = controller() else {
        return finish(SCENARIO);
    };
    let Some(tab) = ide_ui_agent::open_replay(&controller) else {
        check("an AI tab opens", false, "");
        return finish(SCENARIO);
    };
    tab.focus_composer();
    tab.type_in_composer("Bitte ");
    let Some(target) = focused_text().as_deref().and_then(dictation::target_of) else {
        check("the composer has a microphone", false, "");
        return finish(SCENARIO);
    };
    let (title, enabled) = dictation::menu_item(mtm);
    check(
        "the menu offers to start a dictation in the field with the focus",
        enabled && title == t("Diktat starten"),
        title,
    );
    dictation::answer([Answer {
        after: QUICK,
        result: Ok("prüfe das".into()),
    }]);
    dictation::menu(mtm);
    let key = tab.key;
    wait_until(
        move || dictation::seen(target).listening,
        move |ok| {
            let Some(mtm) = MainThreadMarker::new() else { return };
            let (title, _) = dictation::menu_item(mtm);
            check(
                "while it records, the menu item ends it",
                ok && title == t("Diktat beenden"),
                title,
            );
            picture("dictation-composer-listening");
            dictation::menu(mtm);
            wait_until(
                move || super::agent::tab_of(key).is_some_and(|t| t.composer_text() == "Bitte prüfe das"),
                move |ok| {
                    let text = super::agent::tab_of(key).map(|t| t.composer_text()).unwrap_or_default();
                    check("the text goes in at the composer's cursor", ok, format!("{text:?}"));
                    escape_recording(target, key);
                },
            );
        },
    );
}

/// Esc in the field ends a recording without text.
fn escape_recording(target: Target, key: AgentKey) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if let Some(controller) = controller() {
        super::agent::appearance(&controller, constant!(NSAppearanceNameDarkAqua));
    }
    dictation::menu(mtm);
    wait_until(
        move || dictation::seen(target).listening,
        move |ok| {
            picture("dictation-composer-listening-dark");
            escape();
            let seen = dictation::seen(target);
            check(
                "Esc ends a recording",
                ok && !seen.recording && seen.line.is_empty(),
                format!("{seen:?}"),
            );
            helpers::after(WIND_DOWN, move || escape_transcription(target, key));
        },
    );
}

/// Esc while the recording is transcribed drops its text.
fn escape_transcription(target: Target, key: AgentKey) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    dictation::answer([Answer {
        after: SLOW,
        result: Ok("verworfen".into()),
    }]);
    dictation::menu(mtm);
    wait_until(
        move || dictation::seen(target).listening,
        move |_| {
            let Some(mtm) = MainThreadMarker::new() else { return };
            dictation::menu(mtm);
            wait_until(
                move || dictation::seen(target).transcribing,
                move |_| {
                    escape();
                    helpers::after(DROPPED_TEXT, move || {
                        let text = super::agent::tab_of(key).map(|t| t.composer_text()).unwrap_or_default();
                        let seen = dictation::seen(target);
                        check(
                            "Esc while it transcribes drops the text",
                            text == "Bitte prüfe das" && !seen.transcribing,
                            format!("{text:?}, {seen:?}"),
                        );
                        failure();
                    });
                },
            );
        },
    );
}

/// A failed transcription says why in the interface language, in lowspeak's words.
fn failure() {
    if let Some(controller) = controller() {
        controller.open_file(&root().join(NOTES));
    }
    let Some(target) = editor_target() else {
        return finish(SCENARIO);
    };
    dictation::answer([Answer {
        after: QUICK,
        result: Err(TranscriptionError::InvalidResponse),
    }]);
    dictation::click(target);
    wait_until(
        move || dictation::seen(target).listening,
        move |_| {
            dictation::click(target);
            let expected = dictation::failure_text(&Failure::Transcription(TranscriptionError::InvalidResponse));
            let language = objc2_foundation::NSBundle::mainBundle()
                .preferredLocalizations()
                .firstObject()
                .map_or_else(|| "en".to_string(), |l| l.to_string());
            let lowspeak = lowspeak::dictation::texts_for_locale(&language).invalid_response;
            wait_until(
                move || !dictation::seen(target).line.is_empty() && !dictation::seen(target).transcribing,
                move |ok| {
                    let seen = dictation::seen(target);
                    check(
                        "a failure shows under the field in the interface language",
                        ok && seen.line == expected && expected == lowspeak,
                        format!("{seen:?}, {language}: {lowspeak:?}"),
                    );
                    picture("dictation-editor-failed-dark");
                    super::dictation_fn::run(target, settings);
                },
            );
        },
    );
}

fn settings() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if let Some(controller) = controller() {
        super::agent::appearance(&controller, constant!(objc2_app_kit::NSAppearanceNameAqua));
    }
    crate::settings::show_dictation(mtm);
    let lines = crate::settings::dictation_keys();
    check(
        "the settings tell the key taken over from the one Lowspeak still keeps",
        lines == ["OpenAI: im Schlüsselbund", "Groq: in Lowspeak"],
        lines.join(" | "),
    );
    helpers::wait_for(crate::settings::window, move |window| {
        if let Some(window) = window {
            snapshot(&window, "dictation-settings");
            window.setAppearance(NSAppearance::appearanceNamed(constant!(NSAppearanceNameDarkAqua)).as_deref());
            helpers::after(DRAW, move || {
                if let Some(window) = crate::settings::window() {
                    snapshot(&window, "dictation-settings-dark");
                }
                super::dictation_failures::settings_buttons();
            });
        } else {
            super::dictation_failures::settings_buttons();
        }
    });
}
