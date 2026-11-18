//! What the installers of language servers and debug adapters share: their stages, their progress and why they fail.

use std::fmt;
use std::io;

use ide_process::Ending;

use crate::DownloadError;

/// A stage of an installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Finding out what to download.
    Resolving,
    /// Downloading.
    Downloading,
    /// Checking what was downloaded.
    Verifying,
    /// Unpacking it.
    Unpacking,
    /// Running the installing program, such as `go install`.
    Running,
    /// Installed.
    Done,
}

/// What an installation reports while it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// A new stage began.
    Step(Step),
    /// Bytes downloaded so far.
    Bytes {
        /// So far.
        done: u64,
        /// Of this many, if known.
        total: Option<u64>,
    },
    /// A line the installing program printed.
    Output(String),
}

/// Why an installation failed.
#[derive(Debug)]
pub enum InstallError {
    /// The user cancelled it.
    Cancelled,
    /// A program it needs is not on the login shell's PATH, such as `go`, `node` or `npm`.
    Missing(&'static str),
    /// The installing program failed.
    Failed {
        /// Which.
        program: String,
        /// How it ended; `None` if it took too long.
        ending: Option<Ending>,
        /// What it printed to standard error.
        output: String,
    },
    /// The network failed or the release has no download for this Mac.
    Network(String),
    /// The download does not match the digest the release publishes.
    Checksum {
        /// Published.
        expected: String,
        /// Downloaded.
        actual: String,
    },
    /// The release publishes no digest, so the download cannot be checked.
    Unverifiable,
    /// The installed program does not start.
    Unusable(String),
    /// Writing the tools folder failed.
    Io(io::Error),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::Cancelled => f.write_str("the installation was cancelled"),
            InstallError::Missing(program) => write!(f, "{program} is not installed"),
            InstallError::Failed {
                program,
                ending,
                output,
            } => {
                write!(f, "{program} failed ({ending:?}): {}", output.trim())
            }
            InstallError::Network(why) => write!(f, "the download failed: {why}"),
            InstallError::Checksum { expected, actual } => {
                write!(f, "the download is damaged: expected {expected}, got {actual}")
            }
            InstallError::Unverifiable => f.write_str("the release publishes no digest to check the download against"),
            InstallError::Unusable(why) => write!(f, "the installed program does not start: {why}"),
            InstallError::Io(error) => write!(f, "the tools folder could not be written: {error}"),
        }
    }
}

impl ide_core::explain::Explain for InstallError {
    fn explain(&self) -> String {
        use splat_i18n::{t, tf};
        match self {
            InstallError::Cancelled => t("Die Installation wurde abgebrochen.").to_string(),
            InstallError::Missing(program) => tf("{} ist nicht installiert.", &[program]),
            InstallError::Failed {
                program, ending: None, ..
            } => tf("{} hat zu lange gebraucht und wurde beendet.", &[program]),
            InstallError::Failed { program, output, .. } => tf("{} ist fehlgeschlagen: {}", &[program, output.trim()]),
            InstallError::Network(why) => tf("Der Download ist fehlgeschlagen: {}", &[why]),
            InstallError::Checksum { .. } => {
                t("Der Download stimmt nicht mit seiner Prüfsumme überein und wurde verworfen").to_string()
            }
            InstallError::Unverifiable => {
                t("Das Release veröffentlicht keine Prüfsumme, gegen die sich der Download prüfen ließe.").to_string()
            }
            InstallError::Unusable(why) => tf("Das installierte Programm startet nicht: {}", &[why]),
            InstallError::Io(error) => tf(
                "Der Ordner für Werkzeuge ließ sich nicht schreiben: {}",
                &[&error.explain()],
            ),
        }
    }
}

impl std::error::Error for InstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InstallError::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> InstallError {
        InstallError::Io(error)
    }
}

impl From<DownloadError> for InstallError {
    fn from(error: DownloadError) -> InstallError {
        match error {
            DownloadError::Cancelled => InstallError::Cancelled,
            DownloadError::Network(error) => InstallError::Network(error.to_string()),
            error @ DownloadError::TooLarge(_) => InstallError::Network(error.to_string()),
            DownloadError::Io(error) => InstallError::Io(error),
        }
    }
}
