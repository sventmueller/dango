//! What a failed start of a debug session says to the user, with the command that installs a missing adapter.

use ide_core::explain::Explain;
use splat_i18n::{t, tf};

use crate::{CurrentError, StartError};

impl Explain for CurrentError {
    fn explain(&self) -> String {
        match self {
            CurrentError::Untrusted => {
                t("Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.").into()
            }
            CurrentError::Unsupported(language) => tf(
                "Für {}-Dateien gibt es keinen Debugger ohne Startkonfiguration.",
                &[language],
            ),
            CurrentError::Missing(program) => tf("{} ist nicht installiert.", &[program]),
            CurrentError::NoPackage => t("Die Datei gehört zu keinem Cargo-Paket.").into(),
            CurrentError::WhichBinary(names) => tf(
                "Das Paket hat mehrere Programme ({}); lege in .vscode/launch.json fest, welches.",
                &[&names.join(", ")],
            ),
            CurrentError::Build(errors) => tf("Der Build ist fehlgeschlagen:\n{}", &[errors]),
            CurrentError::Cancelled => t("Der Build wurde abgebrochen.").into(),
            CurrentError::NoProgram => t("Der Build hat kein Programm erzeugt.").into(),
            CurrentError::LibraryOnly(package) => tf(
                "„{}“ ist eine Bibliothek ohne Programm, und diese Datei hat keine Tests. Debugge eine Datei mit main oder Tests, oder lege in .vscode/launch.json fest, welches Programm startet.",
                &[package],
            ),
        }
    }
}

impl Explain for StartError {
    fn explain(&self) -> String {
        match self {
            StartError::Untrusted => {
                t("Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.").into()
            }
            StartError::NeedsProcess => t("Wähle einen Prozess, an den sich der Debugger anhängt.").into(),
            StartError::Missing(missing) => match (&missing.command, missing.needs) {
                (_, Some(needs)) => tf("{} fehlt; es braucht {}.", &[missing.program, needs]),
                (Some(command), None) => tf("{} fehlt. Installiere es mit: {}", &[missing.program, command]),
                (None, None) => tf("{} fehlt.", &[missing.program]),
            },
            StartError::Build(error) => error.explain(),
            StartError::Io(error) => tf("Der Debug-Adapter konnte nicht starten: {}", &[&error.explain()]),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{CurrentError, Missing, StartError};

    use super::*;

    #[test]
    fn why_the_current_program_cannot_be_debugged_is_said_in_the_users_words() {
        let cases = [
            (
                CurrentError::Untrusted,
                "Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.",
            ),
            (
                CurrentError::Unsupported("cpp".into()),
                "Für cpp-Dateien gibt es keinen Debugger ohne Startkonfiguration.",
            ),
            (CurrentError::Missing("cargo"), "cargo ist nicht installiert."),
            (CurrentError::NoPackage, "Die Datei gehört zu keinem Cargo-Paket."),
            (
                CurrentError::WhichBinary(vec!["a".into(), "b".into()]),
                "Das Paket hat mehrere Programme (a, b); lege in .vscode/launch.json fest, welches.",
            ),
            (
                CurrentError::Build("E0425".into()),
                "Der Build ist fehlgeschlagen:\nE0425",
            ),
            (CurrentError::Cancelled, "Der Build wurde abgebrochen."),
            (CurrentError::NoProgram, "Der Build hat kein Programm erzeugt."),
            (
                CurrentError::LibraryOnly("lib".into()),
                "„lib“ ist eine Bibliothek ohne Programm, und diese Datei hat keine Tests. Debugge eine Datei mit main oder Tests, oder lege in .vscode/launch.json fest, welches Programm startet.",
            ),
        ];
        for (error, words) in cases {
            assert_eq!(error.explain(), words);
        }
    }

    #[test]
    fn why_a_session_did_not_start_names_the_missing_adapter_and_how_to_get_it() {
        let missing = |needs, command: Option<&str>| {
            StartError::Missing(Missing {
                kind: crate::AdapterKind::Go,
                program: "dlv",
                needs,
                install: None,
                command: command.map(str::to_string),
            })
        };
        let cases = [
            (
                StartError::Untrusted,
                "Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.".to_string(),
            ),
            (
                StartError::NeedsProcess,
                "Wähle einen Prozess, an den sich der Debugger anhängt.".to_string(),
            ),
            (
                missing(Some("go"), Some("go install")),
                "dlv fehlt; es braucht go.".to_string(),
            ),
            (
                missing(None, Some("go install dlv")),
                "dlv fehlt. Installiere es mit: go install dlv".to_string(),
            ),
            (missing(None, None), "dlv fehlt.".to_string()),
            (
                StartError::Build(CurrentError::NoPackage),
                "Die Datei gehört zu keinem Cargo-Paket.".to_string(),
            ),
            (
                StartError::Io(std::io::Error::other("refused")),
                "Der Debug-Adapter konnte nicht starten: refused".to_string(),
            ),
        ];
        for (error, words) in cases {
            assert_eq!(error.explain(), words);
        }
    }
}
