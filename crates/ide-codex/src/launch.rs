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
    /// `-c key=value` overrides of `config.toml`, the value in TOML.
    pub config: Vec<(String, String)>,
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
            config: Vec::new(),
        }
    }

    /// The arguments: the app-server on stdio, which is its default transport, and the overrides.
    pub fn args(&self) -> Vec<String> {
        let mut args = vec!["app-server".to_string()];
        for (key, value) in &self.config {
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
        if let Some(approval) = self.approval {
            params["approvalPolicy"] = json!(approval.wire());
        }
        if let Some(sandbox) = self.sandbox {
            params["sandbox"] = json!(sandbox.wire());
        }
        if let Some(effort) = &self.effort {
            params["config"] = json!({"model_reasoning_effort": effort});
        }
        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_go_where_each_request_takes_them() {
        let mut launch = Launch::new("/bin/codex", vec![("PATH".into(), "/bin".into())], "/work");
        launch.config.push(("model_verbosity".into(), "\"low\"".into()));
        assert_eq!(launch.args(), ["app-server", "-c", "model_verbosity=\"low\""]);
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
}
