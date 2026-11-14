//! Vitest and Jest: `vitest list --json` lists tests with their place; Jest lists files only (`--listTests`), its
//! tests come from the run. A run loads Dango's reporter beside the tool's own, which writes each test's start and
//! result as a JSON line as it happens, naming a test by its describe blocks and title joined with ` > ` as Vitest's
//! list writes them.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Command, DebugTarget, Location, Report, Scope, State, Test, TestId, command, escaped};

/// Dango's Vitest reporter.
const VITEST_REPORTER: &str = include_str!("dango_vitest.mjs");

/// Dango's Jest reporter.
const JEST_REPORTER: &str = include_str!("dango_jest.cjs");

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
struct Event {
    event: String,
    file: PathBuf,
    name: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(default)]
    text: Option<String>,
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

/// The run with Dango's reporter `file` holding `source` named last by `flag`, which writes its events to a file in
/// `scratch`; Jest would take test files after its flag as further reporters.
fn with_reporter(mut run: Command, flag: &str, file: &str, source: &str, scratch: &Path) -> Command {
    let reporter = scratch.join(file);
    let events = scratch.join("dango-js-events.jsonl");
    run.args.push(format!("{flag}={}", reporter.display()));
    run.files.push((reporter, source.into()));
    run.files.push((events.clone(), String::new()));
    run.env.push(("DANGO_JS_EVENTS".into(), events.display().to_string()));
    run.results = Some(events);
    run
}

/// Narrows `run` to `scope`; Vitest matches a name filter against the name with ` > ` between its parts, Jest
/// against the parts joined with spaces.
fn filter(run: &mut Command, scope: &Scope, joined: &str) {
    match scope {
        Scope::All | Scope::Package(_) => {}
        Scope::File(file) => run.args.push(file.display().to_string()),
        Scope::Test(test) => {
            if let Some(file) = &test.file {
                run.args.push(file.display().to_string());
            }
            let title = test.name.split(" > ").collect::<Vec<_>>().join(joined);
            run.args.extend(["-t".to_string(), format!("^{}$", escaped(&title))]);
        }
    }
}

pub(crate) fn vitest_run(root: &Path, scope: &Scope, scratch: &Path) -> Command {
    let mut run = command("npx", &["vitest", "run", "--reporter=default"], root);
    filter(&mut run, scope, " > ");
    with_reporter(run, "--reporter", "dango_vitest.mjs", VITEST_REPORTER, scratch)
}

pub(crate) fn jest_run(root: &Path, scope: &Scope, scratch: &Path) -> Command {
    let mut run = command("npx", &["jest", "--testLocationInResults"], root);
    filter(&mut run, scope, " ");
    run.args.push("--reporters=default".into());
    with_reporter(run, "--reporters", "dango_jest.cjs", JEST_REPORTER, scratch)
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

/// The reports in the events of Dango's reporters, the last event of a test giving its state.
pub(crate) fn reports(output: &str) -> Vec<Report> {
    let mut out: Vec<Report> = Vec::new();
    for event in output.lines().filter_map(|l| serde_json::from_str::<Event>(l).ok()) {
        let state = match (event.event.as_str(), event.outcome.as_deref()) {
            ("started", _) => State::Running,
            (_, Some("passed")) => State::Passed,
            (_, Some("failed")) => State::Failed,
            (_, Some("skipped")) => State::Skipped,
            _ => continue,
        };
        let failure = event.text.as_deref().and_then(|text| {
            text.lines()
                .filter(|l| l.trim_start().starts_with("at "))
                .filter_map(|l| crate::place(l, Path::new("/")))
                .find(|place| place.path == event.file)
        });
        let report = Report {
            location: event.line.map(|line| Location {
                path: event.file.clone(),
                line,
            }),
            id: TestId {
                package: String::new(),
                file: Some(event.file),
                name: event.name,
            },
            state,
            message: event.text.as_deref().map(first_lines),
            failure,
        };
        match out.iter_mut().find(|r| r.id == report.id) {
            Some(known) => *known = report,
            None => out.push(report),
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
    filter(&mut run, &Scope::Test(test.clone()), " > ");
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
    filter(&mut run, &Scope::Test(test.clone()), " ");
    args.extend(run.args);
    DebugTarget::Node {
        program: runner(root, "jest/bin/jest.js"),
        args,
        cwd: root.to_path_buf(),
    }
}
