//! pytest with Dango's plugin, which writes each collected test, start and result as a JSON line with its node id.
//! The plugin lies in a scratch folder put on `PYTHONPATH`; the interpreter is the project's `.venv` where it has one.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Command, DebugTarget, Location, PYTEST_PLUGIN, Report, Scope, State, Test, TestId, command};

#[derive(Deserialize)]
struct Line {
    event: String,
    id: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// The project's own interpreter where it has a virtual environment, else `python3` of the PATH.
fn python(root: &Path) -> String {
    [".venv/bin/python", "venv/bin/python"]
        .iter()
        .map(|p| root.join(p))
        .find(|p| p.is_file())
        .map_or_else(|| "python3".into(), |p| p.display().to_string())
}

fn with_plugin(root: &Path, scratch: &Path, args: &[&str]) -> Command {
    let mut all = vec!["-m", "pytest", "-p", "dango_pytest"];
    all.extend_from_slice(args);
    let mut run = command(&python(root), &all, root);
    let events = scratch.join("dango-pytest-events.jsonl");
    run.files.push((scratch.join("dango_pytest.py"), PYTEST_PLUGIN.into()));
    run.files.push((events.clone(), String::new()));
    run.env.push(("PYTHONPATH".into(), scratch.display().to_string()));
    run.env
        .push(("DANGO_PYTEST_EVENTS".into(), events.display().to_string()));
    run.results = Some(events);
    run
}

pub(crate) fn list(root: &Path, scratch: &Path) -> Command {
    with_plugin(root, scratch, &["--collect-only", "-q"])
}

pub(crate) fn run(root: &Path, scope: &Scope, scratch: &Path) -> Command {
    let mut run = with_plugin(root, scratch, &["-q"]);
    match scope {
        Scope::All => {}
        Scope::Package(package) => run.args.push(package.clone()),
        Scope::File(file) => run.args.push(file.display().to_string()),
        Scope::Test(test) => run.args.push(test.name.clone()),
    }
    run
}

fn lines(output: &str) -> impl Iterator<Item = Line> + '_ {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Line>(line).ok())
}

/// The file of a node id such as `tests/test_a.py::test_b[1]`.
fn file_of(root: &Path, id: &str) -> PathBuf {
    root.join(id.split("::").next().unwrap_or(id))
}

pub(crate) fn tests(root: &Path, output: &str) -> Vec<Test> {
    lines(output)
        .filter(|l| l.event == "collected")
        .map(|l| {
            let file = file_of(root, &l.id);
            Test {
                location: l.line.map(|line| Location {
                    path: l.path.as_deref().map_or_else(|| file.clone(), |p| root.join(p)),
                    line,
                }),
                id: TestId {
                    package: file
                        .parent()
                        .and_then(|d| d.strip_prefix(root).ok())
                        .map(|d| d.display().to_string())
                        .unwrap_or_default(),
                    file: Some(file),
                    name: l.id,
                },
            }
        })
        .collect()
}

pub(crate) fn reports(root: &Path, output: &str) -> Vec<Report> {
    let collected = tests(root, output);
    let mut out: Vec<Report> = Vec::new();
    for line in lines(output) {
        if line.event == "collected" {
            continue;
        }
        let known = collected.iter().find(|t| t.id.name == line.id);
        let index = out.iter().position(|r| r.id.name == line.id).unwrap_or_else(|| {
            let file = file_of(root, &line.id);
            out.push(Report {
                id: known.map(|t| t.id.clone()).unwrap_or(TestId {
                    package: String::new(),
                    file: Some(file),
                    name: line.id.clone(),
                }),
                state: State::NotRun,
                message: None,
                failure: None,
                location: known.and_then(|t| t.location.clone()),
            });
            out.len() - 1
        });
        let report = &mut out[index];
        report.state = match (line.event.as_str(), line.outcome.as_deref()) {
            ("started", _) => State::Running,
            (_, Some("passed")) => State::Passed,
            (_, Some("failed")) => State::Failed,
            (_, Some("skipped")) => State::Skipped,
            _ => report.state,
        };
        if line.message.is_some() {
            report.message = line.message;
        }
        if let (Some(path), Some(number)) = (line.path, line.line) {
            report.failure = Some(Location {
                path: root.join(path),
                line: number,
            });
        }
    }
    out
}

/// pytest as a module through debugpy, on the test's node id.
pub(crate) fn debug(root: &Path, test: &TestId) -> DebugTarget {
    DebugTarget::PythonModule {
        module: "pytest".into(),
        args: vec![test.name.clone()],
        cwd: root.to_path_buf(),
    }
}
