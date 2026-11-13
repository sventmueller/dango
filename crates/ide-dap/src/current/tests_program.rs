//! Debugging one test (B27, C31, F27): the launch configuration for a test of a test adapter, with the runtime,
//! folder and environment of a normal run. A Rust test program is built in the profile `ide-debug` into a folder of its
//! own, as programs are; Go runs in Delve's test mode, pytest as a module through debugpy, Vitest and Jest through
//! js-debug with the runner's resolved entry.

use std::path::{Path, PathBuf};

use ide_net::Cancel;
use serde_json::{Map, Value, json};

use super::{BUILD_TIMEOUT, CurrentError, build_env, compiler_errors, rust_formatters};
use crate::adapter::AdapterKind;
use crate::discover::Adapters;
use crate::helper::{self, Helper};
use crate::launch::LaunchConfig;
use crate::types::Request;

/// What a test needs to run under a debugger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestProgram {
    /// A Rust test program: `cargo` with `build` in `cwd` names it in its JSON; it runs with `args`. `target` is the
    /// test program's name, such as the crate's or an integration test's.
    Native {
        /// Cargo's arguments that build the test programs.
        build: Vec<String>,
        /// Where Cargo runs.
        cwd: PathBuf,
        /// The test program's name.
        target: String,
        /// The test program's arguments, such as the test's name with `--exact`.
        args: Vec<String>,
    },
    /// A Go package's folder in Delve's test mode with `args`.
    Go {
        /// The package's folder.
        package: PathBuf,
        /// The test binary's arguments.
        args: Vec<String>,
    },
    /// A Python module with `args` in `cwd`.
    PythonModule {
        /// The module, such as `pytest`.
        module: String,
        /// Its arguments.
        args: Vec<String>,
        /// Where it runs.
        cwd: PathBuf,
    },
    /// A Node program with `args` in `cwd`.
    Node {
        /// The program.
        program: PathBuf,
        /// Its arguments.
        args: Vec<String>,
        /// Where it runs.
        cwd: PathBuf,
    },
}

/// The launch configuration named `name` for `test` in the trusted project at `root`; builds a Rust test program
/// first, so it blocks, and `line` hears the build. Call it off the main thread.
pub fn test_program(
    adapters: &Adapters,
    trusted: bool,
    name: &str,
    test: &TestProgram,
    cancel: &Cancel,
    line: &mut dyn FnMut(&str),
) -> Result<LaunchConfig, CurrentError> {
    if !trusted {
        return Err(CurrentError::Untrusted);
    }
    let (kind, arguments) = match test {
        TestProgram::Native {
            build,
            cwd,
            target,
            args,
        } => (
            AdapterKind::Lldb,
            native(adapters, build, cwd, target, args, cancel, line)?,
        ),
        TestProgram::Go { package, args } => (
            AdapterKind::Go,
            object(json!({"mode": "test", "program": package, "cwd": package, "args": args})),
        ),
        TestProgram::PythonModule { module, args, cwd } => (
            AdapterKind::Python,
            object(
                json!({"module": module, "args": args, "cwd": cwd, "console": "internalConsole", "justMyCode": false}),
            ),
        ),
        TestProgram::Node { program, args, cwd } => (
            AdapterKind::JsDebug,
            object(json!({"program": program, "args": args, "cwd": cwd, "console": "internalConsole"})),
        ),
    };
    Ok(LaunchConfig {
        name: name.to_string(),
        kind,
        request: Request::Launch,
        arguments,
        process_pick: false,
    })
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn native(
    adapters: &Adapters,
    build: &[String],
    cwd: &Path,
    target: &str,
    args: &[String],
    cancel: &Cancel,
    line: &mut dyn FnMut(&str),
) -> Result<Map<String, Value>, CurrentError> {
    let cargo = helper::which("cargo", adapters.path()).ok_or(CurrentError::Missing("cargo"))?;
    let helper = Helper {
        supervisor: adapters.supervisor(),
        env: &build_env(adapters),
        timeout: BUILD_TIMEOUT,
        cancel: Some(cancel),
    };
    let build: Vec<&str> = build.iter().map(String::as_str).collect();
    let output = helper
        .run(&cargo, &build, cwd, line)
        .map_err(|e| CurrentError::Build(e.to_string()))?;
    if output.ending.is_none() {
        return Err(CurrentError::Cancelled);
    }
    if !output.succeeded() {
        let errors = compiler_errors(&output.stdout);
        return Err(CurrentError::Build(if errors.is_empty() {
            output.stderr
        } else {
            errors
        }));
    }
    let program = test_executable(&output.stdout, target).ok_or(CurrentError::NoProgram)?;
    let mut arguments = object(json!({"program": program, "cwd": cwd, "args": args}));
    if let Some(commands) = rust_formatters(&helper, cwd) {
        arguments.insert("initCommands".into(), json!(commands));
    }
    Ok(arguments)
}

/// The test program of `target` among the artifacts in Cargo's JSON `stdout`; any test program without one.
fn test_executable(stdout: &str, target: &str) -> Option<PathBuf> {
    let artifacts: Vec<Value> = stdout
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|m| m.get("reason").and_then(Value::as_str) == Some("compiler-artifact"))
        .filter(|m| m["profile"]["test"].as_bool() == Some(true))
        .collect();
    let named = |m: &&Value| target.is_empty() || m["target"]["name"].as_str() == Some(target);
    artifacts
        .iter()
        .filter(named)
        .chain(artifacts.iter())
        .find_map(|m| m.get("executable").and_then(Value::as_str).map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::test_executable;

    #[test]
    fn the_test_program_of_the_named_target_is_taken() {
        let stdout = concat!(
            r#"{"reason":"compiler-artifact","target":{"name":"sample","kind":["lib"]},"profile":{"test":true},"executable":"/t/sample-1"}"#,
            "\n",
            r#"{"reason":"compiler-artifact","target":{"name":"api","kind":["test"]},"profile":{"test":true},"executable":"/t/api-2"}"#,
            "\n",
            r#"{"reason":"build-finished","success":true}"#,
        );
        assert_eq!(
            test_executable(stdout, "api").as_deref(),
            Some(std::path::Path::new("/t/api-2"))
        );
        assert_eq!(
            test_executable(stdout, "sample").as_deref(),
            Some(std::path::Path::new("/t/sample-1"))
        );
        assert_eq!(
            test_executable(stdout, "").as_deref(),
            Some(std::path::Path::new("/t/sample-1"))
        );
    }
}
