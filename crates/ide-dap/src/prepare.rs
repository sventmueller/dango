//! From a launch configuration to a running session: the trust check, building what the configuration asks Cargo
//! for, finding the adapter, and the settings each adapter needs.

use std::fmt;
use std::path::Path;

use ide_lsp::Cancel;
use serde_json::json;

use crate::adapter::AdapterKind;
use crate::current::{self, CurrentError};
use crate::discover::{Adapters, Missing};
use crate::launch::LaunchConfig;
use crate::session::{Event, Session, Setup};
use crate::types::{Request, SessionId};

/// Why a session did not start.
#[derive(Debug)]
pub enum StartError {
    /// The folder is not trusted, so nothing it configures runs (E22).
    Untrusted,
    /// The configuration waits for the user to pick a process (`LaunchConfig::process_pick`).
    NeedsProcess,
    /// The adapter is not installed.
    Missing(Missing),
    /// Building the program the configuration names failed.
    Build(CurrentError),
    /// The adapter process could not be started or reached.
    Io(std::io::Error),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartError::Untrusted => f.write_str("the folder is not trusted"),
            StartError::NeedsProcess => f.write_str("pick a process to attach to"),
            StartError::Missing(missing) => missing.fmt(f),
            StartError::Build(error) => error.fmt(f),
            StartError::Io(error) => write!(f, "the debug adapter could not be started: {error}"),
        }
    }
}

impl std::error::Error for StartError {}

/// Starts a session for `config` in the project at `root`, only when the folder is `trusted`: builds what the
/// configuration asks Cargo for, finds the adapter and starts it through the supervisor with the login shell's
/// environment. Blocks while it builds and looks, so call it off the main thread; `line` hears the build's progress.
#[allow(clippy::too_many_arguments, reason = "each is one thing a session is started from")]
pub fn start(
    adapters: &Adapters,
    root: &Path,
    trusted: bool,
    mut config: LaunchConfig,
    setup: Setup,
    cancel: &Cancel,
    line: &mut dyn FnMut(&str),
    events: impl Fn(SessionId, Event) + Send + Sync + 'static,
) -> Result<Session, StartError> {
    if !trusted {
        return Err(StartError::Untrusted);
    }
    if config.process_pick {
        return Err(StartError::NeedsProcess);
    }
    if config.kind == AdapterKind::Lldb && config.request == Request::Launch {
        current::resolve_cargo(adapters, root, &mut config.arguments, cancel, line).map_err(StartError::Build)?;
    }
    let adapter = adapters.find(config.kind, root).map_err(StartError::Missing)?;
    fill_in(adapters, root, &mut config);
    Session::start(adapters.supervisor(), adapter, config, setup, events).map_err(StartError::Io)
}

/// The settings each adapter needs and a configuration may leave out.
fn fill_in(adapters: &Adapters, root: &Path, config: &mut LaunchConfig) {
    let arguments = &mut config.arguments;
    match config.kind {
        AdapterKind::Lldb | AdapterKind::Go | AdapterKind::JsDebug => {
            if config.request == Request::Launch {
                arguments.entry("cwd").or_insert(json!(root));
            }
            if config.kind == AdapterKind::Go && config.request == Request::Launch {
                keep_paths(arguments);
            }
        }
        AdapterKind::Python => {
            if let Some(python) = adapters.python(root) {
                arguments.entry("python").or_insert(json!(python));
            }
            arguments.entry("cwd").or_insert(json!(root));
            arguments.entry("console").or_insert(json!("internalConsole"));
        }
        AdapterKind::Fake => {}
    }
}

/// Builds a Go program with its real source paths: a `-trimpath` in `GOFLAGS`, which Go reads from its own settings
/// too, leaves Delve unable to place any breakpoint, and a flag on the command line wins over `GOFLAGS`.
fn keep_paths(arguments: &mut serde_json::Map<String, serde_json::Value>) {
    const FLAG: &str = "-trimpath=false";
    match arguments.get_mut("buildFlags") {
        None => {
            arguments.insert("buildFlags".into(), json!(FLAG));
        }
        Some(serde_json::Value::String(flags)) if !flags.contains("trimpath") => {
            flags.push(' ');
            flags.push_str(FLAG);
        }
        Some(serde_json::Value::Array(flags))
            if !flags.iter().any(|f| f.as_str().is_some_and(|f| f.contains("trimpath"))) =>
        {
            flags.push(json!(FLAG));
        }
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, json};

    use super::keep_paths;

    #[test]
    fn a_go_build_keeps_its_paths_unless_the_configuration_says_otherwise() {
        let mut none = Map::new();
        keep_paths(&mut none);
        assert_eq!(none["buildFlags"], json!("-trimpath=false"));

        let mut text = Map::from_iter([("buildFlags".to_string(), json!("-tags=dev"))]);
        keep_paths(&mut text);
        assert_eq!(text["buildFlags"], json!("-tags=dev -trimpath=false"));

        let mut list = Map::from_iter([("buildFlags".to_string(), json!(["-race"]))]);
        keep_paths(&mut list);
        assert_eq!(list["buildFlags"], json!(["-race", "-trimpath=false"]));

        let mut chosen = Map::from_iter([("buildFlags".to_string(), json!("-trimpath"))]);
        keep_paths(&mut chosen);
        assert_eq!(chosen["buildFlags"], json!("-trimpath"));
    }
}
