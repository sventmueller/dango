//! cargo-nextest: `nextest list` as JSON; a run in its experimental libtest JSON with nextest's additions, which
//! reports each test's start and end as it happens. A test there is `<package>::<binary name>$<test>`, and the suite
//! lines name each binary's kind, which together give the binary id the listing and filters use.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::reading::Changes;
use crate::{Command, Report, Scope, State, Test, TestId, command};

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    event: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    stdout: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    nextest: Option<Suite>,
}

#[derive(Deserialize)]
struct Suite {
    #[serde(rename = "crate")]
    package: String,
    test_binary: String,
    kind: String,
}

pub(crate) fn list(root: &Path) -> Command {
    command("cargo", &["nextest", "list", "--message-format", "json"], root)
}

pub(crate) fn run(root: &Path, scope: &Scope) -> Command {
    let mut run = command(
        "cargo",
        &[
            "nextest",
            "run",
            "--no-fail-fast",
            "--message-format",
            "libtest-json-plus",
        ],
        root,
    );
    run.env.push(("NEXTEST_EXPERIMENTAL_LIBTEST_JSON".into(), "1".into()));
    match scope {
        Scope::All | Scope::File(_) => {}
        Scope::Package(package) => run.args.extend(["-E".into(), format!("binary_id({package})")]),
        Scope::Test(test) => run.args.extend([
            "-E".into(),
            format!("binary_id({}) & test(={})", test.package, test.name),
        ]),
    }
    run
}

/// The binary id nextest gives the test binary `binary` of `kind` in `package`: the package alone for its library.
pub(crate) fn binary_id(package: &str, kind: &str, binary: &str) -> String {
    match kind {
        "lib" | "proc-macro" => package.to_string(),
        "test" => format!("{package}::{binary}"),
        other => format!("{package}::{other}/{binary}"),
    }
}

pub(crate) fn tests(output: &str) -> Vec<Test> {
    let Ok(list) = serde_json::from_str::<serde_json::Value>(output) else {
        return Vec::new();
    };
    let Some(suites) = list.get("rust-suites").and_then(|s| s.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (binary, suite) in suites {
        let Some(cases) = suite.get("testcases").and_then(|c| c.as_object()) else {
            continue;
        };
        for name in cases.keys() {
            out.push(Test {
                id: TestId {
                    package: binary.clone(),
                    file: None,
                    name: name.clone(),
                },
                location: None,
            });
        }
    }
    out
}

/// A run's libtest JSON as it comes, its human output among it skipped: the kind of each test binary, and the
/// report of each test.
#[derive(Debug, Default)]
pub(crate) struct Lines {
    kinds: HashMap<(String, String), String>,
    at: HashMap<TestId, usize>,
}

impl Lines {
    pub(crate) fn line(&mut self, root: &Path, line: &str, out: &mut Vec<Report>, changes: &mut Changes) {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            return;
        };
        if line.kind == "suite" {
            if let Some(suite) = line.nextest {
                self.kinds.insert((suite.package, suite.test_binary), suite.kind);
            }
            return;
        }
        let Some((binary, name)) = line.name.as_deref().and_then(|n| n.split_once('$')) else {
            return;
        };
        let (package, binary) = binary.split_once("::").unwrap_or((binary, binary));
        let kind = self
            .kinds
            .get(&(package.to_string(), binary.to_string()))
            .map_or("lib", String::as_str);
        let state = match line.event.as_str() {
            "started" => State::Running,
            "ok" => State::Passed,
            "failed" => State::Failed,
            "ignored" => State::Skipped,
            _ => return,
        };
        let id = TestId {
            package: binary_id(package, kind, binary),
            file: None,
            name: name.to_string(),
        };
        let output = line.stdout.as_deref().unwrap_or_default();
        let message = match state {
            State::Failed => crate::cargo::panic_message(output).or_else(|| line.message.clone()),
            State::Skipped => line.message.clone(),
            _ => None,
        };
        let report = Report {
            id: id.clone(),
            state,
            message,
            failure: crate::cargo::panic_place(output, root),
            location: None,
        };
        let index = match self.at.get(&id) {
            Some(index) => {
                out[*index] = report;
                *index
            }
            None => {
                self.at.insert(id, out.len());
                out.push(report);
                out.len() - 1
            }
        };
        changes.mark(index);
    }
}
