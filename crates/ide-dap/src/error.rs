//! Why a request to an adapter has no answer, and why the app refuses one of the adapter's.

use std::{fmt, io};

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The adapter answered with an error; the message is in its words, for the user.
    Adapter {
        /// The request.
        command: String,
        /// What the adapter said.
        message: String,
    },
    /// No answer in time.
    Timeout,
    /// The session ended or the adapter is gone.
    Ended,
    /// The adapter does not offer this, by its capabilities.
    Unsupported,
    /// The request does not fit the session's state, such as stepping while the program runs.
    NotStopped,
    /// The adapter stopped reading.
    Congested,
    /// The answer could not be read.
    Malformed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Adapter { command, message } => write!(f, "{command}: {message}"),
            Error::Timeout => f.write_str("the debug adapter did not answer in time"),
            Error::Ended => f.write_str("the debug session has ended"),
            Error::Unsupported => f.write_str("the debug adapter does not support this"),
            Error::NotStopped => f.write_str("the program is not stopped"),
            Error::Congested => f.write_str("the debug adapter stopped reading"),
            Error::Malformed(why) => write!(f, "the debug adapter's answer could not be read: {why}"),
        }
    }
}

impl ide_core::explain::Explain for Error {
    fn explain(&self) -> String {
        use splat_i18n::{t, tf};
        match self {
            Error::Adapter { message, .. } => message.clone(),
            Error::Timeout => t("Der Debug-Adapter hat nicht rechtzeitig geantwortet.").to_string(),
            Error::Ended => t("Die Debug-Sitzung ist beendet.").to_string(),
            Error::Unsupported => t("Der Debug-Adapter kann das nicht.").to_string(),
            Error::NotStopped => t("Das Programm ist nicht angehalten.").to_string(),
            Error::Congested => t("Der Debug-Adapter liest nicht mehr.").to_string(),
            Error::Malformed(why) => tf("Unerwartete Antwort: {}", &[why]),
        }
    }
}

impl std::error::Error for Error {}

/// Why the app refuses a request of the adapter; its `Display` is the message the adapter gets.
#[derive(Debug)]
pub enum Refusal {
    /// The session ended before the request came.
    Ended,
    /// The app does not take this request.
    Unsupported,
    /// Nobody answered it.
    Unanswered,
    /// `runInTerminal` named no program.
    NoProgram,
    /// The window that would show the terminal is gone.
    WindowClosed,
    /// The terminal did not start the program.
    TerminalNotStarted,
    /// The adapter has a single connection, so no session can start beneath this one.
    SingleConnection,
    /// The session beneath this one could not be reached or started.
    Child(io::Error),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Ended => f.write_str("the session has ended"),
            Refusal::Unsupported => f.write_str("not supported"),
            Refusal::Unanswered => f.write_str("the request was not answered"),
            Refusal::NoProgram => f.write_str("runInTerminal without a program"),
            Refusal::WindowClosed => f.write_str("the window closed"),
            Refusal::TerminalNotStarted => f.write_str("the terminal did not start"),
            Refusal::SingleConnection => f.write_str("child sessions need an adapter that takes several connections"),
            Refusal::Child(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Refusal::Child(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Refusal};

    #[test]
    fn every_reason_reads_as_a_sentence() {
        let cases = [
            (
                Error::Adapter {
                    command: "evaluate".into(),
                    message: "no such name".into(),
                },
                "evaluate: no such name",
            ),
            (Error::Timeout, "the debug adapter did not answer in time"),
            (Error::Ended, "the debug session has ended"),
            (Error::Unsupported, "the debug adapter does not support this"),
            (Error::NotStopped, "the program is not stopped"),
            (Error::Congested, "the debug adapter stopped reading"),
            (
                Error::Malformed("no body".into()),
                "the debug adapter's answer could not be read: no body",
            ),
        ];
        for (error, words) in cases {
            assert_eq!(error.to_string(), words);
        }
    }

    #[test]
    fn a_refusal_tells_the_adapter_why_and_keeps_its_source() {
        assert_eq!(Refusal::Unsupported.to_string(), "not supported");
        assert_eq!(Refusal::Unanswered.to_string(), "the request was not answered");
        let child = Refusal::Child(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        assert_eq!(
            child.to_string(),
            std::io::Error::from(std::io::ErrorKind::ConnectionRefused).to_string()
        );
        assert!(std::error::Error::source(&child).is_some());
    }
}
