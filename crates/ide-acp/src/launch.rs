//! How an agent is started: which program, with which arguments and environment, in which folder.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::install::Installs;
use crate::register::Preset;
use crate::sandbox::Sandbox;
use crate::tool;

/// Where the program of an agent was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The agent's own command on the login shell's PATH, installed by the user.
    OnPath,
    /// A version the app installed.
    Installed {
        /// The version.
        version: String,
        /// Whether it was installed as a combination verified with this build.
        verified: bool,
    },
    /// Given directly, as in tests.
    Given,
}

/// Everything needed to start an agent.
#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    /// The register's id, which threads name as their agent.
    pub agent: String,
    /// The name shown.
    pub name: String,
    /// The program.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<String>,
    /// Its whole environment: the login shell's and the agent's own variables.
    pub env: Vec<(String, String)>,
    /// The project folder the session works in.
    pub cwd: PathBuf,
    /// The `_meta` of `session/new`.
    pub session_meta: Option<Value>,
    /// The agent's id of a session to load instead of starting a new one.
    pub resume: Option<String>,
    /// Where the program came from.
    pub origin: Origin,
}

impl Launch {
    /// `program` with `args` for `agent`, in `cwd` with the login shell's environment `env`.
    pub fn new(
        agent: impl Into<String>,
        name: impl Into<String>,
        program: impl Into<PathBuf>,
        args: Vec<String>,
        env: Vec<(String, String)>,
        cwd: impl Into<PathBuf>,
    ) -> Launch {
        Launch {
            agent: agent.into(),
            name: name.into(),
            program: program.into(),
            args,
            env,
            cwd: cwd.into(),
            session_meta: None,
            resume: None,
            origin: Origin::Given,
        }
    }

    /// How `preset` starts: its own command on the login shell's PATH, else the version the app installed; nothing
    /// when it is neither.
    pub fn of_preset(preset: &Preset, installs: &Installs, env: &[(String, String)], cwd: &Path) -> Option<Launch> {
        if let Some((name, args)) = preset.on_path
            && let Some(program) = tool::which(name, tool::var(env, "PATH"))
        {
            let mut launch = Launch::new(
                preset.id,
                preset.name,
                program,
                args.iter().map(|a| (*a).to_string()).collect(),
                env.to_vec(),
                cwd,
            );
            launch.origin = Origin::OnPath;
            return Some(launch);
        }
        Launch::installed(preset.id, preset.name, installs, env, cwd)
    }

    /// How the app's installed version of `agent` starts; nothing when there is none.
    pub fn installed(
        agent: &str,
        name: &str,
        installs: &Installs,
        env: &[(String, String)],
        cwd: &Path,
    ) -> Option<Launch> {
        let (installed, program) = installs.current(agent)?;
        let mut all = env.to_vec();
        for (key, value) in &installed.env {
            all.retain(|(k, _)| k != key);
            all.push((key.clone(), value.clone()));
        }
        let mut launch = Launch::new(agent, name, program, installed.args.clone(), all, cwd);
        launch.origin = Origin::Installed {
            version: installed.version,
            verified: installed.verified,
        };
        Some(launch)
    }

    /// The same launch with `sandbox` switched on.
    pub fn sandboxed(mut self, sandbox: &Sandbox) -> Launch {
        self.args.extend(sandbox.args());
        for (key, value) in sandbox.env() {
            self.env.retain(|(k, _)| *k != key);
            self.env.push((key, value));
        }
        if let Some(meta) = sandbox.session_meta() {
            self.session_meta = Some(merge(self.session_meta.take(), meta));
        }
        self
    }

    /// The command that starts the agent with its pipes open.
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

/// `extra` laid over `base`, object by object.
fn merge(base: Option<Value>, extra: Value) -> Value {
    match (base, extra) {
        (Some(Value::Object(mut base)), Value::Object(extra)) => {
            for (key, value) in extra {
                let merged = merge(base.remove(&key), value);
                base.insert(key, merged);
            }
            Value::Object(base)
        }
        (_, extra) => extra,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_sandbox_adds_its_switch_where_the_agent_reads_it() {
        let launch = Launch::new(
            "a",
            "A",
            "/bin/a",
            vec!["acp".into()],
            vec![("PATH".into(), "/bin".into())],
            "/w",
        );
        let launch = launch
            .sandboxed(&Sandbox::Args(&["--sandbox"]))
            .sandboxed(&Sandbox::Env(&[("MODE", "workspace-write")]))
            .sandboxed(&Sandbox::SessionMeta(
                r#"{"claudeCode":{"options":{"sandbox":{"enabled":true}}}}"#,
            ));
        assert_eq!(launch.args, ["acp", "--sandbox"]);
        assert!(launch.env.contains(&("MODE".into(), "workspace-write".into())));
        assert_eq!(
            launch.session_meta.unwrap()["claudeCode"]["options"]["sandbox"]["enabled"],
            true
        );
    }

    #[test]
    fn meta_objects_merge_key_by_key() {
        let merged = merge(Some(json!({"a": {"x": 1}, "b": 2})), json!({"a": {"y": 3}}));
        assert_eq!(merged, json!({"a": {"x": 1, "y": 3}, "b": 2}));
    }
}
