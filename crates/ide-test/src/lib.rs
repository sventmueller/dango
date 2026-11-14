//! Finding, running and debugging tests (E110): one adapter per tool — `cargo test`, cargo-nextest, Go, pytest,
//! Vitest and Jest — that says which commands to run and reads what they print. A test's identity (package, file,
//! name with its parameters) is kept apart from where it is in the source, which a test may not have. Only stable
//! outputs are read, but for nextest's libtest JSON, which nextest still calls experimental; the crate starts no
//! processes itself, the app runs the commands through its supervisor.
//!
//! ```
//! use ide_test::{Scope, Tool};
//!
//! let root = std::path::Path::new("/tmp/project");
//! let run = Tool::Go.run(root, &Scope::All, None);
//! assert_eq!(run.program, "go");
//! assert!(run.args.contains(&"-json".to_string()));
//! let reports = Tool::Go.reports(root, r#"{"Action":"pass","Package":"example.com/m","Test":"TestA"}"#);
//! assert_eq!(reports[0].id.name, "TestA");
//! ```

#![warn(missing_docs, unreachable_pub)]

pub mod board;
mod cargo;
mod go;
mod js;
mod nextest;
mod pytest;
mod source;

use std::path::{Path, PathBuf};

/// A tool that runs tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    /// `cargo test`, read from libtest's text.
    Cargo,
    /// cargo-nextest, read from its libtest JSON as it goes.
    Nextest,
    /// `go test -json`.
    Go,
    /// pytest with Dango's plugin.
    Pytest,
    /// Vitest with Dango's reporter.
    Vitest,
    /// Jest with Dango's reporter.
    Jest,
}

/// Who a test is, apart from where its source is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TestId {
    /// The package, crate target or Go package it belongs to.
    pub package: String,
    /// The file it is in, where the tool says so.
    pub file: Option<PathBuf>,
    /// Its name as the tool writes it, parameters included, such as `tests::adds`, `TestTable/case_one`,
    /// `test_param[2]` or `math > adds`.
    pub name: String,
}

/// Where something is in the source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    /// The file.
    pub path: PathBuf,
    /// The line, counted from 1.
    pub line: u32,
}

/// A test the tool lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Test {
    /// Who it is.
    pub id: TestId,
    /// Where it is, if the tool says.
    pub location: Option<Location>,
}

/// What became of a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// It did not run yet.
    NotRun,
    /// It runs; only tools that report starts say so.
    Running,
    /// It passed.
    Passed,
    /// It failed.
    Failed,
    /// It was skipped or ignored.
    Skipped,
}

/// One test's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Which test.
    pub id: TestId,
    /// What became of it.
    pub state: State,
    /// The failure's or skip's message.
    pub message: Option<String>,
    /// Where the failing assert is, for a mark at its line.
    pub failure: Option<Location>,
    /// Where the test is, where the run says so.
    pub location: Option<Location>,
}

/// What to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Every test.
    All,
    /// The tests of a package.
    Package(String),
    /// The tests of a file.
    File(PathBuf),
    /// One test.
    Test(TestId),
}

/// A command to start, in a folder, with variables beside the login shell's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// The program, looked for on the PATH where it is no path.
    pub program: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// Where it runs.
    pub cwd: PathBuf,
    /// Variables it gets beside the environment.
    pub env: Vec<(String, String)>,
    /// A file the tool writes its results to, which [`Tool::reports`] reads in place of the output.
    pub results: Option<PathBuf>,
    /// Files to write before the command starts, such as Dango's pytest plugin or nextest's tool configuration.
    pub files: Vec<(PathBuf, String)>,
}

/// How to debug one test, with the same runtime, folder and environment as a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugTarget {
    /// A Rust test program, built with `build` (whose JSON names the executable), run with `args`.
    Native {
        /// The command that builds the test program.
        build: Command,
        /// The test program's arguments.
        args: Vec<String>,
    },
    /// A Go package in Delve's test mode with `args`.
    Go {
        /// The package's folder.
        package: PathBuf,
        /// The test binary's arguments.
        args: Vec<String>,
    },
    /// A Python module through debugpy, such as `pytest` with the test's node id.
    PythonModule {
        /// The module.
        module: String,
        /// Its arguments.
        args: Vec<String>,
        /// Where it runs.
        cwd: PathBuf,
    },
    /// A Node program through js-debug: the test runner's entry with its filter.
    Node {
        /// The runner's resolved entry file.
        program: PathBuf,
        /// Its arguments.
        args: Vec<String>,
        /// Where it runs.
        cwd: PathBuf,
    },
}

/// Dango's pytest plugin, which reports collection, starts and results as JSON lines.
pub const PYTEST_PLUGIN: &str = include_str!("dango_pytest.py");

/// The tools whose tests the project at `root` has, the ones found on `path` taken into account: cargo-nextest
/// in place of `cargo test` where it is installed.
pub fn tools(root: &Path, path: Option<&str>) -> Vec<Tool> {
    let on_path = |name: &str| path.is_some_and(|p| p.split(':').any(|d| Path::new(d).join(name).is_file()));
    let mut out = Vec::new();
    if root.join("Cargo.toml").is_file() {
        out.push(if on_path("cargo-nextest") {
            Tool::Nextest
        } else {
            Tool::Cargo
        });
    }
    if root.join("go.mod").is_file() {
        out.push(Tool::Go);
    }
    if ["pytest.ini", "conftest.py", "pyproject.toml", "setup.cfg", "tox.ini"]
        .iter()
        .any(|f| root.join(f).is_file())
    {
        out.push(Tool::Pytest);
    }
    let package = std::fs::read_to_string(root.join("package.json")).unwrap_or_default();
    if package.contains("\"vitest\"") {
        out.push(Tool::Vitest);
    }
    if package.contains("\"jest\"") {
        out.push(Tool::Jest);
    }
    out
}

impl Tool {
    /// The name users read.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Cargo => "cargo test",
            Tool::Nextest => "cargo-nextest",
            Tool::Go => "go test",
            Tool::Pytest => "pytest",
            Tool::Vitest => "Vitest",
            Tool::Jest => "Jest",
        }
    }

    /// Whether a run reports each test as it starts; without, the panel shows the whole run as running. Every
    /// tool's output reads as the reports of the tests that ended so far.
    pub fn reports_starts(self) -> bool {
        !matches!(self, Tool::Cargo)
    }

    /// Whether listing builds the project, which `cargo test -- --list` and `cargo nextest list` do for every test
    /// program while holding Cargo's lock, so that the panel lists on request only and finds the tests in the
    /// sources until then ([`Tool::from_source`]).
    pub fn lists_by_building(self) -> bool {
        matches!(self, Tool::Cargo | Tool::Nextest)
    }

    /// The tests the sources of the project at `root` show for a tool that lists by building, read without
    /// building: a stand-in until a listing or a run names them.
    pub fn from_source(self, root: &Path) -> Vec<Test> {
        match self {
            Tool::Cargo | Tool::Nextest => source::tests(self, root),
            Tool::Go | Tool::Pytest | Tool::Vitest | Tool::Jest => Vec::new(),
        }
    }

    /// The command that lists the tests of the project at `root`; `scratch` is a folder for files the command needs.
    pub fn list(self, root: &Path, scratch: &Path) -> Command {
        match self {
            Tool::Cargo => cargo::list(root),
            Tool::Nextest => nextest::list(root),
            Tool::Go => go::list(root),
            Tool::Pytest => pytest::list(root, scratch),
            Tool::Vitest => js::vitest_list(root),
            Tool::Jest => js::jest_list(root),
        }
    }

    /// The tests the listing `output` names; for pytest the contents of the results file.
    pub fn tests(self, root: &Path, output: &str) -> Vec<Test> {
        match self {
            Tool::Cargo => cargo::tests(output),
            Tool::Nextest => nextest::tests(output),
            Tool::Go => go::tests(root, output),
            Tool::Pytest => pytest::tests(root, output),
            Tool::Vitest => js::vitest_tests(output),
            Tool::Jest => js::jest_tests(output),
        }
    }

    /// The command that runs `scope` of the project at `root`; `scratch` is a folder for files the command needs.
    pub fn run(self, root: &Path, scope: &Scope, scratch: Option<&Path>) -> Command {
        let scratch = scratch.map_or_else(std::env::temp_dir, Path::to_path_buf);
        match self {
            Tool::Cargo => cargo::run(root, scope),
            Tool::Nextest => nextest::run(root, scope),
            Tool::Go => go::run(root, scope),
            Tool::Pytest => pytest::run(root, scope, &scratch),
            Tool::Vitest => js::vitest_run(root, scope, &scratch),
            Tool::Jest => js::jest_run(root, scope, &scratch),
        }
    }

    /// The outcomes in what a run printed, or in its results file where [`Command::results`] names one.
    pub fn reports(self, root: &Path, output: &str) -> Vec<Report> {
        match self {
            Tool::Cargo => cargo::reports(root, output),
            Tool::Nextest => nextest::reports(root, output),
            Tool::Go => go::reports(root, output),
            Tool::Pytest => pytest::reports(root, output),
            Tool::Vitest | Tool::Jest => js::reports(output),
        }
    }

    /// How to debug `test` of the project at `root`.
    pub fn debug(self, root: &Path, test: &TestId) -> DebugTarget {
        match self {
            Tool::Cargo | Tool::Nextest => cargo::debug(root, test),
            Tool::Go => go::debug(root, test),
            Tool::Pytest => pytest::debug(root, test),
            Tool::Vitest => js::vitest_debug(root, test),
            Tool::Jest => js::jest_debug(root, test),
        }
    }
}

/// `program` with `args` in `cwd`, without variables or a results file.
fn command(program: &str, args: &[&str], cwd: &Path) -> Command {
    Command {
        program: program.to_string(),
        args: args.iter().map(|a| (*a).to_string()).collect(),
        cwd: cwd.to_path_buf(),
        env: Vec::new(),
        results: None,
        files: Vec::new(),
    }
}

/// `"file:line"` or `"file:line:column"` at the end of `text`, as panics and stack frames name a place.
fn place(text: &str, root: &Path) -> Option<Location> {
    let text = text.trim().trim_end_matches(')').trim_end_matches(':');
    let mut parts = text.rsplitn(3, ':');
    let last = parts.next()?;
    let middle = parts.next()?;
    let (file, line) = match (middle.parse::<u32>(), parts.next()) {
        (Ok(line), Some(file)) => (file, line),
        _ => (middle, last.parse().ok()?),
    };
    let file = file.rsplit(['(', ' ']).next().unwrap_or(file);
    if file.is_empty() {
        return None;
    }
    let path = Path::new(file);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    Some(Location { path, line })
}

/// The regular-expression form of `text`, for a filter that must match one name exactly.
fn escaped(text: &str) -> String {
    text.chars()
        .map(|c| {
            if "\\.+*?()|[]{}^$".contains(c) {
                format!("\\{c}")
            } else {
                c.to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
