//! How Codex is started and which settings its thread opens with.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

/// When Codex asks before it runs a command or changes a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApprovalPolicy {
    /// Asks before anything that is not known to be safe.
    Untrusted,
    /// Codex decides when to ask, such as when a command needs to leave the sandbox.
    OnRequest,
    /// Never asks; what the sandbox refuses fails.
    Never,
}

impl ApprovalPolicy {
    /// The protocol's name.
    pub fn wire(self) -> &'static str {
        match self {
            ApprovalPolicy::Untrusted => "untrusted",
            ApprovalPolicy::OnRequest => "on-request",
            ApprovalPolicy::Never => "never",
        }
    }

    /// The policy the protocol names `name`; the granular object form has no counterpart here.
    pub fn from_wire(name: &str) -> Option<ApprovalPolicy> {
        [
            ApprovalPolicy::Untrusted,
            ApprovalPolicy::OnRequest,
            ApprovalPolicy::Never,
        ]
        .into_iter()
        .find(|p| p.wire() == name)
    }
}

/// What the commands Codex runs may touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SandboxMode {
    /// Read anything, write nothing.
    ReadOnly,
    /// Write within the project folder.
    WorkspaceWrite,
    /// No sandbox.
    DangerFullAccess,
}

impl SandboxMode {
    /// The protocol's name in `thread/start` and in `config.toml`.
    pub fn wire(self) -> &'static str {
        match self {
            SandboxMode::ReadOnly => "read-only",
            SandboxMode::WorkspaceWrite => "workspace-write",
            SandboxMode::DangerFullAccess => "danger-full-access",
        }
    }

    /// The mode the protocol names `name`.
    pub fn from_wire(name: &str) -> Option<SandboxMode> {
        [
            SandboxMode::ReadOnly,
            SandboxMode::WorkspaceWrite,
            SandboxMode::DangerFullAccess,
        ]
        .into_iter()
        .find(|m| m.wire() == name)
    }

    /// The mode a `SandboxPolicy` object of a response stands for, such as `{"type": "workspaceWrite"}`.
    pub fn from_policy(policy: &Value) -> Option<SandboxMode> {
        match policy.get("type").and_then(Value::as_str)? {
            "readOnly" => Some(SandboxMode::ReadOnly),
            "workspaceWrite" => Some(SandboxMode::WorkspaceWrite),
            "dangerFullAccess" => Some(SandboxMode::DangerFullAccess),
            _ => None,
        }
    }

    /// The `sandboxPolicy` object `turn/start` takes, which unlike `thread/start` does not accept the mode's name.
    pub fn policy(self) -> Value {
        match self {
            SandboxMode::ReadOnly => json!({"type": "readOnly"}),
            SandboxMode::WorkspaceWrite => json!({"type": "workspaceWrite"}),
            SandboxMode::DangerFullAccess => json!({"type": "dangerFullAccess"}),
        }
    }
}

/// The features of Codex 0.167 (`codex features list`) that bring tools with sign-ins of their own or reach past the
/// sandbox's network block, or subagents that show only as tool cards (E189, E191); off in an order session.
const ORDER_FEATURES_OFF: [&str; 9] = [
    "apps",
    "plugins",
    "remote_plugin",
    "browser_use",
    "browser_use_external",
    "computer_use",
    "in_app_browser",
    "image_generation",
    "multi_agent",
];

/// The bounds of a work order's session (docs/phase-13.md, E173, E193): a server of its own, never the shared one;
/// no approvals, so what the sandbox refuses fails; writes only in the project folder and the write roots; no
/// network; web search, apps, plugins, MCP servers and subagents off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confinement {
    /// Whether commands and edits may write: in the project folder and `write_roots` (`workspace-write`); a reader
    /// gets `read-only`.
    pub writes: bool,
    /// Folders writable besides the project folder, such as the build folder.
    pub write_roots: Vec<PathBuf>,
    /// The project's `AGENTS.md` as frozen from the user's working copy (E172), given as the thread's developer
    /// instructions; the worktree's own copies are not read.
    pub instructions: Option<String>,
    /// Folders marked untrusted for this server, so no `.codex/` configuration or hooks load from them: the worktree,
    /// and the working copy whose trust a linked worktree would share (C57).
    pub untrusted: Vec<PathBuf>,
    /// The MCP servers of the user's configuration, each switched off by name: an empty `mcp_servers` table leaves
    /// them running, since Codex merges overrides into its configuration.
    pub mcp_servers: Vec<String>,
}

impl Confinement {
    /// The `sandboxPolicy` every `turn/start` of the session carries, which unlike `thread/start` names write roots
    /// and the network.
    pub fn sandbox_policy(&self) -> Value {
        if self.writes {
            let roots: Vec<String> = self
                .write_roots
                .iter()
                .map(|root| root.to_string_lossy().into_owned())
                .collect();
            json!({
                "type": "workspaceWrite",
                "writableRoots": roots,
                "networkAccess": false,
                "excludeTmpdirEnvVar": true,
                "excludeSlashTmp": true
            })
        } else {
            json!({"type": "readOnly", "networkAccess": false})
        }
    }

    fn mode(&self) -> SandboxMode {
        if self.writes {
            SandboxMode::WorkspaceWrite
        } else {
            SandboxMode::ReadOnly
        }
    }

    /// The `-c` overrides: the features off, web search off, no MCP server, no `AGENTS.md` read from the project, and
    /// the untrusted folders, each a key of an inline table since a dotted `-c` path is split at every dot.
    fn overrides(&self) -> Vec<(String, String)> {
        let mut overrides: Vec<(String, String)> = ORDER_FEATURES_OFF
            .iter()
            .map(|feature| (format!("features.{feature}"), "false".to_string()))
            .collect();
        overrides.push(("web_search".into(), "\"disabled\"".into()));
        overrides.push(("mcp_servers".into(), "{}".into()));
        for name in &self.mcp_servers {
            overrides.push((format!("mcp_servers.{name}.enabled"), "false".into()));
        }
        overrides.push(("project_doc_max_bytes".into(), "0".into()));
        if !self.untrusted.is_empty() {
            let projects: Vec<String> = self
                .untrusted
                .iter()
                .map(|folder| {
                    format!(
                        "{} = {{ trust_level = \"untrusted\" }}",
                        toml_string(&folder.to_string_lossy())
                    )
                })
                .collect();
            overrides.push(("projects".into(), format!("{{ {} }}", projects.join(", "))));
        }
        overrides
    }

    /// Puts the bounds into the parameters of a `turn/start`, over any mode the session switched to.
    pub(crate) fn bound_turn(&self, params: &mut Value) {
        params["approvalPolicy"] = json!(ApprovalPolicy::Never.wire());
        params["sandboxPolicy"] = self.sandbox_policy();
    }
}

/// Everything needed to start Codex and open its thread.
#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    /// The name shown in the activity view.
    pub name: String,
    /// The `codex` executable.
    pub program: PathBuf,
    /// Its whole environment: the login shell's (E10, E23).
    pub env: Vec<(String, String)>,
    /// The project folder the thread works in.
    pub cwd: PathBuf,
    /// The model to start with; Codex's configured one when `None`.
    pub model: Option<String>,
    /// The reasoning effort to start with, such as `high`.
    pub effort: Option<String>,
    /// When Codex asks.
    pub approval: Option<ApprovalPolicy>,
    /// What its commands may touch.
    pub sandbox: Option<SandboxMode>,
    /// Codex's id of a thread to resume instead of starting a new one.
    pub resume: Option<String>,
    /// The release `version::probe` already read for this program; the handshake reads it itself when `None`.
    pub version: Option<crate::version::Version>,
    /// `-c key=value` overrides of `config.toml`, the value in TOML; a server of its own takes them, the shared one
    /// keeps its own configuration.
    pub config: Vec<(String, String)>,
    /// Whether the session joins Codex's shared app-server, the daemon Remote Control reaches (E148), through
    /// `codex app-server proxy` instead of starting a server of its own.
    pub shared: bool,
    /// The bounds of a work order's session; with them the session never joins the shared server (E193), the
    /// approval policy and the sandbox give way to them, and their overrides come after `config`.
    pub confinement: Option<Confinement>,
}

impl Launch {
    /// `program` in `cwd` with the login shell's environment `env`, with Codex's own settings.
    pub fn new(program: impl Into<PathBuf>, env: Vec<(String, String)>, cwd: impl Into<PathBuf>) -> Launch {
        Launch {
            name: "Codex".into(),
            program: program.into(),
            env,
            cwd: cwd.into(),
            model: None,
            effort: None,
            approval: None,
            sandbox: None,
            resume: None,
            version: None,
            config: Vec::new(),
            shared: false,
            confinement: None,
        }
    }

    /// Whether the session goes through `codex app-server proxy` to the shared server; never with bounds.
    pub fn joins_shared(&self) -> bool {
        self.shared && self.confinement.is_none()
    }

    /// The arguments: the app-server on stdio, which is its default transport, and the overrides; or the proxy that
    /// connects stdio to the shared server.
    pub fn args(&self) -> Vec<String> {
        if self.joins_shared() {
            return vec!["app-server".to_string(), "proxy".to_string()];
        }
        let mut args = vec!["app-server".to_string()];
        let bounds = self
            .confinement
            .as_ref()
            .map(Confinement::overrides)
            .unwrap_or_default();
        for (key, value) in self.config.iter().chain(&bounds) {
            args.push("-c".into());
            args.push(format!("{key}={value}"));
        }
        args
    }

    /// The command that starts the app-server with its pipes open.
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(self.args())
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// The parameters of `thread/start`, or of `thread/resume` for the thread to resume.
    pub(crate) fn open_params(&self) -> Value {
        let mut params = json!({"cwd": self.cwd.to_string_lossy()});
        if let Some(id) = &self.resume {
            params["threadId"] = json!(id);
        }
        if let Some(model) = &self.model {
            params["model"] = json!(model);
        }
        let (approval, sandbox) = match &self.confinement {
            Some(bounds) => (Some(ApprovalPolicy::Never), Some(bounds.mode())),
            None => (self.approval, self.sandbox),
        };
        if let Some(approval) = approval {
            params["approvalPolicy"] = json!(approval.wire());
        }
        if let Some(sandbox) = sandbox {
            params["sandbox"] = json!(sandbox.wire());
        }
        if let Some(effort) = &self.effort {
            params["config"] = json!({"model_reasoning_effort": effort});
        }
        if let Some(instructions) = self.confinement.as_ref().and_then(|c| c.instructions.as_ref()) {
            params["developerInstructions"] = json!(instructions);
        }
        params
    }
}

/// `text` as a TOML basic string.
fn toml_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_go_where_each_request_takes_them() {
        let mut launch = Launch::new("/bin/codex", vec![("PATH".into(), "/bin".into())], "/work");
        launch.config.push(("model_verbosity".into(), "\"low\"".into()));
        assert_eq!(launch.args(), ["app-server", "-c", "model_verbosity=\"low\""]);
        launch.shared = true;
        assert_eq!(launch.args(), ["app-server", "proxy"]);
        launch.shared = false;
        assert_eq!(launch.open_params(), json!({"cwd": "/work"}));
        launch.model = Some("gpt-5.5".into());
        launch.effort = Some("high".into());
        launch.approval = Some(ApprovalPolicy::OnRequest);
        launch.sandbox = Some(SandboxMode::WorkspaceWrite);
        launch.resume = Some("thr_1".into());
        assert_eq!(
            launch.open_params(),
            json!({"cwd": "/work", "threadId": "thr_1", "model": "gpt-5.5", "approvalPolicy": "on-request",
                   "sandbox": "workspace-write", "config": {"model_reasoning_effort": "high"}})
        );
        assert_eq!(SandboxMode::WorkspaceWrite.policy(), json!({"type": "workspaceWrite"}));
        assert_eq!(
            SandboxMode::from_policy(&json!({"type": "readOnly", "networkAccess": false})),
            Some(SandboxMode::ReadOnly)
        );
        assert_eq!(ApprovalPolicy::from_wire("on-request"), Some(ApprovalPolicy::OnRequest));
        assert_eq!(ApprovalPolicy::from_wire("on-failure"), None);
    }

    #[test]
    fn an_order_session_has_a_server_of_its_own_with_no_approvals_tools_or_network() {
        let mut launch = Launch::new("/bin/codex", vec![("PATH".into(), "/bin".into())], "/work");
        launch.shared = true;
        launch.approval = Some(ApprovalPolicy::OnRequest);
        launch.sandbox = Some(SandboxMode::DangerFullAccess);
        launch.confinement = Some(Confinement {
            writes: true,
            write_roots: vec!["/builds/o1".into()],
            instructions: None,
            untrusted: Vec::new(),
            mcp_servers: Vec::new(),
        });
        assert!(!launch.joins_shared());
        let args = launch.args();
        assert_eq!(args[0], "app-server");
        assert!(!args.contains(&"proxy".to_string()));
        for wanted in [
            "features.apps=false",
            "features.plugins=false",
            "features.multi_agent=false",
            "features.browser_use=false",
            "web_search=\"disabled\"",
            "mcp_servers={}",
        ] {
            assert!(args.iter().any(|a| a == wanted), "{wanted} is missing from {args:?}");
        }
        let params = launch.open_params();
        assert_eq!(params["approvalPolicy"], "never");
        assert_eq!(params["sandbox"], "workspace-write");
        let mut turn = json!({"threadId": "thr_1"});
        launch.confinement.as_ref().unwrap().bound_turn(&mut turn);
        assert_eq!(
            turn,
            json!({"threadId": "thr_1", "approvalPolicy": "never", "sandboxPolicy": {"type": "workspaceWrite",
                   "writableRoots": ["/builds/o1"], "networkAccess": false, "excludeTmpdirEnvVar": true,
                   "excludeSlashTmp": true}})
        );
    }

    #[test]
    fn a_reader_of_an_order_reads_only() {
        let mut launch = Launch::new("/bin/codex", Vec::new(), "/work");
        let reader = Confinement {
            writes: false,
            write_roots: vec!["/builds/o1".into()],
            instructions: None,
            untrusted: Vec::new(),
            mcp_servers: Vec::new(),
        };
        assert_eq!(
            reader.sandbox_policy(),
            json!({"type": "readOnly", "networkAccess": false})
        );
        launch.confinement = Some(reader);
        assert_eq!(launch.open_params()["sandbox"], "read-only");
    }

    #[test]
    fn an_order_reads_its_frozen_agents_md_and_no_project_configuration() {
        let mut launch = Launch::new("/bin/codex", Vec::new(), "/data/worktrees/o1");
        launch.confinement = Some(Confinement {
            writes: true,
            write_roots: Vec::new(),
            instructions: Some("Run make check.".into()),
            untrusted: vec!["/data/worktrees/o1".into(), "/work/sh\"op.v2".into()],
            mcp_servers: Vec::new(),
        });
        let args = launch.args();
        assert!(args.iter().any(|a| a == "project_doc_max_bytes=0"), "{args:?}");
        let projects = args
            .iter()
            .find(|a| a.starts_with("projects="))
            .expect("the untrusted folders");
        assert_eq!(
            projects,
            r#"projects={ "/data/worktrees/o1" = { trust_level = "untrusted" }, "/work/sh\"op.v2" = { trust_level = "untrusted" } }"#
        );
        assert_eq!(launch.open_params()["developerInstructions"], "Run make check.");
        launch.confinement = None;
        assert!(launch.open_params().get("developerInstructions").is_none());
        assert!(!launch.args().iter().any(|a| a.starts_with("project_doc")));
    }
}
