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
    /// The token lacks a scope or permission the call needs, such as GitLab's `api` for writing (E77, E177).
    MissingScope {
        /// The host whose token it is.
        host: String,
        /// What the service named as needed, such as `api` or `pull_requests=write`; empty when it named nothing.
        scope: String,
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
            Error::MissingScope { host, scope } if scope.is_empty() => {
                write!(f, "the token for {host} lacks write rights")
            }
            Error::MissingScope { host, scope } => write!(f, "the token for {host} lacks the scope {scope}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_failure_reads_as_what_went_wrong_with_what_the_service_said() {
        let cases = [
            (
                Error::Unauthorized {
                    host: "gitlab.com".into(),
                },
                "gitlab.com wants a valid token",
            ),
            (
                Error::Forbidden {
                    message: "read-only".into(),
                },
                "not allowed: read-only",
            ),
            (
                Error::MissingScope {
                    host: "gitlab.com".into(),
                    scope: "api".into(),
                },
                "the token for gitlab.com lacks the scope api",
            ),
            (
                Error::MissingScope {
                    host: "github.com".into(),
                    scope: String::new(),
                },
                "the token for github.com lacks write rights",
            ),
            (Error::NotFound { what: "run 7".into() }, "not found: run 7"),
            (
                Error::RateLimited {
                    until: Some(1_700_000_000),
                },
                "rate limited until 1700000000",
            ),
            (Error::RateLimited { until: None }, "rate limited"),
            (
                Error::Http {
                    status: 502,
                    message: "Bad Gateway".into(),
                },
                "HTTP 502: Bad Gateway",
            ),
            (Error::Network("refused".into()), "network: refused"),
            (Error::Decode("missing id".into()), "unexpected answer: missing id"),
        ];
        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected, "{error:?}");
        }
    }
}
