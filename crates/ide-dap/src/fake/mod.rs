//! A scripted debug adapter for tests and the window self-tests: a pretend program that walks through the lines of
//! one file on its main thread, beside a second thread that waits, and stops where the breakpoints say.
//!
//! It answers like a real adapter: capabilities with switches for restart and terminate, verified breakpoints on the
//! script's lines only, conditions (`false` never holds), hit counts (`3` or `>= 3`), log points (`{hits}` is filled
//! in), the exception filters `panic` (on by default) and `all`, two threads, two frames on the main thread, locals
//! with a nested `point`, and `evaluate` of the local names (`total` is another name for `hits`). In the app it runs on threads of its own; the test
//! binary `ide-dap-fake-adapter` runs it as a process, where it can also start a real program to own.

mod server;

use std::io::{self, Read, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::adapter::AdapterKind;
use crate::launch::LaunchConfig;
use crate::types::{Request, SourceBreakpoint};

pub use server::serve;

/// What the pretend program does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Script {
    /// The file it runs through.
    pub path: PathBuf,
    /// The lines it runs, in order; only these take breakpoints.
    pub lines: Vec<u32>,
    /// Whether the adapter offers `restart`.
    pub restart: bool,
    /// Whether the adapter offers `terminate`.
    pub terminate: bool,
    /// Whether the program panics after its last line, which stops it when the `panic` filter is on.
    pub panics: bool,
    /// What the program prints when it starts.
    pub output: Vec<String>,
    /// Whether launching asks the app to run the program in a terminal tab (`runInTerminal`).
    pub run_in_terminal: bool,
    /// Its exit code.
    pub exit_code: i32,
    /// How long the answer to `launch` or `attach` is held back, as when macOS asks for a password first.
    pub launch_delay_ms: u64,
}

impl Default for Script {
    fn default() -> Script {
        Script {
            path: std::env::temp_dir().join("ide-dap-fake").join("main.rs"),
            lines: (1..=10).collect(),
            restart: true,
            terminate: true,
            panics: false,
            output: vec!["fake program started\n".to_string()],
            run_in_terminal: false,
            exit_code: 0,
            launch_delay_ms: 0,
        }
    }
}

impl Script {
    /// A launch configuration for this script.
    pub fn launch_config(&self) -> LaunchConfig {
        let mut arguments = Map::new();
        arguments.insert("program".into(), json!(self.path));
        LaunchConfig {
            name: "fake".to_string(),
            kind: AdapterKind::Fake,
            request: Request::Launch,
            arguments,
            process_pick: false,
        }
    }

    /// A configuration that attaches to `pid`.
    pub fn attach_config(&self, pid: u32) -> LaunchConfig {
        LaunchConfig::attach(AdapterKind::Fake, pid, "fake (attached)")
    }

    /// A plain breakpoint on `line`.
    pub fn breakpoint(&self, line: u32) -> SourceBreakpoint {
        SourceBreakpoint {
            line,
            ..SourceBreakpoint::default()
        }
    }
}

/// Runs `script` on a thread of its own and returns the streams to talk to it: what it writes, where to write.
pub(crate) fn start(script: Script) -> io::Result<(impl Read + Send + 'static, impl Write + Send + 'static)> {
    let (from_adapter, adapter_out) = std::io::pipe()?;
    let (adapter_in, to_adapter) = std::io::pipe()?;
    std::thread::Builder::new()
        .name("dap-fake".into())
        .spawn(move || serve(script, adapter_in, adapter_out, false))?;
    Ok((from_adapter, to_adapter))
}

/// Whether `value` says the request asks for this program to be started for real; only the process form does it.
fn spawn_args(arguments: &Value) -> Option<Vec<String>> {
    let list = arguments.get("spawn")?.as_array()?;
    let args: Vec<String> = list.iter().filter_map(Value::as_str).map(str::to_string).collect();
    (!args.is_empty()).then_some(args)
}
