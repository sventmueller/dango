//! `cargo test`: libtest's text, with a recording per Rust version in the tests (R29). libtest reports no starts on
//! stable, so a run shows as running as a whole. Each test program's section starts with Cargo's `Running` line, which
//! names the program the tests below belong to.

use std::path::Path;

use crate::{Command, DebugTarget, Location, Report, Scope, State, Test, TestId, command, place};

pub(crate) fn list(root: &Path) -> Command {
    command("cargo", &["test", "--", "--list"], root)
}

pub(crate) fn run(root: &Path, scope: &Scope) -> Command {
    let mut run = command("cargo", &["test", "--no-fail-fast"], root);
    match scope {
        Scope::All | Scope::File(_) => {}
        Scope::Package(package) => run.args.extend(["-p".into(), package.clone()]),
        Scope::Test(test) => run.args.extend(["--".into(), test.name.clone(), "--exact".into()]),
    }
    run
}

/// The test program a `Running` or `Doc-tests` line of Cargo names: `sample` for
/// `Running unittests src/lib.rs (…/deps/sample-2269f62bb3dee741)`.
fn program(line: &str) -> Option<String> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("Doc-tests ") {
        return Some(format!("{rest} (doc)"));
    }
    let rest = line.strip_prefix("Running ")?;
    let path = rest.rsplit_once('(')?.1.trim_end_matches(')');
    let stem = Path::new(path).file_name()?.to_str()?;
    Some(stem.rsplit_once('-').map_or(stem, |(name, _)| name).to_string())
}

pub(crate) fn tests(output: &str) -> Vec<Test> {
    let mut package = String::new();
    let mut out = Vec::new();
    for line in output.lines() {
        if let Some(found) = program(line) {
            package = found;
            continue;
        }
        if let Some(name) = line.strip_suffix(": test") {
            out.push(Test {
                id: TestId {
                    package: package.clone(),
                    file: None,
                    name: name.to_string(),
                },
                location: None,
            });
        }
    }
    out
}

pub(crate) fn reports(root: &Path, output: &str) -> Vec<Report> {
    let mut package = String::new();
    let mut out: Vec<Report> = Vec::new();
    let mut failing: Option<usize> = None;
    for line in output.lines() {
        if let Some(found) = program(line) {
            package = found;
            failing = None;
            continue;
        }
        if let Some(rest) = line.strip_prefix("test ")
            && let Some((name, outcome)) = rest.rsplit_once(" ... ")
        {
            let state = match outcome.trim() {
                "ok" => State::Passed,
                "FAILED" => State::Failed,
                o if o.starts_with("ignored") => State::Skipped,
                _ => continue,
            };
            out.push(Report {
                id: TestId {
                    package: package.clone(),
                    file: None,
                    name: name.to_string(),
                },
                state,
                message: None,
                failure: None,
                location: None,
            });
            continue;
        }
        if let Some(name) = line.strip_prefix("---- ").and_then(|l| l.strip_suffix(" stdout ----")) {
            failing = out.iter().rposition(|r| r.id.name == name && r.id.package == package);
            continue;
        }
        let Some(index) = failing else { continue };
        if let Some(at) = line.split(" panicked at ").nth(1) {
            out[index].failure = place(at, root);
            continue;
        }
        let report = &mut out[index];
        if report.failure.is_some() && !line.is_empty() && !line.starts_with("note:") {
            let message = report.message.get_or_insert_with(String::new);
            if !message.is_empty() {
                message.push('\n');
            }
            message.push_str(line);
        }
    }
    out
}

/// The test program built in the profile `ide-debug` into a folder of its own, as the debugger builds Rust, run with
/// the test's exact name.
pub(crate) fn debug(root: &Path, test: &TestId) -> DebugTarget {
    let mut build = command(
        "cargo",
        &["test", "--no-run", "--message-format=json", "--profile", "ide-debug"],
        root,
    );
    build.env.push((
        "CARGO_TARGET_DIR".into(),
        root.join("target/ide-debug").display().to_string(),
    ));
    if !test.package.is_empty() && !test.package.ends_with("(doc)") {
        build.args.extend(["--test".into(), test.package.clone()]);
    }
    DebugTarget::Native {
        build,
        args: vec![test.name.clone(), "--exact".into(), "--nocapture".into()],
    }
}

/// Where a failure's location is, kept for tools that read libtest's panics elsewhere.
pub(crate) fn panic_place(text: &str, root: &Path) -> Option<Location> {
    text.lines()
        .find_map(|line| line.split(" panicked at ").nth(1))
        .and_then(|at| place(at, root))
}
