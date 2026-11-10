//! Why a request to a language server brought no answer.

use std::fmt;

/// Why a request to a language server brought no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The server answered with an error.
    Server {
        /// The JSON-RPC error code, such as -32801 when the document changed while the server worked on it.
        code: i64,
        /// The server's message.
        message: String,
    },
    /// No answer came in the time allowed; the server was asked to cancel.
    Timeout,
    /// The caller cancelled the request; the server was asked to cancel.
    Cancelled,
    /// The server is starting, restarting or stopped.
    NotRunning,
    /// The server's process ended before it answered.
    ServerGone,
    /// The server does not offer this request.
    Unsupported(&'static str),
    /// The queue of messages to the server is full: it stopped reading.
    Congested,
    /// The document is not open on this server.
    NotOpen,
    /// The answer does not have the shape the protocol defines.
    Malformed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Server { code, message } => write!(f, "the language server answered {code}: {message}"),
            Error::Timeout => f.write_str("the language server did not answer in time"),
            Error::Cancelled => f.write_str("the request was cancelled"),
            Error::NotRunning => f.write_str("the language server is not running"),
            Error::ServerGone => f.write_str("the language server ended"),
            Error::Unsupported(method) => write!(f, "the language server does not offer {method}"),
            Error::Congested => f.write_str("the language server stopped reading its input"),
            Error::NotOpen => f.write_str("the document is not open on the language server"),
            Error::Malformed(why) => write!(f, "the language server's answer is malformed: {why}"),
        }
    }
}

impl std::error::Error for Error {}
