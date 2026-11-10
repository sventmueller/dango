//! The own agent's notices, card texts, reasons and rules in the interface language (E41); the crate hands them over
//! as values, and only what a provider or the system said is shown word for word.

use ide_agent::{
    Destructive, ErrorKind, LoadError, Note, Problem, ProviderError, Reason, Refusal, Rule, Severity, ToolNote, Wording,
};
use splat_i18n::{t, tf};

/// The wording the own agent's sessions are given.
pub struct Words;

impl Wording for Words {
    fn note(&self, note: &Note) -> String {
        match note {
            Note::LengthLimit => t("Die Antwort hat ihre Längengrenze erreicht.").to_string(),
            Note::Refused(None) => t("Das Modell hat die Antwort verweigert.").to_string(),
            Note::Refused(Some(category)) => tf("Das Modell hat die Antwort verweigert ({}).", &[category]),
            Note::Fallback { from, to } => tf("{} wollte nicht weitermachen; {} übernimmt.", &[from, to]),
            Note::Failed(error) => provider_error(error),
            Note::CompactionFailed(error) => tf(
                "Verdichten ist fehlgeschlagen, der Thread geht unverdichtet weiter: {}",
                &[&provider_error(error)],
            ),
            Note::NothingToCompact => t("Noch nichts zu verdichten.").to_string(),
        }
    }

    fn tool(&self, note: &ToolNote) -> String {
        tool_note(note)
    }

    fn reason(&self, reason: &Reason) -> String {
        match reason {
            Reason::Outside => t("Der Pfad liegt außerhalb des Projekts"),
            Reason::Destructive(Destructive::ForcePush) => {
                t("git push --force überschreibt den Verlauf auf dem Server")
            }
            Reason::Destructive(Destructive::ResetHard) => t("git reset --hard verwirft nicht gesicherte Änderungen"),
            Reason::Destructive(Destructive::Clean) => t("git clean -f löscht Dateien, die git nicht verfolgt"),
        }
        .to_string()
    }

    fn rule(&self, rule: &Rule) -> String {
        rule_text(rule)
    }

    fn retry(&self, error: &ProviderError) -> String {
        failure(error.kind).to_string()
    }
}

/// What a stored rule allows.
pub fn rule_text(rule: &Rule) -> String {
    match rule {
        Rule::Command { prefix } => format!("{prefix} …"),
        Rule::Edits => t("Dateien im Projekt ändern").to_string(),
        Rule::Fetch { host } => host.clone(),
    }
}

/// What went wrong with a request, with the provider's own words and status after it.
pub fn provider_error(error: &ProviderError) -> String {
    if error.kind == ErrorKind::NoKey {
        return tf(
            "{} braucht einen Schlüssel; trag ihn in den Einstellungen unter „Agent“ ein.",
            &[&error.message],
        );
    }
    let mut text = failure(error.kind).to_string();
    if let Some(status) = error.status {
        text.push_str(&format!(" ({status})"));
    }
    if error.message.trim().is_empty() {
        text
    } else {
        tf("{}: {}", &[&text, error.message.trim()])
    }
}

fn failure(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NoKey => t("Der Schlüssel fehlt"),
        ErrorKind::Auth => t("Der Anbieter hat den Schlüssel abgelehnt"),
        ErrorKind::Permission => t("Der Schlüssel darf dieses Modell oder diese Funktion nicht nutzen"),
        ErrorKind::Billing => t("Das Konto hat kein Guthaben oder keinen passenden Tarif"),
        ErrorKind::NotFound => t("Modell oder Adresse gibt es beim Anbieter nicht"),
        ErrorKind::TooLarge => t("Die Anfrage ist zu groß"),
        ErrorKind::ContextLength => t("Die Unterhaltung passt nicht mehr in das Kontextfenster des Modells"),
        ErrorKind::RateLimit => t("Zu viele Anfragen, der Anbieter bremst"),
        ErrorKind::Overloaded => t("Der Anbieter ist überlastet"),
        ErrorKind::Server => t("Der Anbieter hatte einen Fehler"),
        ErrorKind::BadRequest => t("Der Anbieter hat die Anfrage abgelehnt"),
        ErrorKind::Network => t("Der Anbieter war nicht erreichbar"),
        ErrorKind::Malformed => t("Die Antwort des Anbieters war nicht lesbar"),
        ErrorKind::Incomplete => t("Die Antwort brach vor ihrem Ende ab"),
    }
}

/// Why a stored thread could not be opened.
pub fn load_error(error: &LoadError) -> String {
    match error {
        LoadError::NoStore => t("Der Thread hat keinen Speicherort.").to_string(),
        LoadError::Unreadable(detail) => tf("Der gespeicherte Thread ließ sich nicht lesen: {}", &[detail]),
        LoadError::Version(version) => tf(
            "Der gespeicherte Thread stammt aus einer anderen Version (Format {}).",
            &[&version.to_string()],
        ),
    }
}

fn refusal(refusal: &Refusal) -> String {
    match refusal {
        Refusal::ReadOnly => t("Abgewiesen: Dieser Thread darf nur lesen.").to_string(),
        Refusal::NoNetwork => t("Abgewiesen: Die Sandbox des Threads hat kein Netzwerk.").to_string(),
        Refusal::OutsideSandbox(path) => tf(
            "Abgewiesen: {} liegt außerhalb des Projekts und der Ordner, die die Sandbox erlaubt.",
            &[path],
        ),
    }
}

fn tool_note(note: &ToolNote) -> String {
    match note {
        ToolNote::Cancelled => t("Abgebrochen, bevor der Aufruf lief.").to_string(),
        ToolNote::InvalidJson => t("Die Eingabe des Modells war kein gültiges JSON.").to_string(),
        ToolNote::CutOff => t("Die Längengrenze der Antwort hat die Eingabe abgeschnitten; nichts lief.").to_string(),
        ToolNote::MissingField(field) => tf("Der Eingabe fehlt das Feld „{}“.", &[field]),
        ToolNote::UnknownTool(name) => tf("Ein Werkzeug „{}“ gibt es nicht.", &[name]),
        ToolNote::Refused(why) => refusal(why),
        ToolNote::Declined => t("Von dir abgelehnt.").to_string(),
        ToolNote::Withdrawn => t("Zurückgezogen, als der Zug abgebrochen wurde.").to_string(),
        ToolNote::Io { path, detail } => tf("{}: {}", &[path, detail]),
        ToolNote::Folder(path) => tf("{} ist ein Ordner.", &[path]),
        ToolNote::TooLarge { path, bytes } => tf("{} ist zu groß zum Lesen ({} Bytes).", &[path, &bytes.to_string()]),
        ToolNote::Binary(path) => tf("{} enthält Binärdaten.", &[path]),
        ToolNote::EmptyEdit => t("Die Änderung nennt keinen Text, den sie ersetzt.").to_string(),
        ToolNote::NoChange => t("Die Änderung ersetzt einen Text durch denselben.").to_string(),
        ToolNote::NotFound(path) => tf("Der zu ersetzende Text steht nicht in {}.", &[path]),
        ToolNote::Ambiguous { path, count } => tf(
            "Der zu ersetzende Text steht {}-mal in {}.",
            &[&count.to_string(), path],
        ),
        ToolNote::NotWeb(url) => tf("„{}“ ist keine http- oder https-Adresse.", &[url]),
        ToolNote::NoHost(url) => tf("„{}“ nennt keinen Host.", &[url]),
        ToolNote::NotText { url, kind } => tf("{} ist {}, kein Text.", &[url, kind]),
        ToolNote::Fetch(error) => match error.status {
            Some(status) => tf("Die Seite ließ sich nicht abrufen ({}).", &[&status.to_string()]),
            None => tf("Die Seite ließ sich nicht abrufen: {}", &[&error.message]),
        },
        ToolNote::Pattern(detail) => tf("Das Suchmuster ist ungültig: {}", &[detail]),
        ToolNote::NotStarted(detail) => tf("Der Befehl startete nicht: {}", &[detail]),
        ToolNote::TodoStatus(status) => tf("Unbekannter Status „{}“ in der To-do-Liste.", &[status]),
        ToolNote::Diagnostics(problems) => problems.iter().map(problem).collect::<Vec<_>>().join("\n"),
        ToolNote::NoProblems => t("Die Sprachserver melden keine Probleme.").to_string(),
        ToolNote::NoLanguageServer => t("Kein Sprachserver meldet etwas zu diesem Projekt.").to_string(),
        ToolNote::EmptyFile(path) => tf("{} ist leer.", &[path]),
        ToolNote::PastEnd { path, lines } => tf("{} hat nur {} Zeilen.", &[path, &lines.to_string()]),
    }
}

/// One line of the diagnostics card, as a compiler writes it, its severity in the interface language.
fn problem(problem: &Problem) -> String {
    let severity = match problem.severity {
        Severity::Error => t("Fehler"),
        Severity::Warning => t("Warnung"),
        Severity::Info => t("Info"),
        Severity::Hint => t("Hinweis"),
    };
    format!(
        "{}:{}:{}: {severity}: {}",
        problem.path, problem.line, problem.column, problem.message
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_provider_error_is_worded_with_its_status_and_the_providers_words() {
        let mut error = ProviderError::new(ErrorKind::Auth, "invalid x-api-key");
        error.status = Some(401);
        assert_eq!(
            provider_error(&error),
            "Der Anbieter hat den Schlüssel abgelehnt (401): invalid x-api-key"
        );
        assert_eq!(
            provider_error(&ProviderError::new(ErrorKind::Incomplete, "")),
            "Die Antwort brach vor ihrem Ende ab"
        );
        assert!(provider_error(&ProviderError::new(ErrorKind::NoKey, "OpenAI")).starts_with("OpenAI braucht"));
    }

    #[test]
    fn diagnostics_name_their_severity_in_the_interface_language() {
        let found = ToolNote::Diagnostics(vec![
            Problem {
                path: "src/a.rs".into(),
                line: 3,
                column: 7,
                severity: Severity::Error,
                message: "mismatched types [rustc]".into(),
            },
            Problem {
                path: "src/b.rs".into(),
                line: 1,
                column: 1,
                severity: Severity::Hint,
                message: "consider this".into(),
            },
        ]);
        assert_eq!(
            Words.tool(&found),
            "src/a.rs:3:7: Fehler: mismatched types [rustc]\nsrc/b.rs:1:1: Hinweis: consider this"
        );
    }
}
