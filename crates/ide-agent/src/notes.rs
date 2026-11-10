//! What the agent tells the user, as values the app words in the interface language (E41): the notices it puts into
//! the thread, what a tool call's card says where the model's text is not meant for the user, why a call asks, and
//! what a stored rule allows. Text a provider, the system or a library wrote comes along word for word as a detail.
//! The model reads its own English text of each, which the user never sees.

use serde::{Deserialize, Serialize};

use crate::chat::ProviderError;
use crate::policy::{Reason, Refusal, Rule};
use crate::workspace::Severity;

/// A notice the agent puts into the thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// The answer reached its length limit.
    LengthLimit,
    /// The model declined to answer; the provider's category, where it names one.
    Refused(Option<String>),
    /// The provider handed the answer from one model to another after the first declined it.
    Fallback {
        /// The model that declined.
        from: String,
        /// The model that continues.
        to: String,
    },
    /// A request to the provider failed for good.
    Failed(ProviderError),
    /// Compacting the conversation failed; the turn goes on with the conversation as it was.
    CompactionFailed(ProviderError),
    /// There was nothing to compact yet.
    NothingToCompact,
}

/// What a tool call's card says: why the call failed or did not run, or what it found where the card words it. It is
/// kept with the call's result, so a resumed thread words it again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolNote {
    /// The user cancelled the turn before the call ran.
    Cancelled,
    /// The model's input was not a JSON object.
    InvalidJson,
    /// The answer's length limit cut the call's input off, so nothing ran.
    CutOff,
    /// The input lacks this field.
    MissingField(String),
    /// There is no tool by this name.
    UnknownTool(String),
    /// The policy refused the call.
    Refused(Refusal),
    /// The user declined the call.
    Declined,
    /// The turn was cancelled while the call waited for the user.
    Withdrawn,
    /// A file or folder could not be read or written.
    Io {
        /// The path as the user reads it.
        path: String,
        /// The system's words.
        detail: String,
    },
    /// The path names a folder where a file is needed.
    Folder(String),
    /// The file is too large to read.
    TooLarge {
        /// The path as the user reads it.
        path: String,
        /// Its size.
        bytes: u64,
    },
    /// The file holds binary data.
    Binary(String),
    /// The edit has no text to replace.
    EmptyEdit,
    /// The edit replaces a text with itself.
    NoChange,
    /// The text to replace is not in the file.
    NotFound(String),
    /// The text to replace occurs more than once.
    Ambiguous {
        /// The path as the user reads it.
        path: String,
        /// How often it occurs.
        count: usize,
    },
    /// The address is not an http or https address.
    NotWeb(String),
    /// The address names no host.
    NoHost(String),
    /// The page is not text.
    NotText {
        /// The address.
        url: String,
        /// The type its server gives it.
        kind: String,
    },
    /// The page could not be fetched.
    Fetch(ProviderError),
    /// The search pattern or glob is not valid; the library's words.
    Pattern(String),
    /// The command did not start; the system's words.
    NotStarted(String),
    /// An item of the to-do list has a status the tool does not know.
    TodoStatus(String),
    /// The problems the language servers report.
    Diagnostics(Vec<Problem>),
    /// The language servers report no problems.
    NoProblems,
    /// No language server reports on the project.
    NoLanguageServer,
    /// The file is empty.
    EmptyFile(String),
    /// The file ends before the line the call asked for.
    PastEnd {
        /// The path as the user reads it.
        path: String,
        /// How many lines it has.
        lines: usize,
    },
}

/// A problem a language server reports, as the diagnostics card lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    /// The file as the user reads it.
    pub path: String,
    /// The line, counted from 1.
    pub line: u32,
    /// The column, counted from 1.
    pub column: u32,
    /// How serious it is.
    pub severity: Severity,
    /// What the server says, with its code and source.
    pub message: String,
}

impl ToolNote {
    /// What the model reads.
    pub(crate) fn for_model(&self) -> String {
        match self {
            ToolNote::Cancelled => "The user cancelled before this call ran.".into(),
            ToolNote::InvalidJson => "The input was not a valid JSON object.".into(),
            ToolNote::CutOff => "The answer reached its length limit before this call's input was complete; nothing \
                                 ran. Try again with a smaller input, such as several smaller edits."
                .into(),
            ToolNote::MissingField(name) => format!("the input lacks the field \"{name}\""),
            ToolNote::UnknownTool(name) => format!("there is no tool called {name}"),
            ToolNote::Refused(refusal) => format!("Refused: {}.", refusal.for_model()),
            ToolNote::Declined => "The user declined this call.".into(),
            ToolNote::Withdrawn => "The user cancelled.".into(),
            ToolNote::Io { path, detail } => format!("{path}: {detail}"),
            ToolNote::Folder(path) => format!("{path} is a folder; list it with glob"),
            ToolNote::TooLarge { path, bytes } => format!("{path} is too large to read ({bytes} bytes)"),
            ToolNote::Binary(path) => format!("{path} holds binary data"),
            ToolNote::EmptyEdit => "old_string is empty; use write_file to create a file".into(),
            ToolNote::NoChange => "old_string and new_string are the same".into(),
            ToolNote::NotFound(path) => {
                format!("old_string does not occur in {path}; read the file and copy the text exactly")
            }
            ToolNote::Ambiguous { path, count } => {
                format!("old_string occurs {count} times in {path}; include more surrounding text or set replace_all")
            }
            ToolNote::NotWeb(url) => format!("{url} is not an http or https address"),
            ToolNote::NoHost(url) => format!("{url} names no host"),
            ToolNote::NotText { url, kind } => format!("{url} is {kind}, not text"),
            ToolNote::Fetch(error) => error.to_string(),
            ToolNote::Pattern(detail) => format!("invalid pattern: {detail}"),
            ToolNote::NotStarted(detail) => format!("the command did not start: {detail}"),
            ToolNote::TodoStatus(status) => format!("unknown to-do status {status:?}"),
            ToolNote::Diagnostics(problems) => problems
                .iter()
                .map(|p| {
                    let severity = match p.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        Severity::Info => "info",
                        Severity::Hint => "hint",
                    };
                    format!("{}:{}:{}: {severity}: {}", p.path, p.line, p.column, p.message)
                })
                .collect::<Vec<_>>()
                .join("\n"),
            ToolNote::NoProblems => "No problems reported.".into(),
            ToolNote::NoLanguageServer => "No language server reports on this project.".into(),
            ToolNote::EmptyFile(path) => format!("{path} is empty."),
            ToolNote::PastEnd { path, lines } => format!("{path} has only {lines} lines."),
        }
    }
}

/// The words for what the agent tells the user; the app gives them in the interface language.
pub trait Wording: Send + Sync {
    /// A notice in the thread.
    fn note(&self, note: &Note) -> String;
    /// What a tool call's card says.
    fn tool(&self, note: &ToolNote) -> String;
    /// Why a call asks.
    fn reason(&self, reason: &Reason) -> String;
    /// What a rule allows.
    fn rule(&self, rule: &Rule) -> String;
    /// A failed request in a few words, for the notice that it is tried again.
    fn retry(&self, error: &ProviderError) -> String;
}

/// Wording that names each value by its Rust form, for tests and tools without an interface.
#[derive(Debug, Clone, Copy, Default)]
pub struct Plain;

impl Wording for Plain {
    fn note(&self, note: &Note) -> String {
        format!("{note:?}")
    }

    fn tool(&self, note: &ToolNote) -> String {
        format!("{note:?}")
    }

    fn reason(&self, reason: &Reason) -> String {
        format!("{reason:?}")
    }

    fn rule(&self, rule: &Rule) -> String {
        format!("{rule:?}")
    }

    fn retry(&self, error: &ProviderError) -> String {
        format!("{:?}", error.kind)
    }
}
