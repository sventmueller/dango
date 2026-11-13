//! The questions before a dictation that needs an API key the app lacks: take Lowspeak's over, or enter one.

use lowspeak::models::ApiProvider;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSModalResponse, NSSecureTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::keys::{self, Keys, Question, TakeOver};
use ide_ui_common::util::ns;

/// Asks `question` and keeps the key the user gives; "not now" leaves the dictation to fall back or fail.
pub fn ask(question: Question, keys: &Keys) {
    match question {
        Question::TakeOver(provider) => offer_takeover(provider, keys),
        Question::Enter(provider) => enter(provider, keys, None),
    }
}

fn offer_takeover(provider: ApiProvider, keys: &Keys) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let name = ide_product::NAME;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(&tf(
        "Lowspeak hat einen Schlüssel für {}. Für {} übernehmen?",
        &[provider.display_name(), name],
    )));
    alert.setInformativeText(&ns(&tf(
        "{} liest ihn dafür einmal, und macOS fragt nach. Wähle dort „Erlauben“, nicht „Immer erlauben“: So bleibt \
         Lowspeaks Eintrag, wie er ist. {} legt den Schlüssel in einem eigenen Eintrag ab.",
        &[name, name],
    )));
    alert.addButtonWithTitle(&ns(t("Übernehmen")));
    alert.addButtonWithTitle(&ns(t("Selbst eingeben …")));
    alert.addButtonWithTitle(&ns(t("Nicht jetzt")));
    let answer = run(&alert);
    if answer == NSAlertFirstButtonReturn {
        match keys::take_over(keys, provider, super::engine::read_lowspeak_key) {
            TakeOver::Done(_) => {}
            TakeOver::Unread(reason) => {
                ide_ui_common::diag::report("dictation", format!("taking Lowspeak's key over: {reason}"));
                enter(
                    provider,
                    keys,
                    Some(t(
                        "Lowspeaks Schlüssel ließ sich nicht lesen. Gib den Schlüssel selbst ein.",
                    )),
                );
            }
        }
    } else if answer == NSAlertSecondButtonReturn {
        enter(provider, keys, None);
    }
}

/// Asks for a key for `provider`, with `problem` above the field if the last attempt failed; again after a key in the
/// wrong format.
pub fn enter(provider: ApiProvider, keys: &Keys, problem: Option<&str>) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(&tf("API-Schlüssel für {}", &[provider.display_name()])));
    let explanation = tf(
        "Lowspeak transkribiert online mit {}. {} braucht dafür einen eigenen Schlüssel und legt ihn im \
         Schlüsselbund ab.",
        &[provider.display_name(), ide_product::NAME],
    );
    alert.setInformativeText(&ns(&match problem {
        Some(problem) => format!("{problem}\n\n{explanation}"),
        None => explanation,
    }));
    let field = NSSecureTextField::initWithFrame(
        NSSecureTextField::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 24.0)),
    );
    field.setPlaceholderString(Some(&ns(match provider {
        ApiProvider::OpenAi => "sk-…",
        ApiProvider::Groq => "gsk_…",
    })));
    alert.setAccessoryView(Some(&field));
    alert.addButtonWithTitle(&ns(t("Sichern")));
    alert.addButtonWithTitle(&ns(t("Nicht jetzt")));
    alert.window().setInitialFirstResponder(Some(&field));
    if run(&alert) != NSAlertFirstButtonReturn {
        return;
    }
    let typed = field.stringValue().to_string();
    if typed.trim().is_empty() {
        return;
    }
    if let Err(error) = keys.save(&typed) {
        ide_ui_common::diag::report("dictation", format!("saving a key: {error}"));
        enter(
            provider,
            keys,
            Some(t("Das ist kein Schlüssel von OpenAI (sk-…) oder Groq (gsk_…).")),
        );
    }
}

fn run(alert: &NSAlert) -> NSModalResponse {
    alert.runModal()
}
