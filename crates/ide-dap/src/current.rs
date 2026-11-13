//! "Debug the current program" without a configuration: Rust through `cargo build --message-format=json` in a profile
//! with full debug information (the binary or the tests the file belongs to), Go through the file's package, Node and
//! Python through the file itself.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_lsp::Cancel;
use serde_json::{Map, Value, json};

use crate::adapter::AdapterKind;
use crate::discover::Adapters;
use crate::helper::{self, Helper};
use crate::launch::LaunchConfig;
use crate::types::Request;

/// How long a build may take.
const BUILD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Why the current file cannot be debugged as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentError {
    /// The folder is not trusted, so nothing in it runs (E22).
    Untrusted,
    /// No debugger knows this language without a configuration.
    Unsupported(String),
    /// A program the build needs is missing from the login shell's PATH, such as `cargo`.
    Missing(&'static str),
    /// The file belongs to no Cargo package.
    NoPackage,
    /// The package has several binaries and the file belongs to none of them; a configuration has to say which.
    WhichBinary(Vec<String>),
    /// The build failed; the compiler's errors, as it wrote them.
    Build(String),
    /// The build was cancelled.
    Cancelled,
    /// The build succeeded but named no program to run.
    NoProgram,
}

impl fmt::Display for CurrentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CurrentError::Untrusted => f.write_str("the folder is not trusted"),
            CurrentError::Unsupported(language) => {
                write!(f, "{language} files cannot be debugged without a configuration")
            }
            CurrentError::Missing(program) => write!(f, "{program} is not installed"),
            CurrentError::NoPackage => f.write_str("the file belongs to no Cargo package"),
            CurrentError::WhichBinary(names) => write!(f, "the package has several binaries: {}", names.join(", ")),
            CurrentError::Build(errors) => write!(f, "the build failed:\n{errors}"),
            CurrentError::Cancelled => f.write_str("the build was cancelled"),
            CurrentError::NoProgram => f.write_str("the build produced no program"),
        }
    }
}

impl std::error::Error for CurrentError {}

/// A launch configuration for `file`, a document of the editor's `language`, in the trusted project at `root`;
/// builds a Rust program first, so it blocks, and `line` hears the build's progress. Call it off the main thread.
pub fn current_program(
    adapters: &Adapters,
    root: &Path,
    trusted: bool,
    file: &Path,
    language: &str,
    cancel: &Cancel,
    line: &mut dyn FnMut(&str),
) -> Result<LaunchConfig, CurrentError> {
    if !trusted {
        return Err(CurrentError::Untrusted);
    }
    let kind = AdapterKind::for_language(language).ok_or_else(|| CurrentError::Unsupported(language.to_string()))?;
    let name = file
        .file_name()
        .map_or_else(|| file.display().to_string(), |n| n.to_string_lossy().into_owned());
    let dir = file.parent().unwrap_or(root);
    let mut arguments = Map::new();
    match (kind, language) {
        (AdapterKind::Lldb, "rust") => return rust(adapters, file, cancel, line),
        (AdapterKind::Go, _) => {
            let test = name.ends_with("_test.go");
            arguments.insert("mode".into(), json!(if test { "test" } else { "debug" }));
            arguments.insert("program".into(), json!(dir));
            arguments.insert("cwd".into(), json!(dir));
        }
        (AdapterKind::JsDebug, _) => {
            arguments.insert("program".into(), json!(file));
            arguments.insert("cwd".into(), json!(root));
        }
        (AdapterKind::Python, _) => {
            arguments.insert("program".into(), json!(file));
            arguments.insert("cwd".into(), json!(root));
            arguments.insert("console".into(), json!("internalConsole"));
        }
        _ => return Err(CurrentError::Unsupported(language.to_string())),
    }
    Ok(LaunchConfig {
        name,
        kind,
        request: Request::Launch,
        arguments,
        process_pick: false,
    })
}

/// The Cargo profile debug builds use: the dev profile with full debug information, which a Cargo configuration may
/// lower to line tables and so leave the debugger without variables. It is defined through the environment, so the
/// project's own files stay untouched, and builds into a folder of its own, so the dev build is not invalidated.
const PROFILE: &str = "ide-debug";

/// The login shell's environment with `PROFILE` defined.
fn build_env(adapters: &Adapters) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = adapters
        .env()
        .iter()
        .filter(|(key, _)| !key.starts_with("CARGO_PROFILE_IDE_DEBUG_"))
        .cloned()
        .collect();
    env.push(("CARGO_PROFILE_IDE_DEBUG_INHERITS".into(), "dev".into()));
    env.push(("CARGO_PROFILE_IDE_DEBUG_DEBUG".into(), "true".into()));
    env
}

/// A target of a Cargo package, as `cargo metadata` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    name: String,
    kinds: Vec<String>,
    src_path: PathBuf,
}

/// What to build so the file can be debugged.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Plan {
    package: String,
    /// `build` or `test`.
    command: &'static str,
    /// Such as `--bin`, `--test`, `--example` or `--lib`.
    selector: &'static str,
    /// The target's name; empty for `--lib`.
    target: String,
    /// The target kind the artifact carries, such as `bin` or `lib`.
    kind: &'static str,
}

fn rust(
    adapters: &Adapters,
    file: &Path,
    cancel: &Cancel,
    line: &mut dyn FnMut(&str),
) -> Result<LaunchConfig, CurrentError> {
    let cargo = helper::which("cargo", adapters.path()).ok_or(CurrentError::Missing("cargo"))?;
    // Cargo names folders without symbolic links, such as /private/var for /var.
    let file = &std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let dir = file.parent().ok_or(CurrentError::NoPackage)?;
    let helper = Helper {
        supervisor: adapters.supervisor(),
        env: &build_env(adapters),
        timeout: BUILD_TIMEOUT,
        cancel: Some(cancel),
    };
    let metadata = helper
        .run(
            &cargo,
            &["metadata", "--no-deps", "--format-version", "1"],
            dir,
            &mut |_| {},
        )
        .map_err(|e| CurrentError::Build(e.to_string()))?;
    if !metadata.succeeded() {
        return Err(CurrentError::Build(metadata.stderr));
    }
    let metadata: Value = serde_json::from_str(&metadata.stdout).map_err(|e| CurrentError::Build(e.to_string()))?;
    let (package, manifest_dir, targets) = package_of(&metadata, file).ok_or(CurrentError::NoPackage)?;
    let source = std::fs::read_to_string(file).unwrap_or_default();
    let plan = plan(&package, &targets, file, &source)?;
    let mut args = vec![plan.command];
    if plan.command == "test" {
        args.push("--no-run");
    }
    args.extend([
        "--profile",
        PROFILE,
        "--message-format=json",
        "--package",
        &plan.package,
        plan.selector,
    ]);
    if !plan.target.is_empty() {
        args.push(&plan.target);
    }
    let output = helper
        .run(&cargo, &args, &manifest_dir, line)
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
    let program = executable(&output.stdout, &plan).ok_or(CurrentError::NoProgram)?;
    let mut arguments = Map::new();
    arguments.insert("program".into(), json!(program));
    arguments.insert("cwd".into(), json!(manifest_dir));
    arguments.insert("args".into(), json!([]));
    if let Some(commands) = rust_formatters(&helper, &manifest_dir) {
        arguments.insert("initCommands".into(), json!(commands));
    }
    let name = if plan.target.is_empty() {
        format!("{} (tests)", plan.package)
    } else {
        plan.target.clone()
    };
    Ok(LaunchConfig {
        name,
        kind: AdapterKind::Lldb,
        request: Request::Launch,
        arguments,
        process_pick: false,
    })
}

/// The package whose folder holds `file`, the innermost if they nest: its name, folder and targets.
fn package_of(metadata: &Value, file: &Path) -> Option<(String, PathBuf, Vec<Target>)> {
    metadata
        .get("packages")?
        .as_array()?
        .iter()
        .filter_map(|package| {
            let manifest = PathBuf::from(package.get("manifest_path")?.as_str()?);
            let dir = manifest.parent()?.to_path_buf();
            file.starts_with(&dir).then_some((package, dir))
        })
        .max_by_key(|(_, dir)| dir.components().count())
        .map(|(package, dir)| {
            let targets = package
                .get("targets")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(target).collect())
                .unwrap_or_default();
            let name = package
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            (name, dir, targets)
        })
}

fn target(value: &Value) -> Option<Target> {
    Some(Target {
        name: value.get("name")?.as_str()?.to_string(),
        kinds: value
            .get("kind")?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        src_path: PathBuf::from(value.get("src_path")?.as_str()?),
    })
}

/// What to build for `file`: its own target if it is a target's root, the unit tests if it has tests, otherwise
/// the package's binary.
fn plan(package: &str, targets: &[Target], file: &Path, source: &str) -> Result<Plan, CurrentError> {
    let make = |command, selector, target: &str, kind| Plan {
        package: package.to_string(),
        command,
        selector,
        target: target.to_string(),
        kind,
    };
    let has = |t: &Target, kind: &str| t.kinds.iter().any(|k| k == kind);
    if let Some(own) = targets.iter().find(|t| t.src_path == file) {
        if has(own, "bin") && !source.contains("#[test]") {
            return Ok(make("build", "--bin", &own.name, "bin"));
        }
        if has(own, "bin") {
            return Ok(make("test", "--bin", &own.name, "bin"));
        }
        if has(own, "test") {
            return Ok(make("test", "--test", &own.name, "test"));
        }
        if has(own, "example") {
            return Ok(make("build", "--example", &own.name, "example"));
        }
    }
    let lib = targets
        .iter()
        .any(|t| has(t, "lib") || has(t, "rlib") || has(t, "proc-macro"));
    if source.contains("#[test]") && lib {
        return Ok(make("test", "--lib", "", "lib"));
    }
    let bins: Vec<&Target> = targets.iter().filter(|t| has(t, "bin")).collect();
    match bins.as_slice() {
        [only] => Ok(make("build", "--bin", &only.name, "bin")),
        [] if lib => Ok(make("test", "--lib", "", "lib")),
        [] => Err(CurrentError::NoProgram),
        many => match many.iter().find(|t| t.name == package) {
            Some(main) => Ok(make("build", "--bin", &main.name, "bin")),
            None => Err(CurrentError::WhichBinary(many.iter().map(|t| t.name.clone()).collect())),
        },
    }
}

/// The program the build made for `plan`, from cargo's JSON messages.
fn executable(stdout: &str, plan: &Plan) -> Option<PathBuf> {
    stdout
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|m| m.get("reason").and_then(Value::as_str) == Some("compiler-artifact"))
        .filter(|m| {
            let target = &m["target"];
            let kinds = target["kind"].as_array().map(Vec::as_slice).unwrap_or_default();
            let kind_matches = kinds.iter().any(|k| k.as_str() == Some(plan.kind))
                || (plan.kind == "lib" && kinds.iter().any(|k| matches!(k.as_str(), Some("rlib" | "proc-macro"))));
            let name_matches = plan.target.is_empty() || target["name"].as_str() == Some(plan.target.as_str());
            kind_matches && name_matches
        })
        .filter_map(|m| m.get("executable").and_then(Value::as_str).map(PathBuf::from))
        .next()
}

/// The errors of a failed build as rustc wrote them.
fn compiler_errors(stdout: &str) -> String {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|m| m.get("reason").and_then(Value::as_str) == Some("compiler-message"))
        .filter(|m| m["message"]["level"].as_str() == Some("error"))
        .filter_map(|m| m["message"]["rendered"].as_str().map(str::to_string))
        .collect()
}

/// The commands that load the toolchain's LLDB formatters for Rust's types, as `rust-lldb` does. Newer toolchains
/// register them when `lldb_lookup.py` is imported and no longer ship the `lldb_commands` file older ones needed.
fn rust_formatters(helper: &Helper<'_>, dir: &Path) -> Option<Vec<String>> {
    let rustc = helper::which("rustc", helper::var(helper.env, "PATH"))?;
    let output = helper.run(&rustc, &["--print", "sysroot"], dir, &mut |_| {}).ok()?;
    let etc = Path::new(output.stdout.trim()).join("lib/rustlib/etc");
    let lookup = etc.join("lldb_lookup.py");
    if !lookup.is_file() {
        return None;
    }
    let mut commands = vec![format!("command script import \"{}\"", lookup.display())];
    let registration = etc.join("lldb_commands");
    if registration.is_file() {
        commands.push(format!("command source -s 0 \"{}\"", registration.display()));
    }
    Some(commands)
}

/// Fills in `program` of a configuration that asks Cargo to build it, as other editors' Rust configurations do:
/// `"cargo": {"args": ["build", "--bin=x"], "filter": {"name": "x", "kind": "bin"}}`.
pub(crate) fn resolve_cargo(
    adapters: &Adapters,
    root: &Path,
    arguments: &mut Map<String, Value>,
    cancel: &Cancel,
    line: &mut dyn FnMut(&str),
) -> Result<(), CurrentError> {
    let Some(cargo_config) = arguments.remove("cargo") else {
        return Ok(());
    };
    let cargo = helper::which("cargo", adapters.path()).ok_or(CurrentError::Missing("cargo"))?;
    let mut args: Vec<String> = cargo_config
        .get("args")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_else(|| vec!["build".to_string()]);
    if args.first().is_some_and(|a| a == "test") && !args.iter().any(|a| a == "--no-run") {
        args.insert(1, "--no-run".into());
    }
    if !args.iter().any(|a| a == "--release" || a.starts_with("--profile")) {
        args.extend(["--profile".to_string(), PROFILE.to_string()]);
    }
    args.push("--message-format=json".into());
    let helper = Helper {
        supervisor: adapters.supervisor(),
        env: &build_env(adapters),
        timeout: BUILD_TIMEOUT,
        cancel: Some(cancel),
    };
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = helper
        .run(&cargo, &refs, root, line)
        .map_err(|e| CurrentError::Build(e.to_string()))?;
    if output.ending.is_none() {
        return Err(CurrentError::Cancelled);
    }
    if !output.succeeded() {
        return Err(CurrentError::Build(compiler_errors(&output.stdout)));
    }
    let filter = cargo_config.get("filter");
    let wanted_name = filter.and_then(|f| f.get("name")).and_then(Value::as_str);
    let wanted_kind = filter.and_then(|f| f.get("kind")).and_then(Value::as_str);
    let programs: Vec<PathBuf> = output
        .stdout
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|m| m.get("reason").and_then(Value::as_str) == Some("compiler-artifact"))
        .filter(|m| wanted_name.is_none_or(|n| m["target"]["name"].as_str() == Some(n)))
        .filter(|m| {
            wanted_kind.is_none_or(|k| {
                m["target"]["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|x| x.as_str() == Some(k)))
            })
        })
        .filter_map(|m| m.get("executable").and_then(Value::as_str).map(PathBuf::from))
        .collect();
    match programs.as_slice() {
        [one] => {
            arguments.insert("program".into(), json!(one));
            Ok(())
        }
        [] => Err(CurrentError::NoProgram),
        many => Err(CurrentError::WhichBinary(
            many.iter().map(|p| p.display().to_string()).collect(),
        )),
    }
}

#[cfg(test)]
#[path = "current_tests.rs"]
mod cargo_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Vec<Target> {
        let target = |name: &str, kind: &str, path: &str| Target {
            name: name.into(),
            kinds: vec![kind.into()],
            src_path: PathBuf::from(path),
        };
        vec![
            target("app", "lib", "/p/src/lib.rs"),
            target("app", "bin", "/p/src/main.rs"),
            target("tool", "bin", "/p/src/bin/tool.rs"),
            target("api", "test", "/p/tests/api.rs"),
            target("demo", "example", "/p/examples/demo.rs"),
        ]
    }

    fn planned(file: &str, source: &str) -> Result<(String, &'static str, &'static str), CurrentError> {
        plan("app", &targets(), Path::new(file), source).map(|p| (p.target, p.command, p.selector))
    }

    #[test]
    fn the_file_decides_between_a_binary_its_tests_and_the_packages_binary() {
        assert_eq!(
            planned("/p/src/bin/tool.rs", "fn main() {}"),
            Ok(("tool".into(), "build", "--bin"))
        );
        assert_eq!(
            planned("/p/tests/api.rs", "#[test] fn t() {}"),
            Ok(("api".into(), "test", "--test"))
        );
        assert_eq!(
            planned("/p/examples/demo.rs", ""),
            Ok(("demo".into(), "build", "--example"))
        );
        assert_eq!(
            planned("/p/src/parse.rs", "#[test]\nfn t() {}"),
            Ok((String::new(), "test", "--lib"))
        );
        assert_eq!(
            planned("/p/src/parse.rs", "pub fn f() {}"),
            Ok(("app".into(), "build", "--bin"))
        );
        assert_eq!(
            planned("/p/src/main.rs", "#[test] fn t() {}"),
            Ok(("app".into(), "test", "--bin"))
        );
    }

    #[test]
    fn several_binaries_without_the_packages_own_need_a_configuration() {
        let only_bins: Vec<Target> = targets().into_iter().filter(|t| t.name == "tool").collect();
        let mut two = only_bins.clone();
        two.push(Target {
            name: "other".into(),
            ..only_bins[0].clone()
        });
        let result = plan("app", &two, Path::new("/p/src/util.rs"), "");
        assert_eq!(
            result,
            Err(CurrentError::WhichBinary(vec!["tool".into(), "other".into()]))
        );
    }

    #[test]
    fn the_program_is_read_from_cargos_messages() {
        let stdout = r#"{"reason":"compiler-artifact","target":{"name":"app","kind":["lib"]},"executable":null}
{"reason":"compiler-artifact","target":{"name":"app","kind":["lib"]},"profile":{"test":true},"executable":"/p/target/debug/deps/app-1f"}
{"reason":"compiler-artifact","target":{"name":"tool","kind":["bin"]},"executable":"/p/target/debug/tool"}
{"reason":"build-finished","success":true}"#;
        let lib = Plan {
            package: "app".into(),
            command: "test",
            selector: "--lib",
            target: String::new(),
            kind: "lib",
        };
        assert_eq!(
            executable(stdout, &lib),
            Some(PathBuf::from("/p/target/debug/deps/app-1f"))
        );
        let tool = Plan {
            target: "tool".into(),
            kind: "bin",
            ..lib
        };
        assert_eq!(executable(stdout, &tool), Some(PathBuf::from("/p/target/debug/tool")));
        let errors = compiler_errors(
            r#"{"reason":"compiler-message","message":{"level":"error","rendered":"error[E0425]: x\n"}}"#,
        );
        assert_eq!(errors, "error[E0425]: x\n");
    }

    #[test]
    fn go_node_and_python_files_need_no_build_and_untrusted_folders_nothing() {
        let adapters = Adapters::with_tools(&ide_process::Supervisor::new(), Vec::new(), PathBuf::from("/none"));
        let go = current_program(
            &adapters,
            Path::new("/p"),
            true,
            Path::new("/p/cmd/x_test.go"),
            "go",
            &Cancel::new(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            (go.arguments["mode"].as_str(), go.arguments["program"].as_str()),
            (Some("test"), Some("/p/cmd"))
        );
        let py = current_program(
            &adapters,
            Path::new("/p"),
            true,
            Path::new("/p/a.py"),
            "python",
            &Cancel::new(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(py.kind, AdapterKind::Python);
        let untrusted = current_program(
            &adapters,
            Path::new("/p"),
            false,
            Path::new("/p/a.py"),
            "python",
            &Cancel::new(),
            &mut |_| {},
        );
        assert_eq!(untrusted, Err(CurrentError::Untrusted));
        let cpp = current_program(
            &adapters,
            Path::new("/p"),
            true,
            Path::new("/p/a.cpp"),
            "cpp",
            &Cancel::new(),
            &mut |_| {},
        );
        assert!(matches!(cpp, Err(CurrentError::Unsupported(_))));
    }
}
