//! Why a control request got no usable answer.

/// Why a control request got no usable answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    /// The CLI refused it, such as for a subtype it does not know.
    Refused(String),
    /// No answer came in time; the request was withdrawn.
    Timeout,
    /// The CLI ended before it answered.
    Closed,
    /// The answer did not have the expected shape.
    Unexpected(String),
    /// No thread could wait for the answer.
    Unwaited(String),
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlError::Refused(why) => write!(f, "refused: {why}"),
            ControlError::Timeout => f.write_str("no answer in time"),
            ControlError::Closed => f.write_str("the CLI ended"),
            ControlError::Unexpected(why) => write!(f, "unexpected answer: {why}"),
            ControlError::Unwaited(why) => write!(f, "the answer could not be waited for: {why}"),
        }
    }
}

impl std::error::Error for ControlError {}
