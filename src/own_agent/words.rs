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
            Reason::Outside => t("Der Pfad liegt außerhalb des Projekts").to_string(),
            Reason::Destructive(why) => destructive_text(*why),
        }
    }

    fn rule(&self, rule: &Rule) -> String {
        rule_text(rule)
    }

    fn retry(&self, error: &ProviderError) -> String {
        failure(error.kind).to_string()
    }
}

/// What a destructive git command throws away, for every agent's permission card (E27).
pub fn destructive_text(why: Destructive) -> String {
    match why {
        Destructive::ForcePush => t("git push --force überschreibt den Verlauf auf dem Server"),
        Destructive::ResetHard => t("git reset --hard verwirft nicht gesicherte Änderungen"),
        Destructive::Clean => t("git clean -f löscht Dateien, die git nicht verfolgt"),
        Destructive::StashDrop => t("git stash drop löscht einen Stash endgültig"),
        Destructive::StashClear => t("git stash clear löscht alle Stashes endgültig"),
    }
    .to_string()
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
    fn a_call_that_did_not_run_says_why() {
        let cases = [
            (ToolNote::Cancelled, "Abgebrochen, bevor der Aufruf lief."),
            (ToolNote::InvalidJson, "Die Eingabe des Modells war kein gültiges JSON."),
            (
                ToolNote::CutOff,
                "Die Längengrenze der Antwort hat die Eingabe abgeschnitten; nichts lief.",
            ),
            (
                ToolNote::MissingField("path".into()),
                "Der Eingabe fehlt das Feld „path“.",
            ),
            (
                ToolNote::UnknownTool("rm_rf".into()),
                "Ein Werkzeug „rm_rf“ gibt es nicht.",
            ),
            (
                ToolNote::Refused(Refusal::ReadOnly),
                "Abgewiesen: Dieser Thread darf nur lesen.",
            ),
            (
                ToolNote::Refused(Refusal::NoNetwork),
                "Abgewiesen: Die Sandbox des Threads hat kein Netzwerk.",
            ),
            (
                ToolNote::Refused(Refusal::OutsideSandbox("/etc/hosts".into())),
                "Abgewiesen: /etc/hosts liegt außerhalb des Projekts und der Ordner, die die Sandbox erlaubt.",
            ),
            (ToolNote::Declined, "Von dir abgelehnt."),
            (ToolNote::Withdrawn, "Zurückgezogen, als der Zug abgebrochen wurde."),
        ];
        for (note, words) in cases {
            assert_eq!(Words.tool(&note), words, "{note:?}");
        }
    }

    #[test]
    fn a_call_that_failed_on_a_file_a_page_or_a_command_names_it() {
        let fetch = |status| ProviderError {
            status,
            ..ProviderError::new(ErrorKind::Network, "connection reset")
        };
        let cases = [
            (
                ToolNote::Io {
                    path: "src/a.rs".into(),
                    detail: "Permission denied".into(),
                },
                "src/a.rs: Permission denied",
            ),
            (ToolNote::Folder("src".into()), "src ist ein Ordner."),
            (
                ToolNote::TooLarge {
                    path: "big.bin".into(),
                    bytes: 9000,
                },
                "big.bin ist zu groß zum Lesen (9000 Bytes).",
            ),
            (ToolNote::Binary("logo.png".into()), "logo.png enthält Binärdaten."),
            (ToolNote::EmptyEdit, "Die Änderung nennt keinen Text, den sie ersetzt."),
            (ToolNote::NoChange, "Die Änderung ersetzt einen Text durch denselben."),
            (
                ToolNote::NotFound("src/a.rs".into()),
                "Der zu ersetzende Text steht nicht in src/a.rs.",
            ),
            (
                ToolNote::Ambiguous {
                    path: "src/a.rs".into(),
                    count: 3,
                },
                "Der zu ersetzende Text steht 3-mal in src/a.rs.",
            ),
            (
                ToolNote::NotWeb("ftp://x".into()),
                "„ftp://x“ ist keine http- oder https-Adresse.",
            ),
            (ToolNote::NoHost("https://".into()), "„https://“ nennt keinen Host."),
            (
                ToolNote::NotText {
                    url: "https://x.org/a.zip".into(),
                    kind: "application/zip".into(),
                },
                "https://x.org/a.zip ist application/zip, kein Text.",
            ),
            (
                ToolNote::Fetch(fetch(Some(404))),
                "Die Seite ließ sich nicht abrufen (404).",
            ),
            (
                ToolNote::Fetch(fetch(None)),
                "Die Seite ließ sich nicht abrufen: connection reset",
            ),
            (
                ToolNote::Pattern("unclosed [".into()),
                "Das Suchmuster ist ungültig: unclosed [",
            ),
            (
                ToolNote::NotStarted("no such file".into()),
                "Der Befehl startete nicht: no such file",
            ),
            (
                ToolNote::TodoStatus("later".into()),
                "Unbekannter Status „later“ in der To-do-Liste.",
            ),
            (ToolNote::NoProblems, "Die Sprachserver melden keine Probleme."),
            (
                ToolNote::NoLanguageServer,
                "Kein Sprachserver meldet etwas zu diesem Projekt.",
            ),
            (ToolNote::EmptyFile("a.txt".into()), "a.txt ist leer."),
            (
                ToolNote::PastEnd {
                    path: "a.txt".into(),
                    lines: 12,
                },
                "a.txt hat nur 12 Zeilen.",
            ),
        ];
        for (note, words) in cases {
            assert_eq!(Words.tool(&note), words, "{note:?}");
        }
    }

    #[test]
    fn notes_reasons_and_rules_are_worded_for_the_user() {
        let failed = ProviderError::new(ErrorKind::Overloaded, "busy");
        let notes = [
            (
                Note::LengthLimit,
                "Die Antwort hat ihre Längengrenze erreicht.".to_string(),
            ),
            (
                Note::Refused(None),
                "Das Modell hat die Antwort verweigert.".to_string(),
            ),
            (
                Note::Refused(Some("cyber".into())),
                "Das Modell hat die Antwort verweigert (cyber).".to_string(),
            ),
            (
                Note::Fallback {
                    from: "opus".into(),
                    to: "sonnet".into(),
                },
                "opus wollte nicht weitermachen; sonnet übernimmt.".to_string(),
            ),
            (
                Note::Failed(failed.clone()),
                "Der Anbieter ist überlastet: busy".to_string(),
            ),
            (
                Note::CompactionFailed(failed.clone()),
                "Verdichten ist fehlgeschlagen, der Thread geht unverdichtet weiter: Der Anbieter ist überlastet: busy"
                    .to_string(),
            ),
            (Note::NothingToCompact, "Noch nichts zu verdichten.".to_string()),
        ];
        for (note, words) in notes {
            assert_eq!(Words.note(&note), words, "{note:?}");
        }
        assert_eq!(Words.reason(&Reason::Outside), "Der Pfad liegt außerhalb des Projekts");
        assert_eq!(
            Words.reason(&Reason::Destructive(Destructive::ForcePush)),
            "git push --force überschreibt den Verlauf auf dem Server"
        );
        let destructive = [
            Destructive::ForcePush,
            Destructive::ResetHard,
            Destructive::Clean,
            Destructive::StashDrop,
            Destructive::StashClear,
        ]
        .map(destructive_text);
        assert!(destructive[1].starts_with("git reset --hard"));
        assert!(destructive[2].starts_with("git clean -f"));
        assert!(destructive[3].starts_with("git stash drop"));
        assert!(destructive[4].starts_with("git stash clear"));
        let rules = [
            Rule::Command {
                prefix: "cargo test".into(),
            },
            Rule::Edits,
            Rule::Fetch { host: "docs.rs".into() },
        ];
        assert_eq!(
            rules.iter().map(|r| Words.rule(r)).collect::<Vec<_>>(),
            ["cargo test …", "Dateien im Projekt ändern", "docs.rs"]
        );
        assert_eq!(Words.retry(&failed), "Der Anbieter ist überlastet");
    }

    #[test]
    fn every_kind_of_failure_has_words_of_its_own() {
        let kinds = [
            ErrorKind::NoKey,
            ErrorKind::Auth,
            ErrorKind::Permission,
            ErrorKind::Billing,
            ErrorKind::NotFound,
            ErrorKind::TooLarge,
            ErrorKind::ContextLength,
            ErrorKind::RateLimit,
            ErrorKind::Overloaded,
            ErrorKind::Server,
            ErrorKind::BadRequest,
            ErrorKind::Network,
            ErrorKind::Malformed,
            ErrorKind::Incomplete,
        ];
        let words: std::collections::BTreeSet<&str> = kinds.iter().map(|k| failure(*k)).collect();
        assert_eq!(words.len(), kinds.len());
        assert_eq!(
            provider_error(&ProviderError {
                status: Some(429),
                ..ProviderError::new(ErrorKind::RateLimit, "  slow down  ")
            }),
            "Zu viele Anfragen, der Anbieter bremst (429): slow down"
        );
    }

    #[test]
    fn a_stored_thread_that_does_not_open_says_why() {
        assert_eq!(load_error(&LoadError::NoStore), "Der Thread hat keinen Speicherort.");
        assert_eq!(
            load_error(&LoadError::Unreadable("EOF at line 3".into())),
            "Der gespeicherte Thread ließ sich nicht lesen: EOF at line 3"
        );
        assert_eq!(
            load_error(&LoadError::Version(7)),
            "Der gespeicherte Thread stammt aus einer anderen Version (Format 7)."
        );
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
