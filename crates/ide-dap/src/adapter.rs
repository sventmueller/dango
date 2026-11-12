//! The debug adapters the app knows and how one is reached: a process on standard input and output, a process that
//! listens on a local port, or streams the caller provides, such as the fake adapter's.

use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::fake;

/// A debug adapter the app knows (E93).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdapterKind {
    /// `lldb-dap` from Apple's developer tools: Rust, C, C++, Swift, Objective-C.
    Lldb,
    /// Delve's `dlv dap`: Go.
    Go,
    /// vscode-js-debug: JavaScript and TypeScript under Node.
    JsDebug,
    /// debugpy: Python.
    Python,
    /// The scripted adapter of `fake`, for tests and the self-test.
    Fake,
}

impl AdapterKind {
    /// The adapter for a `type` of `.vscode/launch.json`, including the names other editors' adapters use for the
    /// same languages.
    pub fn from_type(kind: &str) -> Option<AdapterKind> {
        match kind {
            "lldb" | "lldb-dap" | "lldb-vscode" | "cppdbg" | "swift" | "rust" => Some(AdapterKind::Lldb),
            "go" | "dlv" | "delve" => Some(AdapterKind::Go),
            "node" | "pwa-node" | "node-terminal" | "js-debug" => Some(AdapterKind::JsDebug),
            "python" | "debugpy" => Some(AdapterKind::Python),
            "fake" => Some(AdapterKind::Fake),
            _ => None,
        }
    }

    /// The adapter for a document of the editor's language id, such as `rust` or `typescript`.
    pub fn for_language(language: &str) -> Option<AdapterKind> {
        match language {
            "rust" | "c" | "cpp" | "objective-c" | "objective-cpp" | "swift" => Some(AdapterKind::Lldb),
            "go" => Some(AdapterKind::Go),
            "javascript" | "typescript" | "javascriptreact" | "typescriptreact" => Some(AdapterKind::JsDebug),
            "python" => Some(AdapterKind::Python),
            _ => None,
        }
    }

    /// What the user reads, such as in the session list and the activity view.
    pub fn label(self) -> &'static str {
        match self {
            AdapterKind::Lldb => "lldb-dap",
            AdapterKind::Go => "Delve",
            AdapterKind::JsDebug => "js-debug",
            AdapterKind::Python => "debugpy",
            AdapterKind::Fake => "fake",
        }
    }

    /// The `adapterID` sent with `initialize`, and the `type` a launch configuration of this adapter gets.
    pub fn adapter_id(self) -> &'static str {
        match self {
            AdapterKind::Lldb => "lldb-dap",
            AdapterKind::Go => "go",
            AdapterKind::JsDebug => "pwa-node",
            AdapterKind::Python => "debugpy",
            AdapterKind::Fake => "fake",
        }
    }
}

/// How a session reaches its adapter.
pub(crate) enum Transport {
    /// A process the supervisor starts; it speaks on its standard streams, or on `port` of the loopback interface.
    Process {
        program: PathBuf,
        args: Vec<String>,
        env: Vec<(String, String)>,
        cwd: PathBuf,
        port: Option<u16>,
    },
    /// Streams the caller connected.
    Streams(Box<dyn Read + Send>, Box<dyn Write + Send>),
    /// The scripted adapter, on threads of its own.
    Fake(Box<fake::Script>),
}

/// A debug adapter ready to start: what `Adapters::find` returns, or one built for tests.
pub struct Adapter {
    /// Which adapter it is.
    pub kind: AdapterKind,
    /// What the activity view shows, such as `lldb-dap`.
    pub label: String,
    pub(crate) transport: Transport,
    pub(crate) record: Option<PathBuf>,
}

impl Adapter {
    /// An adapter process that speaks on its standard input and output, started with `env` in `cwd`.
    pub fn command(
        kind: AdapterKind,
        program: &Path,
        args: &[String],
        env: Vec<(String, String)>,
        cwd: &Path,
    ) -> Adapter {
        Adapter {
            kind,
            label: label(program),
            transport: Transport::Process {
                program: program.to_path_buf(),
                args: args.to_vec(),
                env,
                cwd: cwd.to_path_buf(),
                port: None,
            },
            record: None,
        }
    }

    /// An adapter process that listens on `port` of the loopback interface, as `dlv dap` and js-debug do; `args`
    /// already name the port.
    pub fn listening(
        kind: AdapterKind,
        program: &Path,
        args: &[String],
        env: Vec<(String, String)>,
        cwd: &Path,
        port: u16,
    ) -> Adapter {
        let mut adapter = Adapter::command(kind, program, args, env, cwd);
        if let Transport::Process { port: slot, .. } = &mut adapter.transport {
            *slot = Some(port);
        }
        adapter
    }

    /// An adapter at the other end of two streams, such as a recording played back.
    pub fn streams(
        kind: AdapterKind,
        label: &str,
        from_adapter: impl Read + Send + 'static,
        to_adapter: impl Write + Send + 'static,
    ) -> Adapter {
        Adapter {
            kind,
            label: label.to_string(),
            transport: Transport::Streams(Box::new(from_adapter), Box::new(to_adapter)),
            record: None,
        }
    }

    /// The scripted adapter, which runs on threads of the app and starts no process.
    pub fn fake(script: fake::Script) -> Adapter {
        Adapter {
            kind: AdapterKind::Fake,
            label: "fake".to_string(),
            transport: Transport::Fake(Box::new(script)),
            record: None,
        }
    }

    /// A recorded session played back, as `replay` describes; `recording` is the text `record_to` wrote.
    pub fn replay(kind: AdapterKind, recording: &str) -> std::io::Result<Adapter> {
        let (from_adapter, to_adapter) = crate::replay::start(crate::replay::parse(recording))?;
        Ok(Adapter::streams(kind, "replay", from_adapter, to_adapter))
    }

    /// Writes every message of the session to `path`, one JSON object a line, for a fixture.
    #[must_use]
    pub fn record_to(mut self, path: &Path) -> Adapter {
        self.record = Some(path.to_path_buf());
        self
    }

    /// The adapter's program, if it is a process.
    pub fn program(&self) -> Option<&Path> {
        match &self.transport {
            Transport::Process { program, .. } => Some(program),
            _ => None,
        }
    }

    /// The adapter's arguments, if it is a process.
    pub fn args(&self) -> &[String] {
        match &self.transport {
            Transport::Process { args, .. } => args,
            _ => &[],
        }
    }
}

impl fmt::Debug for Adapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Adapter")
            .field("kind", &self.kind)
            .field("label", &self.label)
            .field("program", &self.program())
            .field("args", &self.args())
            .finish_non_exhaustive()
    }
}

fn label(program: &Path) -> String {
    program
        .file_name()
        .map_or_else(|| program.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// A free port on the loopback interface for an adapter that listens; another program may take it first, which
/// the adapter then reports as a failure to start.
pub fn free_port() -> std::io::Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}
