//! How the CI panel shows states, ages and errors.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ide_ci::{Error, Source, State};
use objc2::rc::Retained;
use objc2_app_kit::NSColor;
use splat_i18n::{t, tf};

/// The symbol of `state`; its colour and word say the same for those who do not see the colour.
pub(crate) fn symbol(state: State) -> &'static str {
    match state {
        State::Queued => "clock",
        State::Running => "circle.dotted.circle",
        State::Success => "checkmark.circle.fill",
        State::Failed => "xmark.circle.fill",
        State::Cancelled => "slash.circle",
        State::Skipped => "arrow.uturn.right.circle",
        State::Manual => "hand.raised.circle",
        State::Neutral => "minus.circle",
    }
}

pub(crate) fn color(state: State) -> Retained<NSColor> {
    match state {
        State::Success => NSColor::systemGreenColor(),
        State::Failed => NSColor::systemRedColor(),
        State::Running => NSColor::systemOrangeColor(),
        State::Queued | State::Manual => NSColor::systemBlueColor(),
        State::Cancelled | State::Skipped | State::Neutral => NSColor::secondaryLabelColor(),
    }
}

pub(crate) fn word(state: State) -> &'static str {
    match state {
        State::Queued => t("wartet"),
        State::Running => t("läuft"),
        State::Success => t("erfolgreich"),
        State::Failed => t("fehlgeschlagen"),
        State::Cancelled => t("abgebrochen"),
        State::Skipped => t("übersprungen"),
        State::Manual => t("wartet auf Start von Hand"),
        State::Neutral => t("ohne Ergebnis"),
    }
}

/// How long ago `seconds` since 1970 was.
pub(crate) fn ago(seconds: i64) -> String {
    let at = UNIX_EPOCH + Duration::from_secs(u64::try_from(seconds).unwrap_or(0));
    ide_ui_agent::age(at.min(SystemTime::now()))
}

/// How long something took that ran for `seconds`.
pub(crate) fn took(seconds: i64) -> String {
    ide_ui_agent::elapsed(Duration::from_secs(u64::try_from(seconds).unwrap_or(0)))
}

/// Where the token came from, in words.
pub(crate) fn source(source: &Source) -> String {
    match source {
        Source::Keychain => t("Schlüsselbund").into(),
        Source::Gh => "gh".into(),
        Source::Glab => "glab".into(),
        Source::Environment(name) => tf("Umgebung ({})", &[name]),
    }
}

/// What went wrong, in the panel's words.
pub(crate) fn problem(error: &Error) -> String {
    match error {
        Error::Unauthorized { host } => tf("{} nimmt den Token nicht an.", &[host]),
        Error::Forbidden { message } => tf("Der Token darf das nicht: {}", &[message]),
        Error::NotFound { .. } => t("Das Repository gibt es dort nicht, oder der Token sieht es nicht.").into(),
        Error::RateLimited { until: Some(until) } => tf(
            "Zu viele Anfragen; der Dienst nimmt in {} wieder welche an.",
            &[&took((*until - ide_ci::now()).max(0))],
        ),
        Error::RateLimited { until: None } => t("Zu viele Anfragen; versuch es gleich noch einmal.").into(),
        Error::Http { status, message } => tf("Der Dienst meldet {}: {}", &[&status.to_string(), message]),
        Error::Network(message) => tf("Der Dienst ist nicht erreichbar: {}", &[message]),
        Error::Decode(message) => tf("Unerwartete Antwort: {}", &[message]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATES: [State; 8] = [
        State::Queued,
        State::Running,
        State::Success,
        State::Failed,
        State::Cancelled,
        State::Skipped,
        State::Manual,
        State::Neutral,
    ];

    #[test]
    fn every_state_has_a_symbol_and_a_word_of_its_own() {
        let symbols: std::collections::HashSet<_> = STATES.iter().map(|s| symbol(*s)).collect();
        let words: std::collections::HashSet<_> = STATES.iter().map(|s| word(*s)).collect();
        assert_eq!(symbols.len(), STATES.len());
        assert_eq!(words.len(), STATES.len());
    }

    #[test]
    fn a_token_names_where_it_came_from() {
        assert_eq!(source(&Source::Keychain), t("Schlüsselbund"));
        assert_eq!(source(&Source::Gh), "gh");
        assert_eq!(source(&Source::Glab), "glab");
        assert!(source(&Source::Environment("GITLAB_TOKEN")).contains("GITLAB_TOKEN"));
    }

    #[test]
    fn each_error_says_what_the_service_answered() {
        let said = [
            (
                Error::Unauthorized {
                    host: "gitlab.com".into(),
                },
                "gitlab.com",
            ),
            (
                Error::Forbidden {
                    message: "read-only".into(),
                },
                "read-only",
            ),
            (
                Error::Http {
                    status: 502,
                    message: "Bad Gateway".into(),
                },
                "502: Bad Gateway",
            ),
            (Error::Network("refused".into()), "refused"),
            (Error::Decode("missing id".into()), "missing id"),
        ];
        for (error, words) in said {
            assert!(problem(&error).contains(words), "{error:?}: {}", problem(&error));
        }
        assert_eq!(
            problem(&Error::NotFound { what: "run".into() }),
            t("Das Repository gibt es dort nicht, oder der Token sieht es nicht.")
        );
        assert_eq!(
            problem(&Error::RateLimited { until: None }),
            t("Zu viele Anfragen; versuch es gleich noch einmal.")
        );
        let later = problem(&Error::RateLimited {
            until: Some(ide_ci::now() + 120),
        });
        assert!(later.contains(&took(120)) || later.contains(&took(119)), "{later}");
    }
}
