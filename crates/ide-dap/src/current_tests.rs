//! Building the current Rust program and the program a configuration's `cargo` names, with stand-ins for `cargo`
//! and `rustc` that answer as the real ones do.

use std::path::{Path, PathBuf};

use ide_net::Cancel;
use ide_process::Supervisor;
use serde_json::{Map, Value, json};

use super::{CurrentError, current_program, resolve_cargo};
use crate::adapter::AdapterKind;
use crate::discover::Adapters;
use crate::stub_tests::{env, folder, printed, script};

/// A Cargo package in a folder of its own, with stand-ins for its tools in `bin`.
struct Package {
    dir: ide_core::testing::Scratch,
    bin: PathBuf,
    log: PathBuf,
}

impl Package {
    fn new(files: &[(&str, &str)]) -> Package {
        let dir = folder("cargo");
        for (name, text) in files {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        Package {
            bin: dir.join("bin"),
            log: dir.join("calls.log"),
            dir,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// `cargo metadata` lists the package `app` with `targets`, each `(name, kind, file)`; `cargo build` and
    /// `cargo test` print `progress` on standard error, then `build` on standard output, and exit with `code`.
    fn cargo(&self, targets: &[(&str, &str, &str)], progress: &str, build: &str, code: i32) {
        let targets: Vec<Value> = targets
            .iter()
            .map(|(name, kind, file)| json!({"name": name, "kind": [kind], "src_path": self.path(file)}))
            .collect();
        let metadata = json!({"packages": [{
            "name": "app",
            "manifest_path": self.path("Cargo.toml"),
            "targets": targets,
        }]});
        let body = format!(
            "echo \"$* | $CARGO_PROFILE_IDE_DEBUG_INHERITS $CARGO_PROFILE_IDE_DEBUG_DEBUG \
             ${{CARGO_PROFILE_IDE_DEBUG_OPT_LEVEL:-none}}\" >> '{log}'\n\
             case \"$1\" in\nmetadata)\n{metadata}exit 0;;\nesac\n\
             echo '{progress}' >&2\n{build}exit {code}\n",
            log = self.log.display(),
            metadata = printed(&metadata.to_string()),
            build = printed(build),
        );
        script(&self.bin.join("cargo"), &body);
    }

    /// `rustc --print sysroot` names a toolchain whose `lib/rustlib/etc` holds `files`.
    fn rustc(&self, files: &[&str]) -> PathBuf {
        let sysroot = self.dir.join("toolchain");
        let etc = sysroot.join("lib/rustlib/etc");
        std::fs::create_dir_all(&etc).unwrap();
        for file in files {
            std::fs::write(etc.join(file), "").unwrap();
        }
        script(&self.bin.join("rustc"), &printed(&sysroot.display().to_string()));
        etc
    }

    fn adapters(&self) -> Adapters {
        let env = env(&self.bin, &[("CARGO_PROFILE_IDE_DEBUG_OPT_LEVEL", "3")]);
        Adapters::with_tools(&Supervisor::new(), env, self.dir.join("tools"))
    }

    /// How each `cargo` was called, with the profile it got from the environment.
    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn current(
        &self,
        file: &str,
        cancel: &Cancel,
        heard: &mut Vec<String>,
    ) -> Result<crate::LaunchConfig, CurrentError> {
        current_program(
            &self.adapters(),
            &self.dir,
            true,
            &self.path(file),
            "rust",
            cancel,
            &mut |line| {
                heard.push(line.to_string());
            },
        )
    }
}

fn artifact(name: &str, kind: &str, executable: &Path) -> String {
    json!({"reason": "compiler-artifact", "target": {"name": name, "kind": [kind]}, "executable": executable})
        .to_string()
}

fn error(rendered: &str) -> String {
    json!({"reason": "compiler-message", "message": {"level": "error", "rendered": rendered}}).to_string()
}

#[test]
fn a_binary_is_built_with_full_debug_information_and_launched_with_the_toolchains_formatters() {
    let package = Package::new(&[("Cargo.toml", ""), ("src/main.rs", "fn main() {}\n")]);
    let program = package.path("target/ide-debug/app");
    package.cargo(
        &[("app", "bin", "src/main.rs")],
        "   Compiling app v0.1.0",
        &artifact("app", "bin", &program),
        0,
    );
    let etc = package.rustc(&["lldb_lookup.py", "lldb_commands"]);
    let mut heard = Vec::new();
    let config = package.current("src/main.rs", &Cancel::new(), &mut heard).unwrap();

    assert_eq!((config.name.as_str(), config.kind), ("app", AdapterKind::Lldb));
    assert_eq!(config.arguments["program"], json!(program));
    assert_eq!(config.arguments["cwd"], json!(package.dir.path()));
    assert_eq!(config.arguments["args"], json!([]));
    assert_eq!(
        config.arguments["initCommands"],
        json!([
            format!("command script import \"{}\"", etc.join("lldb_lookup.py").display()),
            format!("command source -s 0 \"{}\"", etc.join("lldb_commands").display()),
        ])
    );
    assert_eq!(heard, ["   Compiling app v0.1.0"]);
    assert_eq!(
        package.calls(),
        [
            "metadata --no-deps --format-version 1 | dev true none",
            "build --profile ide-debug --message-format=json --package app --bin app | dev true none",
        ]
    );
}

#[test]
fn a_library_file_with_tests_builds_the_unit_tests_and_a_newer_toolchain_needs_only_the_lookup() {
    let package = Package::new(&[
        ("Cargo.toml", ""),
        ("src/lib.rs", ""),
        ("src/parse.rs", "#[test]\nfn t() {}\n"),
    ]);
    let tests = package.path("target/ide-debug/deps/app-1f");
    package.cargo(&[("app", "lib", "src/lib.rs")], "", &artifact("app", "lib", &tests), 0);
    let etc = package.rustc(&["lldb_lookup.py"]);
    let config = package
        .current("src/parse.rs", &Cancel::new(), &mut Vec::new())
        .unwrap();

    assert_eq!(config.name, "app (tests)");
    assert_eq!(config.arguments["program"], json!(tests));
    assert_eq!(
        config.arguments["initCommands"],
        json!([format!(
            "command script import \"{}\"",
            etc.join("lldb_lookup.py").display()
        )])
    );
    assert!(
        package.calls()[1].starts_with("test --no-run --profile ide-debug --message-format=json --package app --lib |")
    );
}

#[test]
fn without_the_toolchains_lookup_script_no_formatters_are_loaded() {
    let package = Package::new(&[("Cargo.toml", ""), ("src/main.rs", "fn main() {}\n")]);
    package.cargo(
        &[("app", "bin", "src/main.rs")],
        "",
        &artifact("app", "bin", Path::new("/p/app")),
        0,
    );
    package.rustc(&["lldb_commands"]);
    let config = package.current("src/main.rs", &Cancel::new(), &mut Vec::new()).unwrap();
    assert!(!config.arguments.contains_key("initCommands"));
}

#[test]
fn a_failed_build_reports_the_compilers_errors_or_else_what_cargo_wrote() {
    let package = Package::new(&[("Cargo.toml", ""), ("src/main.rs", "fn main() {}\n")]);
    package.cargo(
        &[("app", "bin", "src/main.rs")],
        "progress",
        &error("error[E0425]: x\n"),
        101,
    );
    assert_eq!(
        package.current("src/main.rs", &Cancel::new(), &mut Vec::new()),
        Err(CurrentError::Build("error[E0425]: x\n".into()))
    );

    package.cargo(
        &[("app", "bin", "src/main.rs")],
        "error: could not find `Cargo.toml`",
        "",
        101,
    );
    assert_eq!(
        package.current("src/main.rs", &Cancel::new(), &mut Vec::new()),
        Err(CurrentError::Build("error: could not find `Cargo.toml`\n".into()))
    );
}

#[test]
fn a_build_without_a_program_a_file_outside_the_package_and_a_failing_metadata_each_say_so() {
    let package = Package::new(&[("Cargo.toml", ""), ("src/main.rs", "fn main() {}\n")]);
    package.cargo(
        &[("app", "bin", "src/main.rs")],
        "",
        "{\"reason\":\"build-finished\"}",
        0,
    );
    assert_eq!(
        package.current("src/main.rs", &Cancel::new(), &mut Vec::new()),
        Err(CurrentError::NoProgram)
    );

    let scratch_dir = folder("outside");
    let outside = scratch_dir.join("main.rs");
    std::fs::write(&outside, "fn main() {}\n").unwrap();
    let result = current_program(
        &package.adapters(),
        &package.dir,
        true,
        &outside,
        "rust",
        &Cancel::new(),
        &mut |_| {},
    );
    assert_eq!(result, Err(CurrentError::NoPackage));

    script(
        &package.bin.join("cargo"),
        "echo 'error: manifest is broken' >&2\nexit 101\n",
    );
    assert_eq!(
        package.current("src/main.rs", &Cancel::new(), &mut Vec::new()),
        Err(CurrentError::Build("error: manifest is broken\n".into()))
    );

    let none = Adapters::with_tools(&Supervisor::new(), env(&folder("empty"), &[]), PathBuf::from("/none"));
    let result = current_program(
        &none,
        &package.dir,
        true,
        &package.path("src/main.rs"),
        "rust",
        &Cancel::new(),
        &mut |_| {},
    );
    assert_eq!(result, Err(CurrentError::Missing("cargo")));
}

#[test]
fn cancelling_ends_the_build_and_says_it_was_cancelled() {
    let package = Package::new(&[("Cargo.toml", ""), ("src/main.rs", "fn main() {}\n")]);
    script(
        &package.bin.join("cargo"),
        &format!(
            "case \"$1\" in\nmetadata)\n{}exit 0;;\nesac\necho '   Compiling app' >&2\nsleep 30\n",
            printed(
                &json!({"packages": [{"name": "app", "manifest_path": package.path("Cargo.toml"),
                "targets": [{"name": "app", "kind": ["bin"], "src_path": package.path("src/main.rs")}]}]})
                .to_string()
            )
        ),
    );
    let cancel = Cancel::new();
    let started = std::time::Instant::now();
    let result = current_program(
        &package.adapters(),
        &package.dir,
        true,
        &package.path("src/main.rs"),
        "rust",
        &cancel,
        &mut |_| cancel.cancel(),
    );
    assert_eq!(result, Err(CurrentError::Cancelled));
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

#[test]
fn every_reason_reads_as_a_sentence() {
    let cases = [
        (CurrentError::Untrusted, "the folder is not trusted"),
        (
            CurrentError::Unsupported("cpp".into()),
            "cpp files cannot be debugged without a configuration",
        ),
        (CurrentError::Missing("cargo"), "cargo is not installed"),
        (CurrentError::NoPackage, "the file belongs to no Cargo package"),
        (
            CurrentError::WhichBinary(vec!["a".into(), "b".into()]),
            "the package has several binaries: a, b",
        ),
        (CurrentError::Build("boom".into()), "the build failed:\nboom"),
        (CurrentError::Cancelled, "the build was cancelled"),
        (CurrentError::NoProgram, "the build produced no program"),
        (
            CurrentError::LibraryOnly("lib".into()),
            "lib is a library without a program",
        ),
    ];
    for (error, words) in cases {
        assert_eq!(error.to_string(), words);
    }
}

/// `resolve_cargo` on `cargo`, the `cargo` object of a configuration; the arguments it leaves.
fn resolved(package: &Package, cargo: Value, cancel: &Cancel) -> (Result<(), CurrentError>, Map<String, Value>) {
    let mut arguments = Map::from_iter([("cargo".to_string(), cargo), ("cwd".to_string(), json!("/p"))]);
    let result = resolve_cargo(&package.adapters(), &package.dir, &mut arguments, cancel, &mut |_| {});
    (result, arguments)
}

#[test]
fn a_configurations_cargo_build_fills_in_the_program_its_filter_picks() {
    let package = Package::new(&[("Cargo.toml", "")]);
    let build = [
        artifact("app", "lib", Path::new("/p/deps/app")),
        artifact("x", "bin", Path::new("/p/x")),
        artifact("y", "bin", Path::new("/p/y")),
    ]
    .join("\n");
    package.cargo(&[], "", &build, 0);
    let cargo = json!({"args": ["test", "--bin=x"], "filter": {"name": "x", "kind": "bin"}});
    let (result, arguments) = resolved(&package, cargo, &Cancel::new());
    assert_eq!(result, Ok(()));
    assert_eq!(arguments["program"], json!("/p/x"));
    assert!(!arguments.contains_key("cargo"));
    assert_eq!(arguments["cwd"], json!("/p"));

    let (result, _) = resolved(&package, json!({"args": ["build", "--release"]}), &Cancel::new());
    assert_eq!(
        result,
        Err(CurrentError::WhichBinary(vec![
            "/p/deps/app".into(),
            "/p/x".into(),
            "/p/y".into()
        ]))
    );
    let (result, _) = resolved(&package, json!({"filter": {"kind": "example"}}), &Cancel::new());
    assert_eq!(result, Err(CurrentError::NoProgram));
    assert_eq!(
        package.calls(),
        [
            "test --no-run --bin=x --profile ide-debug --message-format=json | dev true none",
            "build --release --message-format=json | dev true none",
            "build --profile ide-debug --message-format=json | dev true none",
        ]
    );
}

#[test]
fn a_configuration_without_cargo_needs_no_build_and_a_failed_or_cancelled_one_says_so() {
    let package = Package::new(&[("Cargo.toml", "")]);
    let mut arguments = Map::from_iter([("program".to_string(), json!("/p/x"))]);
    let none = Adapters::with_tools(&Supervisor::new(), Vec::new(), PathBuf::from("/none"));
    assert_eq!(
        resolve_cargo(&none, &package.dir, &mut arguments, &Cancel::new(), &mut |_| {}),
        Ok(())
    );
    assert_eq!(arguments["program"], json!("/p/x"));
    let mut arguments = Map::from_iter([("cargo".to_string(), json!({}))]);
    assert_eq!(
        resolve_cargo(&none, &package.dir, &mut arguments, &Cancel::new(), &mut |_| {}),
        Err(CurrentError::Missing("cargo"))
    );

    package.cargo(&[], "", &error("error: mismatched types\n"), 101);
    let (result, _) = resolved(&package, json!({}), &Cancel::new());
    assert_eq!(result, Err(CurrentError::Build("error: mismatched types\n".into())));

    script(&package.bin.join("cargo"), "sleep 30\n");
    let cancel = Cancel::new();
    cancel.cancel();
    let (result, _) = resolved(&package, json!({}), &cancel);
    assert_eq!(result, Err(CurrentError::Cancelled));
}
