//! How a server is started, how patient the client is with it, and the settings it asks for.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::Inner;
use crate::encoding::Encoding;

/// How to start a language server and how patient to be with it.
#[derive(Clone)]
pub struct ServerConfig {
    /// What the activity view shows, such as `gopls`.
    pub label: String,
    /// The server's executable.
    pub(crate) program: PathBuf,
    /// Its arguments, such as `--stdio`.
    pub args: Vec<String>,
    /// The whole environment, normally the login shell's; the app's own is not passed on.
    pub(crate) env: Vec<(String, String)>,
    /// The project root: the working directory and the one workspace folder.
    pub(crate) root: PathBuf,
    /// Sent with `initialize`.
    pub initialization_options: Option<Value>,
    /// The answer to `workspace/configuration`, looked up by section, such as `{"gopls": {...}}`.
    pub settings: Value,
    /// The position encodings offered, most wanted first.
    pub position_encodings: Vec<Encoding>,
    /// Whether the app watches files for the server; then servers register their watchers instead of watching
    /// themselves, and the app must report changes with `Server::files_changed`.
    pub client_watches_files: bool,
    /// When to restart after a crash.
    pub restart: RestartPolicy,
    /// How long a request may take.
    pub request_timeout: Duration,
    /// How long `initialize` may take.
    pub(crate) start_timeout: Duration,
    /// How long each step of stopping may take: `shutdown`, then SIGTERM, then SIGKILL.
    pub stop_grace: Duration,
}

impl ServerConfig {
    /// A configuration with the usual limits.
    pub fn new(label: &str, program: &Path, root: &Path, env: Vec<(String, String)>) -> ServerConfig {
        ServerConfig {
            label: label.to_string(),
            program: program.to_path_buf(),
            args: vec![],
            env,
            root: root.to_path_buf(),
            initialization_options: None,
            settings: Value::Null,
            // rust-analyzer and typescript-go pick UTF-8 whenever it is offered, whatever the order, which would
            // recount every position; UTF-16 alone is what every server must accept.
            position_encodings: vec![Encoding::Utf16],
            client_watches_files: false,
            restart: RestartPolicy::default(),
            request_timeout: Duration::from_secs(30),
            start_timeout: Duration::from_secs(60),
            stop_grace: Duration::from_secs(2),
        }
    }
}

impl std::fmt::Debug for ServerConfig {
    /// Leaves out the environment's values: the login shell's environment often holds tokens.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("label", &self.label)
            .field("program", &self.program)
            .field("args", &self.args)
            .field("env", &format_args!("{} variables", self.env.len()))
            .field("root", &self.root)
            .field("position_encodings", &self.position_encodings)
            .field("restart", &self.restart)
            .finish_non_exhaustive()
    }
}

/// When a crashed server is started again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPolicy {
    /// Crashes in a row after which the server stays down.
    pub max_restarts: u32,
    /// The wait before the first restart; it doubles with every further crash in a row.
    pub first_delay: Duration,
    /// The longest wait.
    pub max_delay: Duration,
    /// A server that ran this long before it crashed starts the count anew.
    pub stable_after: Duration,
}

impl Default for RestartPolicy {
    fn default() -> RestartPolicy {
        RestartPolicy {
            max_restarts: 5,
            first_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            stable_after: Duration::from_secs(120),
        }
    }
}

impl RestartPolicy {
    /// The wait before restart number `attempt`, counted from 1.
    pub(crate) fn delay(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        self.first_delay.saturating_mul(factor).min(self.max_delay)
    }
}

impl Inner {
    /// The answer to `workspace/configuration`: each item's section looked up in the settings.
    pub(crate) fn configuration(&self, params: &Value) -> Value {
        let settings = self.lock().settings.clone();
        let items = params
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Value::Array(
            items
                .iter()
                .map(|item| match item.get("section").and_then(Value::as_str) {
                    None | Some("") => settings.clone(),
                    Some(section) => section
                        .split('.')
                        .try_fold(&settings, |value, key| value.get(key))
                        .cloned()
                        .unwrap_or(Value::Null),
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configuration_does_not_print_the_environment() {
        let env = vec![("GITHUB_TOKEN".to_string(), "secret".to_string())];
        let config = ServerConfig::new("x", Path::new("/bin/x"), Path::new("/"), env);
        let printed = format!("{config:?}");
        assert!(!printed.contains("secret") && printed.contains("1 variables"));
    }

    #[test]
    fn restarts_wait_longer_each_time_up_to_the_limit() {
        let policy = RestartPolicy {
            max_restarts: 10,
            first_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(1000),
            stable_after: Duration::from_secs(1),
        };
        let delays: Vec<u128> = (1..=6).map(|n| policy.delay(n).as_millis()).collect();
        assert_eq!(delays, [100, 200, 400, 800, 1000, 1000]);
        assert_eq!(policy.delay(u32::MAX), policy.max_delay);
    }
}
