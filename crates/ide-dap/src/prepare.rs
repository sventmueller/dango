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
    use std::path::{Path, PathBuf};

    use ide_lsp::Cancel;
    use ide_process::Supervisor;
    use serde_json::{Map, json};

    use super::*;
    use crate::stub_tests::{env, folder, script};

    fn config(kind: AdapterKind, request: Request, arguments: serde_json::Value) -> LaunchConfig {
        LaunchConfig {
            name: "app".into(),
            kind,
            request,
            arguments: arguments.as_object().cloned().unwrap_or_default(),
            process_pick: false,
        }
    }

    fn started(adapters: &Adapters, root: &Path, trusted: bool, config: LaunchConfig) -> Result<Session, StartError> {
        start(
            adapters,
            root,
            trusted,
            config,
            Setup::default(),
            &Cancel::new(),
            &mut |_| {},
            |_, _| {},
        )
    }

    #[test]
    fn nothing_starts_in_an_untrusted_folder_or_before_a_process_is_picked() {
        let adapters = Adapters::with_tools(&Supervisor::new(), Vec::new(), PathBuf::from("/none"));
        let fake = config(AdapterKind::Fake, Request::Launch, json!({}));
        let untrusted = started(&adapters, Path::new("/p"), false, fake.clone());
        assert!(matches!(untrusted, Err(StartError::Untrusted)));
        let pick = LaunchConfig {
            process_pick: true,
            ..config(AdapterKind::Lldb, Request::Attach, json!({}))
        };
        assert!(matches!(
            started(&adapters, Path::new("/p"), true, pick),
            Err(StartError::NeedsProcess)
        ));
    }

    #[test]
    fn a_missing_adapter_or_cargo_is_named_and_the_fake_adapter_starts() {
        let adapters = Adapters::with_tools(&Supervisor::new(), env(&folder("empty"), &[]), PathBuf::from("/none"));
        let go = started(
            &adapters,
            Path::new("/p"),
            true,
            config(AdapterKind::Go, Request::Launch, json!({})),
        );
        assert!(
            matches!(go, Err(StartError::Missing(Missing { program: "dlv", .. }))),
            "{go:?}"
        );
        let cargo = config(
            AdapterKind::Lldb,
            Request::Launch,
            json!({"cargo": {"args": ["build"]}}),
        );
        let built = started(&adapters, Path::new("/p"), true, cargo);
        assert!(
            matches!(built, Err(StartError::Build(CurrentError::Missing("cargo")))),
            "{built:?}"
        );
        let session = started(
            &adapters,
            Path::new("/p"),
            true,
            config(AdapterKind::Fake, Request::Launch, json!({})),
        )
        .unwrap();
        assert!(session.stop_and_wait(std::time::Duration::from_secs(10)));
    }

    #[test]
    fn each_adapter_gets_the_settings_a_configuration_may_leave_out() {
        let root = folder("fill");
        script(&root.join(".venv/bin/python"), "exit 0\n");
        let adapters = Adapters::with_tools(&Supervisor::new(), env(&root.join("bin"), &[]), root.clone());
        let filled = |kind, request, arguments| {
            let mut config = config(kind, request, arguments);
            fill_in(&adapters, &root, &mut config);
            serde_json::Value::Object(config.arguments)
        };
        assert_eq!(
            filled(
                AdapterKind::Python,
                Request::Launch,
                json!({"console": "integratedTerminal"})
            ),
            json!({"python": root.join(".venv/bin/python"), "cwd": root, "console": "integratedTerminal"})
        );
        assert_eq!(
            filled(AdapterKind::Lldb, Request::Launch, json!({})),
            json!({"cwd": root})
        );
        assert_eq!(
            filled(AdapterKind::JsDebug, Request::Launch, json!({"cwd": "/w"})),
            json!({"cwd": "/w"})
        );
        assert_eq!(
            filled(AdapterKind::Lldb, Request::Attach, json!({"pid": 7})),
            json!({"pid": 7})
        );
        assert_eq!(
            filled(AdapterKind::Go, Request::Launch, json!({})),
            json!({"cwd": root, "buildFlags": "-trimpath=false"})
        );
        assert_eq!(
            filled(AdapterKind::Go, Request::Attach, json!({"processId": 7})),
            json!({"processId": 7})
        );
        assert_eq!(filled(AdapterKind::Fake, Request::Launch, json!({})), json!({}));
    }

    #[test]
    fn every_reason_a_start_fails_reads_as_a_sentence() {
        let missing = Missing {
            kind: AdapterKind::Go,
            program: "dlv",
            needs: None,
            install: None,
            command: None,
        };
        let cases = [
            (StartError::Untrusted, "the folder is not trusted"),
            (StartError::NeedsProcess, "pick a process to attach to"),
            (StartError::Missing(missing), "dlv is not installed"),
            (
                StartError::Build(CurrentError::NoProgram),
                "the build produced no program",
            ),
            (
                StartError::Io(std::io::Error::other("no such file")),
                "the debug adapter could not be started: no such file",
            ),
        ];
        for (error, words) in cases {
            assert_eq!(error.to_string(), words);
        }
    }

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
