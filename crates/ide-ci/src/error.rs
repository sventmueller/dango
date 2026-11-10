//! What can go wrong talking to a CI service, sorted by what the app does about it.

use std::fmt;

/// The result of a call to a CI service.
pub type Result<T> = std::result::Result<T, Error>;

/// Why a call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The service wants a token, or did not take the one it got.
    Unauthorized {
        /// The host that refused.
        host: String,
    },
    /// The token may not do this, such as re-running jobs with a read-only token.
    Forbidden {
        /// What the service said.
        message: String,
    },
    /// No such repository, run or job; GitHub also says so for private repositories without a token.
    NotFound {
        /// What was asked for.
        what: String,
    },
    /// Too many calls; the service takes more from this time on.
    RateLimited {
        /// When, in seconds since 1970, if the service said.
        until: Option<i64>,
    },
    /// The service answered with another error.
    Http {
        /// The status code.
        status: u16,
        /// What it said.
        message: String,
    },
    /// The service could not be reached.
    Network(String),
    /// The answer was not what the service documents.
    Decode(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unauthorized { host } => write!(f, "{host} wants a valid token"),
            Error::Forbidden { message } => write!(f, "not allowed: {message}"),
            Error::NotFound { what } => write!(f, "not found: {what}"),
            Error::RateLimited { until: Some(until) } => write!(f, "rate limited until {until}"),
            Error::RateLimited { until: None } => write!(f, "rate limited"),
            Error::Http { status, message } => write!(f, "HTTP {status}: {message}"),
            Error::Network(message) => write!(f, "network: {message}"),
            Error::Decode(message) => write!(f, "unexpected answer: {message}"),
        }
    }
}

impl std::error::Error for Error {}
