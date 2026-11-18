//! What a debug session reports and is asked with, as the editor uses it: lines and columns count from 1, columns in
//! UTF-16 code units, as the protocol does when the client says `linesStartAt1` and `columnsStartAt1`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Error;

/// Called once with the answer to a request, on a background thread.
pub type Reply<T> = Box<dyn FnOnce(Result<T, Error>) + Send>;

/// The number of a debug session; never reused while the app runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(pub(crate) u64);

impl SessionId {
    /// The number, for logs and keys.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// A thread of the debugged program, by the adapter's number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThreadId(pub i64);

/// A stack frame, by the adapter's number; valid only while the program stays stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FrameId(pub i64);

/// Where the children of a scope, variable or result are fetched from; 0 means there are none.
///
/// Valid only while the program stays stopped: after it continues the adapter may give the number to something else.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VariablesRef(pub i64);

impl VariablesRef {
    /// Whether there are children to fetch with `Session::variables`.
    pub(crate) fn has_children(self) -> bool {
        self.0 > 0
    }
}

/// Whether the session started the program or joined one that already ran (Review C7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Request {
    /// The adapter starts the program; it belongs to the session, and stopping the session ends it.
    Launch,
    /// The adapter attaches to a running program; it does not belong to the app, and stopping only detaches.
    Attach,
}

/// What an adapter can do, from its answer to `initialize` and later `capabilities` events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// It waits for `configurationDone` after the breakpoints are set.
    pub configuration_done: bool,
    /// Breakpoints with a condition.
    pub conditional_breakpoints: bool,
    /// Breakpoints that stop only at a certain hit count.
    pub hit_conditional_breakpoints: bool,
    /// Log points: breakpoints that print a message instead of stopping.
    pub log_points: bool,
    /// Breakpoints on a function by its name.
    pub function_breakpoints: bool,
    /// `evaluate` with the `hover` context, without side effects.
    pub evaluate_for_hovers: bool,
    /// `restart`; without it, restarting means stopping and starting anew.
    pub restart: bool,
    /// `terminate`, which asks the program to end gracefully.
    pub terminate: bool,
    /// `terminateDebuggee` in `disconnect`.
    pub terminate_debuggee: bool,
    /// Stepping backwards.
    pub step_back: bool,
    /// Changing a variable's value.
    pub set_variable: bool,
    /// Copying a value with the `clipboard` context.
    pub clipboard_context: bool,
    /// Stepping one thread while the others stay stopped.
    pub single_thread_execution: bool,
    /// The kinds of exception breakpoints it offers.
    pub exception_filters: Vec<ExceptionFilter>,
}

impl Capabilities {
    /// What `body`, the protocol's `Capabilities` object, says; unknown fields are ignored.
    pub(crate) fn from_json(body: &Value) -> Capabilities {
        let mut capabilities = Capabilities::default();
        capabilities.update(body);
        capabilities
    }

    /// Takes what `body` says over and leaves the rest, as a `capabilities` event asks.
    pub(crate) fn update(&mut self, body: &Value) {
        let flags: [(&str, &mut bool); 13] = [
            ("supportsConfigurationDoneRequest", &mut self.configuration_done),
            ("supportsConditionalBreakpoints", &mut self.conditional_breakpoints),
            (
                "supportsHitConditionalBreakpoints",
                &mut self.hit_conditional_breakpoints,
            ),
            ("supportsLogPoints", &mut self.log_points),
            ("supportsFunctionBreakpoints", &mut self.function_breakpoints),
            ("supportsEvaluateForHovers", &mut self.evaluate_for_hovers),
            ("supportsRestartRequest", &mut self.restart),
            ("supportsTerminateRequest", &mut self.terminate),
            ("supportTerminateDebuggee", &mut self.terminate_debuggee),
            ("supportsStepBack", &mut self.step_back),
            ("supportsSetVariable", &mut self.set_variable),
            ("supportsClipboardContext", &mut self.clipboard_context),
            (
                "supportsSingleThreadExecutionRequests",
                &mut self.single_thread_execution,
            ),
        ];
        for (key, flag) in flags {
            if let Some(value) = body.get(key).and_then(Value::as_bool) {
                *flag = value;
            }
        }
        if let Some(filters) = body.get("exceptionBreakpointFilters")
            && let Ok(filters) = Vec::<ExceptionFilter>::deserialize(filters)
        {
            self.exception_filters = filters;
        }
    }
}

/// A kind of exception breakpoint the adapter offers, such as Rust panics or uncaught JavaScript exceptions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionFilter {
    /// What `Session::set_exception_filters` takes.
    pub filter: String,
    /// What the user reads.
    pub label: String,
    /// A longer explanation, for a tooltip.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether it is on unless the user turns it off.
    #[serde(default)]
    pub default: bool,
    /// Whether it takes a condition.
    #[serde(default)]
    pub supports_condition: bool,
}

/// A breakpoint as the editor asks for it in one file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakpoint {
    /// Its line, from 1.
    pub line: u32,
    /// Its column, from 1, for a breakpoint inside a line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// An expression; the program stops only where it is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// How many hits to let pass, in the adapter's syntax, such as `5` or `>= 5`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    /// Makes it a log point: the message is printed, with expressions in braces, and the program goes on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
}

/// A breakpoint as the adapter set it, in the order the breakpoints were asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Breakpoint {
    /// The adapter's number, which `Stop::hit_breakpoints` and `Event::Breakpoint` refer to.
    pub id: Option<i64>,
    /// Whether it could be set; an unverified one is shown hollow and greyed.
    pub verified: bool,
    /// Why it is unverified, or what else the adapter says about it.
    pub(crate) message: Option<String>,
    /// The file it is in.
    pub(crate) path: Option<PathBuf>,
    /// The line it really sits on, from 1, which may differ from the one asked for.
    pub(crate) line: Option<u32>,
    /// Its column, from 1.
    pub(crate) column: Option<u32>,
}

/// A thread of the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    /// The adapter's number.
    pub id: ThreadId,
    /// What the user reads.
    pub name: String,
}

/// A source file, or code the adapter holds that has no file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    /// What the user reads.
    pub(crate) name: Option<String>,
    /// The file, if it is one.
    pub path: Option<PathBuf>,
    /// The adapter's number for code without a file, which only the adapter can show.
    pub(crate) reference: Option<i64>,
    /// `emphasize` or `deemphasize`, such as for library code.
    pub(crate) hint: Option<String>,
}

/// A frame of a thread's call stack, the innermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackFrame {
    /// The adapter's number.
    pub id: FrameId,
    /// The function, as the user reads it.
    pub name: String,
    /// Where it is, if there is a source.
    pub source: Option<Source>,
    /// Its line, from 1; 0 without a source.
    pub line: u32,
    /// Its column, from 1; 0 without a source.
    pub column: u32,
    /// `normal`, `label` for a separator row, or `subtle` for a frame worth less attention.
    pub(crate) hint: Option<String>,
}

/// A group of variables in a frame, such as locals or registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// What the user reads.
    pub name: String,
    /// Where its variables are fetched from.
    pub variables: VariablesRef,
    /// Whether fetching them is slow, so the inspector leaves it closed.
    pub expensive: bool,
    /// `arguments`, `locals` or `registers`, if the adapter says.
    pub(crate) hint: Option<String>,
}

/// A variable, a field or an element.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Variable {
    pub name: String,
    /// Its value as the adapter writes it.
    pub value: String,
    /// Its type, if the adapter says.
    pub type_name: Option<String>,
    /// Its children, fetched only when the user opens it.
    pub children: VariablesRef,
    /// An expression for it that `evaluate` understands, such as for a watch.
    pub(crate) evaluate_name: Option<String>,
}

/// The value of an expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    /// The value as the adapter writes it.
    pub result: String,
    /// Its type, if the adapter says.
    pub type_name: Option<String>,
    /// Its children, fetched only when the user opens it.
    pub children: VariablesRef,
}

/// Where an expression is evaluated, which decides what the adapter allows and how it formats the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvaluateContext {
    /// A watched expression in the inspector.
    Watch,
    /// A line typed into the debug console.
    Repl,
    /// The value under the pointer in the editor; without side effects.
    Hover,
    /// A value copied to the clipboard.
    Clipboard,
}

impl EvaluateContext {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            EvaluateContext::Watch => "watch",
            EvaluateContext::Repl => "repl",
            EvaluateContext::Hover => "hover",
            EvaluateContext::Clipboard => "clipboard",
        }
    }
}

/// Why the program stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// A step ended.
    Step,
    /// A breakpoint was hit.
    Breakpoint,
    /// An exception or panic, as an exception filter asked.
    Exception,
    /// The user paused it.
    Pause,
    /// It stopped at its entry, as the launch configuration asked.
    Entry,
    /// A function breakpoint was hit.
    FunctionBreakpoint,
    /// A data breakpoint was hit.
    DataBreakpoint,
    /// Something else the adapter names.
    Other(String),
}

impl StopReason {
    pub(crate) fn from_wire(reason: &str) -> StopReason {
        match reason {
            "step" | "goto" => StopReason::Step,
            "breakpoint" => StopReason::Breakpoint,
            "exception" => StopReason::Exception,
            "pause" => StopReason::Pause,
            "entry" => StopReason::Entry,
            "function breakpoint" => StopReason::FunctionBreakpoint,
            "data breakpoint" => StopReason::DataBreakpoint,
            other => StopReason::Other(other.to_string()),
        }
    }
}

/// The program stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    /// Why.
    pub reason: StopReason,
    /// The reason in the adapter's words, for the user.
    pub(crate) description: Option<String>,
    /// More, such as the exception's message.
    pub text: Option<String>,
    /// The thread that stopped, if the adapter says.
    pub thread: Option<ThreadId>,
    /// Whether every thread stopped, not only `thread`.
    pub(crate) all_threads: bool,
    /// The breakpoints that were hit, by `Breakpoint::id`.
    pub hit_breakpoints: Vec<i64>,
}

/// What kind of output a line of the debug console is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputCategory {
    /// The adapter's own messages.
    Console,
    /// A message the user should notice.
    Important,
    /// The program's standard output.
    Stdout,
    /// The program's standard error.
    Stderr,
    /// Something else the adapter names.
    Other(String),
}

/// Output for the debug console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Its kind.
    pub category: OutputCategory,
    /// The text, with its line breaks.
    pub text: String,
    /// Where it was printed from, such as a log point, if the adapter says.
    pub(crate) source: Option<Source>,
    /// The line there, from 1.
    pub line: Option<u32>,
}

/// How the program being debugged came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartMethod {
    /// The adapter started it.
    Launch,
    /// The adapter attached to it.
    Attach,
    /// The adapter attached to a program it had started suspended, such as in a terminal tab.
    AttachForSuspendedLaunch,
}

/// The program being debugged, from the adapter's `process` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// Its name, usually the path of its executable.
    pub(crate) name: String,
    /// Its process number, if the adapter says.
    pub pid: Option<u32>,
    /// How it came to be, if the adapter says.
    pub(crate) start: Option<StartMethod>,
}

/// Why a session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndReason {
    /// The program ended or the adapter finished the session.
    Finished,
    /// The user stopped the session: a launched program was ended, an attached one detached.
    Stopped,
    /// The adapter could not start, launch or attach; the message says why, in the adapter's words.
    Failed(String),
    /// The adapter process ended on its own; a launched program was ended with its group.
    AdapterCrashed {
        /// How the adapter ended, if known.
        ending: Option<ide_process::Ending>,
        /// The end of what it wrote to standard error.
        stderr: String,
    },
}

/// The end of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct End {
    /// Why it ended.
    pub reason: EndReason,
    /// The program's exit code, if it ended and the adapter said.
    pub exit_code: Option<i64>,
}

/// Where a session is in its life.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// The adapter is starting, launching or attaching.
    Starting,
    /// The program runs.
    Running,
    /// The program is stopped.
    Stopped(Stop),
    /// Stopping was asked for and is under way.
    Stopping,
    /// It is over; nothing more can be asked.
    Ended(End),
}

/// Where the adapter wants the program run in a terminal (`runInTerminal`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRequest {
    /// A title for the tab.
    pub(crate) title: Option<String>,
    /// The folder to run it in.
    pub cwd: PathBuf,
    /// The program and its arguments.
    pub args: Vec<String>,
    /// Variables to set, or with `None` to remove, on top of the login shell's environment.
    pub env: Vec<(String, Option<String>)>,
    /// Whether the adapter asked for an external terminal; the app opens a tab either way.
    pub(crate) external: bool,
}
