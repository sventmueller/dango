//! The texts of `Error::Unavailable`, in the interface language, for the status bar.

use splat_i18n::{t, tf};

pub(crate) fn unreachable(service: &str) -> String {
    tf(
        "{} ist nicht erreichbar. Starte den Server oder prüfe die Adresse in den Einstellungen.",
        &[service],
    )
}

pub(crate) fn unknown_model(service: &str, model: &str) -> String {
    tf(
        "{} kennt das Modell {} nicht. Wähle in den Einstellungen ein installiertes Modell.",
        &[service, model],
    )
}

pub(crate) fn no_fill_in(model: &str) -> String {
    tf(
        "Das Modell {} kann keine Lücken im Code füllen. Wähle in den Einstellungen ein Code-Modell wie Qwen2.5 Coder.",
        &[model],
    )
}

pub(crate) fn no_model() -> String {
    t("Kein Modell gewählt. Wähle eines in den Einstellungen.").to_string()
}

pub(crate) fn failed(service: &str, message: &str) -> String {
    tf("{} meldet einen Fehler: {}", &[service, message])
}

pub(crate) fn copilot_starting() -> String {
    t("Copilot startet noch.").to_string()
}

pub(crate) fn copilot_down() -> String {
    t("Copilot läuft nicht mehr. Starte es in den Einstellungen neu.").to_string()
}

pub(crate) fn no_subscription() -> String {
    t("Für dein GitHub-Konto ist Copilot nicht freigeschaltet. Wähle auf github.com ein Abo, auch das kostenlose genügt.")
        .to_string()
}
