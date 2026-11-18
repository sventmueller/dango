//! The agent's tool calls, what they show, and its to-do list.

use std::time::{Duration, SystemTime};

use serde_json::Value;

/// The sort of work a tool does, as ACP names it, for the card's icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// Reads files.
    Read,
    /// Changes files.
    Edit,
    /// Deletes files.
    Delete,
    /// Moves or renames files.
    Move,
    /// Searches files or tools.
    Search,
    /// Runs a command.
    Execute,
    /// Plans or delegates.
    Think,
    /// Reads from the web.
    Fetch,
    /// Changes the agent's mode.
    SwitchMode,
    /// A tool of an MCP server.
    Mcp,
    /// Anything else.
    Other,
}

/// Where a tool call stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    /// Its input is still arriving or it waits for permission.
    Pending,
    /// It runs.
    InProgress,
    /// It finished.
    Completed,
    /// It failed or was refused.
    Failed,
}

/// A tool the agent calls.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// The agent's id for the call.
    pub call_id: String,
    /// The tool's name as the agent knows it.
    pub name: String,
    /// The sort of work it does.
    pub kind: ToolKind,
    /// What the card's head reads.
    pub title: String,
    /// Where it stands.
    pub status: ToolStatus,
    /// The input exactly as the agent sent it.
    pub input: Value,
    /// The input as far as the card shows it field by field.
    pub view: ToolView,
    /// What it returned.
    pub output: Option<ToolOutput>,
    /// When it began to run.
    pub started: Option<SystemTime>,
    /// When it finished.
    pub ended: Option<SystemTime>,
    /// How long it ran, as the agent last reported or as measured at its end.
    pub elapsed: Option<Duration>,
    /// The tool call that started the subagent this call belongs to.
    pub parent: Option<String>,
    /// A one-line summary of the group of calls this one belongs to.
    pub(crate) summary: Option<String>,
    /// The background task it became, when it runs in the background.
    pub background_task: Option<String>,
}

impl ToolCall {
    /// A call that has just been announced.
    pub fn new(call_id: impl Into<String>, name: impl Into<String>, kind: ToolKind) -> ToolCall {
        let name = name.into();
        ToolCall {
            call_id: call_id.into(),
            title: name.clone(),
            name,
            kind,
            status: ToolStatus::Pending,
            input: Value::Null,
            view: ToolView::Other,
            output: None,
            started: None,
            ended: None,
            elapsed: None,
            parent: None,
            summary: None,
            background_task: None,
        }
    }
}

/// The input of a tool call field by field, for the tools whose cards show more than the raw input.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolView {
    /// A shell command.
    Command {
        /// The command line.
        command: String,
        /// What the agent says it does.
        description: Option<String>,
        /// Whether it runs in the background.
        background: bool,
        /// How long it may run.
        timeout: Option<Duration>,
    },
    /// Reading a file.
    Read {
        /// The file.
        path: String,
        /// The first line read, counted from 1.
        offset: Option<u64>,
        /// How many lines are read.
        limit: Option<u64>,
    },
    /// Replacing text in a file.
    Edit {
        /// The file.
        path: String,
        /// The replacements, in order.
        edits: Vec<TextEdit>,
    },
    /// Writing a whole file.
    Write {
        /// The file.
        path: String,
        /// Its new content.
        content: String,
    },
    /// Searching files by content or name.
    Search {
        /// The pattern.
        pattern: String,
        /// Where it searches.
        path: Option<String>,
        /// Which files it looks at.
        glob: Option<String>,
        /// How many results it found, once it has.
        results: Option<u64>,
    },
    /// Reading a web page.
    Fetch {
        /// The page.
        url: String,
        /// What the agent wants to know from it.
        prompt: Option<String>,
    },
    /// Searching the web.
    WebSearch {
        /// The query.
        query: String,
    },
    /// A tool of an MCP server.
    Mcp {
        /// The server.
        server: String,
        /// The tool.
        tool: String,
    },
    /// Starting a subagent.
    Agent {
        /// What it is to do, in a few words.
        description: String,
        /// Which sort of agent it is.
        agent_type: Option<String>,
        /// Its instructions.
        prompt: String,
        /// Whether it runs in the background.
        background: bool,
    },
    /// No view beyond the raw input.
    Other,
}

impl ToolView {
    /// The first and last line a read covers, counted from 1, when it does not read the whole file.
    pub fn line_range(&self) -> Option<(u64, Option<u64>)> {
        match self {
            ToolView::Read { offset, limit, .. } if offset.is_some() || limit.is_some() => {
                let first = offset.unwrap_or(1).max(1);
                Some((first, limit.map(|n| first + n.saturating_sub(1))))
            }
            _ => None,
        }
    }
}

/// One replacement in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// The text replaced.
    pub old: String,
    /// The text put in its place.
    pub new: String,
    /// Whether every occurrence is replaced.
    pub replace_all: bool,
}

/// What a tool returned.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolOutput {
    /// Text.
    Text(String),
    /// What a command wrote.
    Terminal {
        /// Its standard output.
        stdout: String,
        /// Its standard error.
        stderr: String,
        /// Whether it was stopped before it finished.
        interrupted: bool,
    },
    /// The change made to a file.
    Diff {
        /// The file.
        path: String,
        /// The changed regions.
        hunks: Vec<Hunk>,
    },
}

/// One changed region of a file in unified-diff form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// The first line before the change, counted from 1.
    pub old_start: u64,
    /// How many lines it covered before.
    pub old_lines: u64,
    /// The first line after the change, counted from 1.
    pub new_start: u64,
    /// How many lines it covers after.
    pub new_lines: u64,
    /// The lines, each starting with ` `, `-` or `+`.
    pub lines: Vec<String>,
}

/// One item of the agent's to-do list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Todo {
    /// What is to be done.
    pub content: String,
    /// The same in the present continuous, shown while it is in progress.
    pub active_form: Option<String>,
    /// Where it stands.
    pub status: TodoStatus,
}

/// Where a to-do stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    /// Not begun.
    Pending,
    /// Being worked on.
    InProgress,
    /// Done.
    Completed,
}
