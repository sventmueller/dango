//! `go test -json`: a stream of events with starts, outcomes and output per test. A listing names top-level tests
//! only; subtests such as `TestTable/case_one` come from the run. With `-fullpath` the output names a failure's file
//! by module path, which `go.mod`'s `module` line turns into a file of the project.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Command, DebugTarget, Location, Report, Scope, State, Test, TestId, command, escaped};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Event {
    action: String,
    #[serde(default)]
    package: String,
    #[serde(default)]
    test: Option<String>,
    #[serde(default)]
    output: Option<String>,
}

pub(crate) fn list(root: &Path) -> Command {
    command("go", &["test", "-json", "-list", ".", "./..."], root)
}

pub(crate) fn run(root: &Path, scope: &Scope) -> Command {
    let mut run = command("go", &["test", "-json", "-fullpath"], root);
    match scope {
        Scope::All => run.args.push("./...".into()),
        Scope::Package(package) => run.args.push(package.clone()),
        Scope::File(file) => run.args.push(package_dir(root, file)),
        Scope::Test(test) => {
            run.args.extend(["-run".into(), format!("^{}$", path_pattern(&test.name))]);
            run.args.push(test.package.clone());
        }
    }
    run
}

/// A `-run` pattern for `name`, each part of a subtest's path matched exactly.
fn path_pattern(name: &str) -> String {
    name.split('/').map(escaped).collect::<Vec<_>>().join("$/^")
}

fn package_dir(root: &Path, file: &Path) -> String {
    let dir = file.parent().unwrap_or(root);
    match dir.strip_prefix(root) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".into(),
        Ok(relative) => format!("./{}", relative.display()),
        Err(_) => dir.display().to_string(),
    }
}

fn events(output: &str) -> impl Iterator<Item = Event> + '_ {
    output.lines().filter_map(|line| serde_json::from_str::<Event>(line).ok())
}

pub(crate) fn tests(_root: &Path, output: &str) -> Vec<Test> {
    events(output)
        .filter(|e| e.action == "output" && e.test.is_none())
        .filter_map(|e| {
            let name = e.output?.trim().to_string();
            (name.starts_with("Test") || name.starts_with("Example") || name.starts_with("Fuzz")).then(|| Test {
                id: TestId {
                    package: e.package.clone(),
                    file: None,
                    name,
                },
                location: None,
            })
        })
        .collect()
}

/// The module path `go.mod` of `root` declares.
fn module(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("go.mod")).ok()?;
    text.lines()
        .find_map(|line| line.trim().strip_prefix("module "))
        .map(|m| m.trim().to_string())
}

/// The file of the project a `-fullpath` output names by module path, such as `example.com/m/a_test.go:12`.
fn file_of(root: &Path, module: Option<&str>, named: &str) -> PathBuf {
    match module.and_then(|m| named.strip_prefix(m)) {
        Some(rest) => root.join(rest.trim_start_matches('/')),
        None if Path::new(named).is_absolute() => PathBuf::from(named),
        None => root.join(named),
    }
}

pub(crate) fn reports(root: &Path, output: &str) -> Vec<Report> {
    let module = module(root);
    let mut out: Vec<Report> = Vec::new();
    for event in events(output) {
        let Some(name) = event.test.clone() else { continue };
        let index = out.iter().position(|r| r.id.name == name && r.id.package == event.package);
        let index = index.unwrap_or_else(|| {
            out.push(Report {
                id: TestId {
                    package: event.package.clone(),
                    file: None,
                    name,
                },
                state: State::NotRun,
                message: None,
                failure: None,
                location: None,
            });
            out.len() - 1
        });
        let report = &mut out[index];
        match event.action.as_str() {
            "run" => report.state = State::Running,
            "pass" => report.state = State::Passed,
            "fail" => report.state = State::Failed,
            "skip" => report.state = State::Skipped,
            "output" => {
                let line = event.output.unwrap_or_default();
                let trimmed = line.trim();
                if trimmed.starts_with("=== ") || trimmed.starts_with("--- ") || trimmed.is_empty() {
                    continue;
                }
                if let Some((place, message)) = trimmed.split_once(": ")
                    && let Some((file, number)) = place.rsplit_once(':')
                    && let Ok(line) = number.parse::<u32>()
                    && file.ends_with(".go")
                {
                    let path = file_of(root, module.as_deref(), file);
                    report.failure.get_or_insert(Location { path, line });
                    push(&mut report.message, message);
                } else {
                    push(&mut report.message, trimmed);
                }
            }
            _ => {}
        }
    }
    for report in &mut out {
        if report.state != State::Failed && report.state != State::Skipped {
            report.failure = None;
            report.message = None;
        }
        if report.state == State::Skipped {
            report.failure = None;
        }
    }
    out
}

fn push(message: &mut Option<String>, line: &str) {
    let text = message.get_or_insert_with(String::new);
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(line);
}

/// Delve's test mode on the package's folder, the test chosen with `-test.run`, special characters masked.
pub(crate) fn debug(root: &Path, test: &TestId) -> DebugTarget {
    let module = module(root);
    let package = match module.as_deref().and_then(|m| test.package.strip_prefix(m)) {
        Some(rest) => root.join(rest.trim_start_matches('/')),
        None => root.to_path_buf(),
    };
    DebugTarget::Go {
        package,
        args: vec!["-test.run".into(), format!("^{}$", path_pattern(&test.name))],
    }
}
