//! The end of the `dictation` scenario: the dictation settings' own controls (E103). Lowspeak's choices are taken
//! over once, each control writes the app's settings and the engine starts with them, and a missing local model is
//! downloaded from the tab.

use lowspeak::models::{Language, LocalModel, PostProcessingMode, TranscriptionMode};
use objc2::runtime::Sel;
use objc2::sel;
use objc2_app_kit::{NSPopUpButton, NSTokenField};
use objc2_foundation::{NSArray, NSString};

use super::check;
use super::helpers::wait_until;
use super::settings_pages::{act, control};
use crate::settings::dictation::LOCAL;
use crate::util::ns;
use ide_ui_dictation::choices;
use ide_ui_dictation::selftest as dictation;

const MODEL: LocalModel = LocalModel::Tiny;
const LANGUAGE: Language = Language::Fr;
const POST_PROCESSING: PostProcessingMode = PostProcessingMode::Cleanup;
const TERMS: [&str; 2] = ["Kubernetes", "Traefik"];

pub(crate) fn run() {
    let (offered, _) = crate::settings::dictation_rows();
    let took = control(sel!(ideDictationLowspeakChoices:), Some(1)).is_some_and(|b| act(&b));
    let (still_offered, _) = crate::settings::dictation_rows();
    let lowspeak = ide_ui_dictation::lowspeak_choices();
    check(
        "the settings offer Lowspeak's choices once and take them over",
        offered && took && !still_offered && choices::current() == lowspeak,
        format!("{offered} {took} {still_offered}: {:?}", choices::current()),
    );
    set_each();
}

fn choose(action: Sel, tag: isize) -> bool {
    control(action, None)
        .and_then(|c| c.downcast::<NSPopUpButton>().ok())
        .is_some_and(|popup| popup.selectItemWithTag(tag) && act(&popup))
}

fn index<T: PartialEq>(all: &[T], item: &T) -> isize {
    all.iter()
        .position(|i| i == item)
        .and_then(|i| isize::try_from(i).ok())
        .unwrap_or(-1)
}

fn type_terms() -> bool {
    let Some(field) = control(sel!(ideDictationVocabulary:), None).and_then(|c| c.downcast::<NSTokenField>().ok())
    else {
        return false;
    };
    let terms: Vec<_> = TERMS.iter().map(|term| ns(term)).collect();
    // SAFETY: A token field takes an array of strings as its value, one per token.
    unsafe { field.setObjectValue(Some(&NSArray::<NSString>::from_retained_slice(&terms))) };
    act(&field)
}

/// Every control set as a click would, then read back from the settings file and from the engine.
fn set_each() {
    let set = [
        choose(
            sel!(ideDictationTranscription:),
            LOCAL + index(&LocalModel::ALL, &MODEL),
        ),
        choose(sel!(ideDictationLanguage:), index(&Language::ALL, &LANGUAGE)),
        choose(
            sel!(ideDictationPostProcessing:),
            index(&PostProcessingMode::ALL, &POST_PROCESSING),
        ),
        choose(sel!(ideDictationMicrophone:), 0),
        type_terms(),
    ];
    let stored = choices::current();
    let engine = dictation::engine_choices();
    let settings = stored.settings;
    let expected = settings.mode == TranscriptionMode::Local
        && settings.local_model == MODEL
        && settings.language == LANGUAGE
        && settings.post_processing == POST_PROCESSING
        && stored.microphone.as_ref().is_some_and(|m| m.id == "selftest:usb")
        && stored.vocabulary == TERMS;
    check(
        "each control of the dictation settings is written to the app's settings and the engine starts with it",
        set.iter().all(|done| *done) && expected && engine == stored,
        format!("{set:?} {stored:?}"),
    );
    download();
}

/// The chosen model is missing, so the tab offers its download, which runs with progress and ends installed.
fn download() {
    let (_, shown) = crate::settings::dictation_rows();
    let started =
        !ide_ui_dictation::installed(MODEL) && control(sel!(ideDictationDownload:), None).is_some_and(|b| act(&b));
    let shows = || ide_ui_dictation::downloading().is_some() || ide_ui_dictation::installed(MODEL);
    super::helpers::wait_until(shows, move |_| {
        if let Some(window) = crate::settings::window() {
            super::snapshot(&window, "dictation-settings-download");
        }
        let running = ide_ui_dictation::downloading().is_some();
        wait_until(
            || ide_ui_dictation::installed(MODEL) && !crate::settings::dictation_rows().1,
            move |ok| {
                check(
                    "a missing local model is offered for download in the settings and installed from there",
                    shown && started && running && ok,
                    format!("{shown} {started} {running} {ok}"),
                );
                super::finish("dictation");
            },
        );
    });
}
