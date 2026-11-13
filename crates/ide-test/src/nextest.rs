//! cargo-nextest: `nextest list` as JSON, results from a JUnit report that a tool configuration of Dango's asks for
//! (`--tool-config-file`), which leaves the repository's own configuration in place. The report lies under
//! `target/nextest/dango/`; the app checks that path before the build, as the target folder may be elsewhere.

use std::path::Path;

use crate::{Command, Report, Scope, State, Test, TestId, command};

/// The tool configuration with Dango's profile, which writes the JUnit report.
pub(crate) const CONFIG: &str = "[profile.dango]\nfail-fast = false\n\n[profile.dango.junit]\npath = \"junit.xml\"\n";

pub(crate) fn list(root: &Path) -> Command {
    command("cargo", &["nextest", "list", "--message-format", "json"], root)
}

pub(crate) fn run(root: &Path, scope: &Scope, scratch: &Path) -> Command {
    let config = scratch.join("dango-nextest.toml");
    let mut run = command("cargo", &["nextest", "run", "--profile", "dango", "--no-fail-fast"], root);
    run.args.insert(2, format!("dango:{}", config.display()));
    run.args.insert(2, "--tool-config-file".into());
    run.files.push((config, CONFIG.into()));
    run.results = Some(root.join("target/nextest/dango/junit.xml"));
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

/// The outcomes in a JUnit report: a `testsuite` per test program, a `testcase` per test with a `failure` or
/// `skipped` inside where it did not pass.
pub(crate) fn reports(root: &Path, junit: &str) -> Vec<Report> {
    let mut out = Vec::new();
    let mut suite = String::new();
    let mut rest = junit;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[1..end];
        if tag.starts_with("testsuite ") {
            suite = attribute(tag, "name").unwrap_or_default();
        } else if tag.starts_with("testcase ") {
            let name = attribute(tag, "name").unwrap_or_default();
            let closed = tag.ends_with('/');
            let body = if closed {
                ""
            } else {
                let close = rest.find("</testcase>").unwrap_or(rest.len());
                &rest[end..close]
            };
            let (state, message) = if body.contains("<failure") || body.contains("<error") {
                let text = body
                    .split_once("<failure")
                    .or_else(|| body.split_once("<error"))
                    .and_then(|(_, b)| b.split_once('>'))
                    .map(|(_, b)| unescape(b.split("</").next().unwrap_or_default()))
                    .unwrap_or_default();
                (State::Failed, Some(text))
            } else if body.contains("<skipped") {
                (State::Skipped, None)
            } else {
                (State::Passed, None)
            };
            let failure = message.as_deref().and_then(|m| crate::cargo::panic_place(m, root));
            out.push(Report {
                id: TestId {
                    package: suite.clone(),
                    file: None,
                    name,
                },
                state,
                message,
                failure,
                location: None,
            });
        }
        rest = &rest[end.min(rest.len())..];
        if rest.len() <= 1 {
            break;
        }
        rest = &rest[1..];
    }
    out
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = tag[start..].find('"')? + start;
    Some(unescape(&tag[start..end]))
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#10;", "\n")
        .replace("&amp;", "&")
}
