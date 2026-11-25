//! `bun test`, found by a `test` script that runs bun. Where the script is `bun test` itself, Dango runs `bun test`
//! and lists the files bun's naming rule picks; where it runs a project's own runner, Dango runs the script, which
//! takes `bun test`'s flags and file paths and lists its files with `--list`. Either way the results come as JUnit
//! XML in a file, which may hold several documents one after the other as a runner finishes its parts; bun writes
//! one per invocation, at its end, so a run shows no test as running.

use std::path::Path;

use crate::reading::Changes;
use crate::{Command, Location, Report, Scope, State, Test, TestId, command, escaped};

/// How the project runs its tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Its `test` script is `bun test`.
    Direct,
    /// Its `test` script runs its own runner with bun.
    Script,
}

/// Dango's lister of the files `bun test` runs.
const LISTER: &str = include_str!("dango_bun_list.mjs");

/// The endings of the files `bun test` runs.
const ENDINGS: [&str; 4] = [".test.", "_test.", ".spec.", "_spec."];
const EXTENSIONS: [&str; 8] = ["js", "jsx", "ts", "tsx", "mjs", "cjs", "mts", "cts"];

fn mode(root: &Path) -> Option<Mode> {
    let text = std::fs::read_to_string(root.join("package.json")).ok()?;
    let package: serde_json::Value = serde_json::from_str(&text).ok()?;
    let script = package.get("scripts")?.get("test")?.as_str()?.trim();
    let mut words = script.split_whitespace();
    match (words.next(), words.next()) {
        (Some("bun"), Some("test")) => Some(Mode::Direct),
        (Some("bun"), Some(_)) => Some(Mode::Script),
        _ => None,
    }
}

/// Whether the project at `root` runs its tests with bun.
pub(crate) fn found(root: &Path) -> bool {
    mode(root).is_some()
}

/// The command with bun's own arguments, through the `test` script where it is a runner of its own.
fn bun(root: &Path, args: &[&str]) -> Command {
    match mode(root) {
        Some(Mode::Script) => {
            let mut run = command("bun", &["run", "test"], root);
            run.args.extend(args.iter().map(|a| (*a).to_string()));
            run
        }
        _ => {
            let mut run = command("bun", &["test"], root);
            run.args.extend(args.iter().map(|a| (*a).to_string()));
            run
        }
    }
}

pub(crate) fn list(root: &Path, scratch: &Path) -> Command {
    if mode(root) == Some(Mode::Script) {
        return command("bun", &["run", "test", "--list"], root);
    }
    let lister = scratch.join("dango_bun_list.mjs");
    let mut run = command("bun", &[], root);
    run.args.push(lister.display().to_string());
    run.files.push((lister, LISTER.into()));
    run
}

/// The test file a listed line names last, such as `src/a.test.ts` or a runner's `dom\tsrc/a.test.ts`.
fn listed_file(line: &str) -> Option<&str> {
    let file = line.split_whitespace().last()?;
    is_test_file(file.rsplit('/').next()?).then_some(file)
}

/// Whether the file `name` is one the JavaScript runners take for a test file, such as `a.test.ts`.
pub(crate) fn is_test_file(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| EXTENSIONS.contains(&extension) && ENDINGS.iter().any(|e| name.contains(e)))
}

/// The listing names files only; each stands for its tests until a run names them.
pub(crate) fn tests(root: &Path, output: &str) -> Vec<Test> {
    output
        .lines()
        .filter_map(listed_file)
        .map(|file| Test {
            id: TestId {
                package: String::new(),
                file: Some(root.join(file)),
                name: String::new(),
            },
            location: None,
        })
        .collect()
}

pub(crate) fn run(root: &Path, scope: &Scope, scratch: &Path) -> Command {
    let results = scratch.join("dango-bun-junit.xml");
    let outfile = format!("--reporter-outfile={}", results.display());
    let mut run = bun(root, &["--reporter=junit", &outfile]);
    match scope {
        Scope::All | Scope::Package(_) => {}
        Scope::File(file) => run.args.push(root.join(file).display().to_string()),
        Scope::Test(test) => {
            if let Some(file) = &test.file {
                run.args.push(root.join(file).display().to_string());
            }
            let title = test.name.split(" > ").collect::<Vec<_>>().join(" ");
            run.args.extend(["-t".to_string(), format!("^{}$", escaped(&title))]);
        }
    }
    run.files.push((results.clone(), String::new()));
    run.results = Some(results);
    run
}

/// The JUnit documents of a run as they come, one element a line as bun writes them; a test is named by the
/// describe blocks it is in and its title, joined with ` > `.
#[derive(Debug, Default)]
pub(crate) struct Lines {
    /// The names of the open test suites; the outermost of each document is the file.
    suites: Vec<String>,
    /// The test case still open, waiting for its `</testcase>`.
    open: Option<Report>,
}

impl Lines {
    pub(crate) fn line(&mut self, root: &Path, line: &str, out: &mut Vec<Report>, changes: &mut Changes) {
        let line = line.trim();
        if line.starts_with("<testsuites") || line.starts_with("</testsuites") {
            self.suites.clear();
        } else if line.starts_with("<testsuite") {
            if !line.ends_with("/>") {
                self.suites.push(attribute(line, "name").unwrap_or_default());
            }
        } else if line.starts_with("</testsuite") {
            self.suites.pop();
        } else if line.starts_with("<testcase") {
            let report = self.case(root, line);
            if line.ends_with("/>") {
                push(report, out, changes);
            } else {
                self.open = Some(report);
            }
        } else if line.starts_with("</testcase") {
            if let Some(report) = self.open.take() {
                push(report, out, changes);
            }
        } else if let Some(report) = &mut self.open {
            outcome(root, report, line);
        }
    }

    fn case(&self, root: &Path, line: &str) -> Report {
        let file = attribute(line, "file").map(|f| root.join(f));
        let mut name: Vec<String> = self.suites.iter().skip(1).cloned().collect();
        name.push(attribute(line, "name").unwrap_or_default());
        Report {
            location: file.clone().zip(attribute(line, "line")).and_then(|(path, line)| {
                Some(Location {
                    path,
                    line: line.parse().ok()?,
                })
            }),
            id: TestId {
                package: String::new(),
                file,
                name: name.join(" > "),
            },
            state: State::Passed,
            message: None,
            failure: None,
        }
    }
}

/// What a child of an open test case says of it: a failure with its message and the place it names in the test's
/// file, or a skip.
fn outcome(root: &Path, report: &mut Report, line: &str) {
    if line.starts_with("<skipped") {
        report.state = State::Skipped;
        report.message = attribute(line, "message");
    } else if line.starts_with("<failure") || line.starts_with("<error") {
        report.state = State::Failed;
        let body = line
            .split_once('>')
            .map(|(_, rest)| unescape(rest.rsplit_once("</").map_or(rest, |(text, _)| text)))
            .unwrap_or_default();
        report.message = attribute(line, "message").or_else(|| Some(body.clone()));
        report.failure = body
            .lines()
            .filter(|l| l.trim_start().starts_with("at "))
            .filter_map(|l| crate::place(l, root))
            .find(|place| report.id.file.as_ref() == Some(&place.path));
    }
}

fn push(report: Report, out: &mut Vec<Report>, changes: &mut Changes) {
    out.push(report);
    changes.mark(out.len() - 1);
}

/// The value of the attribute `name` in the tag `line`, its entities resolved.
fn attribute(line: &str, name: &str) -> Option<String> {
    let start = line.find(&format!(" {name}=\""))? + name.len() + 3;
    let length = line[start..].find('"')?;
    Some(unescape(&line[start..start + length]))
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';') else { break };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| entity.strip_prefix('#').map(str::parse))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
