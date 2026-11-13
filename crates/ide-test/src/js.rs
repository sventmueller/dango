//! Vitest and Jest: `vitest list --json` lists tests with their place; Jest lists files only (`--listTests`), its
//! tests come from the run. Both write the same JSON of results to a file, whose `assertionResults` name each test by
//! its describe blocks and title, joined with ` > ` as Vitest's list writes them.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Command, DebugTarget, Location, Report, Scope, State, Test, TestId, command, escaped};

#[derive(Deserialize)]
struct Listed {
    name: String,
    file: PathBuf,
    #[serde(default)]
    location: Option<Place>,
}

#[derive(Deserialize)]
struct Place {
    line: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Results {
    #[serde(default)]
    test_results: Vec<FileResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileResult {
    name: PathBuf,
    #[serde(default)]
    assertion_results: Vec<Assertion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Assertion {
    #[serde(default)]
    ancestor_titles: Vec<String>,
    title: String,
    status: String,
    #[serde(default)]
    failure_messages: Vec<String>,
    #[serde(default)]
    location: Option<Place>,
}

/// The runner's own program in the project, run with node so that its version is the project's.
fn runner(root: &Path, entry: &str) -> PathBuf {
    root.join("node_modules").join(entry)
}

pub(crate) fn vitest_list(root: &Path) -> Command {
    command("npx", &["vitest", "list", "--json"], root)
}

pub(crate) fn jest_list(root: &Path) -> Command {
    command("npx", &["jest", "--listTests"], root)
}

fn with_results(mut run: Command, flag: &str, scratch: &Path) -> Command {
    let results = scratch.join("dango-js-results.json");
    run.args.push(format!("{flag}={}", results.display()));
    run.files.push((results.clone(), String::new()));
    run.results = Some(results);
    run
}

fn filter(run: &mut Command, scope: &Scope, flag: &str) {
    match scope {
        Scope::All | Scope::Package(_) => {}
        Scope::File(file) => run.args.push(file.display().to_string()),
        Scope::Test(test) => {
            if let Some(file) = &test.file {
                run.args.push(file.display().to_string());
            }
            let title = test.name.rsplit(" > ").collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ");
            run.args.extend([flag.to_string(), format!("^{}$", escaped(&title))]);
        }
    }
}

pub(crate) fn vitest_run(root: &Path, scope: &Scope, scratch: &Path) -> Command {
    let mut run = with_results(
        command("npx", &["vitest", "run", "--reporter=json"], root),
        "--outputFile",
        scratch,
    );
    filter(&mut run, scope, "-t");
    run
}

pub(crate) fn jest_run(root: &Path, scope: &Scope, scratch: &Path) -> Command {
    let mut run = with_results(
        command("npx", &["jest", "--json", "--testLocationInResults"], root),
        "--outputFile",
        scratch,
    );
    filter(&mut run, scope, "-t");
    run
}

pub(crate) fn vitest_tests(output: &str) -> Vec<Test> {
    let Ok(listed) = serde_json::from_str::<Vec<Listed>>(output) else {
        return Vec::new();
    };
    listed
        .into_iter()
        .map(|l| Test {
            location: l.location.map(|p| Location {
                path: l.file.clone(),
                line: p.line,
            }),
            id: TestId {
                package: String::new(),
                file: Some(l.file),
                name: l.name,
            },
        })
        .collect()
}

/// Jest's listing names files only; each stands for its tests until a run names them.
pub(crate) fn jest_tests(output: &str) -> Vec<Test> {
    output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|file| Test {
            id: TestId {
                package: String::new(),
                file: Some(PathBuf::from(file.trim())),
                name: String::new(),
            },
            location: None,
        })
        .collect()
}

pub(crate) fn reports(output: &str) -> Vec<Report> {
    let Ok(results) = serde_json::from_str::<Results>(output) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for file in results.test_results {
        for assertion in file.assertion_results {
            let mut parts = assertion.ancestor_titles.clone();
            parts.push(assertion.title.clone());
            let state = match assertion.status.as_str() {
                "passed" => State::Passed,
                "failed" => State::Failed,
                "pending" | "skipped" | "todo" | "disabled" => State::Skipped,
                _ => State::NotRun,
            };
            let message = assertion.failure_messages.first().map(|m| first_lines(m));
            let failure = assertion
                .failure_messages
                .iter()
                .flat_map(|m| m.lines())
                .filter(|l| l.trim_start().starts_with("at "))
                .filter_map(|l| crate::place(l, Path::new("/")))
                .find(|place| place.path == file.name);
            out.push(Report {
                id: TestId {
                    package: String::new(),
                    file: Some(file.name.clone()),
                    name: parts.join(" > "),
                },
                state,
                message,
                failure,
                location: assertion.location.map(|p| Location {
                    path: file.name.clone(),
                    line: p.line,
                }),
            });
        }
    }
    out
}

/// The message up to its stack trace.
fn first_lines(message: &str) -> String {
    message
        .lines()
        .take_while(|l| !l.trim_start().starts_with("at "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub(crate) fn vitest_debug(root: &Path, test: &TestId) -> DebugTarget {
    let mut args = vec!["run".to_string(), "--no-file-parallelism".to_string()];
    let mut run = command("", &[], root);
    filter(&mut run, &Scope::Test(test.clone()), "-t");
    args.extend(run.args);
    DebugTarget::Node {
        program: runner(root, "vitest/vitest.mjs"),
        args,
        cwd: root.to_path_buf(),
    }
}

/// Jest comes into the debug list with this: its runner in band, the test chosen by file and name.
pub(crate) fn jest_debug(root: &Path, test: &TestId) -> DebugTarget {
    let mut args = vec!["--runInBand".to_string()];
    let mut run = command("", &[], root);
    filter(&mut run, &Scope::Test(test.clone()), "-t");
    args.extend(run.args);
    DebugTarget::Node {
        program: runner(root, "jest/bin/jest.js"),
        args,
        cwd: root.to_path_buf(),
    }
}
