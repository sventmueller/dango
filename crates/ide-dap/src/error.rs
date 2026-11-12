//! Why a request to an adapter has no answer.

use std::fmt;

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

impl std::error::Error for Error {}
