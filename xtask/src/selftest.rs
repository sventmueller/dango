//! Runs the scenarios the app carries with the `selftest` feature and judges each by the report the app writes.

use std::io::{BufRead as _, BufReader};
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ide_product::BUNDLE_ID;
use serde::Deserialize;

use crate::*;

pub(crate) const DEFAULT_SCENARIO: &str = "probe";

/// The argument of `cargo xtask selftest` that stands for every scenario the app marks offline.
pub(crate) const OFFLINE: &str = "--offline";

/// The argument that makes the app print its scenario table, one JSON object a line.
const LIST_SCENARIOS: &str = "--list-scenarios";

/// One scenario as the app's table lists it (`src/selftest/scenarios.rs`).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Listed {
    pub(crate) name: String,
    limit_secs: u64,
    pub(crate) offline: bool,
    launches: Launches,
    arguments: Vec<String>,
    dictation: bool,
}

/// How a scenario's launches end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Launches {
    Once,
    /// A first launch quits as the user does, and a second one in the same home checks what came back.
    Quit,
    /// A first launch kills itself with `SIGKILL`, and a second one in the same home checks what survived.
    Crash,
}

/// The scenario table the app at `program` prints.
fn list(program: &Path) -> Result<Vec<Listed>, String> {
    let out = Command::new(program)
        .arg(LIST_SCENARIOS)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    if !out.status.success() {
        return Err(format!("{} {LIST_SCENARIOS} failed: {}", program.display(), out.status));
    }
    parse_listing(&String::from_utf8_lossy(&out.stdout))
}

pub(crate) fn parse_listing(text: &str) -> Result<Vec<Listed>, String> {
    let listed: Vec<Listed> = text
        .lines()
        .filter(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).map_err(|e| format!("the scenario table is unreadable: {e}: {line}")))
        .collect::<Result<_, _>>()?;
    if listed.is_empty() {
        return Err("the app lists no scenarios".into());
    }
    Ok(listed)
}

/// The scenarios `asked` names out of `table`, with `OFFLINE` standing for every offline one.
pub(crate) fn resolve(asked: &[String], table: &[Listed]) -> Result<Vec<Listed>, String> {
    let mut chosen: Vec<Listed> = Vec::new();
    for name in asked {
        let found: Vec<&Listed> = if name == OFFLINE {
            table.iter().filter(|s| s.offline).collect()
        } else {
            vec![
                table
                    .iter()
                    .find(|s| &s.name == name)
                    .ok_or_else(|| format!("the app has no scenario {name:?}"))?,
            ]
        };
        for scenario in found {
            if !chosen.iter().any(|s| s.name == scenario.name) {
                chosen.push(scenario.clone());
            }
        }
    }
    Ok(chosen)
}

/// The signal a crashing launch ends with, which neither runs any of the app's code nor asks macOS for a crash report.
pub(crate) const SIGKILL: i32 = 9;

/// How UIFoundation's line on a failed TextKit 2 assertion begins; the app carries on after it, so only its error
/// output shows that the text system's state went wrong.
const TEXTKIT_ASSERTION: &str = "Make a symbolic breakpoint at NSText";

/// What the app writes to `IDE_SELFTEST_REPORT` at the end of a scenario.
#[derive(Debug, Deserialize)]
pub(crate) struct Report {
    pub(crate) scenario: String,
    pub(crate) checks: Vec<Check>,
    /// `light` or `dark`, which the window pictures were taken in.
    #[serde(default)]
    pub(crate) appearance: Option<String>,
}

/// One thing a scenario checked.
#[derive(Debug, Deserialize)]
pub(crate) struct Check {
    pub(crate) name: String,
    pub(crate) ok: bool,
    pub(crate) detail: String,
}

/// How the app's process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ended {
    Exited(i32),
    Signalled(i32),
    /// Killed by the runner at the time limit.
    TimedOut(Duration),
}

impl Ended {
    pub(crate) fn of(status: ExitStatus) -> Ended {
        status
            .code()
            .map_or_else(|| Ended::Signalled(status.signal().unwrap_or(0)), Ended::Exited)
    }
}

/// A scenario name, which also names the report file: letters, digits, `-` and `_`, starting with a letter or digit.
pub(crate) fn is_scenario(name: &str) -> bool {
    name.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Builds the release with the selftest and runs `scenarios` one after another; the error names those that failed.
pub(crate) fn selftest(scenarios: &[String]) -> Result<(), String> {
    selftest_with(scenarios, &[])
}

/// `selftest`, with the variables `env` set for the app on top of this process's.
pub(crate) fn selftest_with(scenarios: &[String], env: &[(String, String)]) -> Result<(), String> {
    let mut app = bundle(Build::Selftest {
        release: true,
        dictation: false,
    })?;
    let chosen = resolve(scenarios, &list(&executable(&app))?)?;
    if chosen.iter().any(|s| s.dictation) {
        app = bundle(Build::Selftest {
            release: true,
            dictation: true,
        })?;
    }
    let program = executable(&app);
    let reports = target_dir().join("selftest");
    files::create_dir_all(&reports)?;
    let mut failed = Vec::new();
    for scenario in &chosen {
        let name = scenario.name.as_str();
        println!("selftest {name}");
        let outcome = run_scenario(&program, scenario, &reports.join(format!("{name}.json")), env)
            .and_then(|()| baseline::compare(name, &reports));
        match outcome {
            Ok(()) => println!("selftest {name}: passed"),
            Err(e) => {
                eprintln!("selftest {name} failed: {e}");
                failed.push(name);
            }
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("selftest failed: {}", failed.join(", ")))
    }
}

/// Runs the app on one scenario with a home of its own, twice for a crashing one, and prints the checks it reports.
fn run_scenario(program: &Path, listed: &Listed, report_file: &Path, env: &[(String, String)]) -> Result<(), String> {
    let scenario = listed.name.as_str();
    let home = fresh_home(scenario)?;
    let mut problems = Vec::new();
    if listed.launches == Launches::Quit {
        let first = report_file.with_file_name(format!("{scenario}-quit.json"));
        println!("  first launch, which quits:");
        let (quit, focus) = launch(program, listed, &first, &home, Some("quit"), env)?;
        problems.extend(
            focus
                .into_iter()
                .chain(judge(quit, Ended::Exited(0), &first, scenario))
                .map(|problem| format!("first launch: {problem}")),
        );
        if problems.is_empty() {
            println!("  second launch:");
        }
    }
    if listed.launches == Launches::Crash {
        let first = report_file.with_file_name(format!("{scenario}-crash.json"));
        println!("  first launch, which crashes:");
        let (crashed, focus) = launch(program, listed, &first, &home, Some("crash"), env)?;
        problems.extend(
            focus
                .into_iter()
                .chain(judge(crashed, Ended::Signalled(SIGKILL), &first, scenario))
                .map(|problem| format!("first launch: {problem}")),
        );
        if problems.is_empty() {
            println!("  second launch:");
        }
    }
    if problems.is_empty() {
        let (ended, focus) = launch(program, listed, report_file, &home, None, env)?;
        problems = focus
            .into_iter()
            .chain(judge(ended, Ended::Exited(0), report_file, scenario))
            .collect();
    }
    if problems.is_empty() {
        let _ = std::fs::remove_dir_all(&home);
        return Ok(());
    }
    Err(format!(
        "{}\n  report: {}\n  home, kept for a look: {}",
        problems.join("; "),
        report_file.display(),
        home.display()
    ))
}

/// Starts the app on `scenario` in `home`, telling it which `launch` this is if it is one of several, and waits for it
/// to end; with it, the problems if it took the keyboard or TextKit asserted.
fn launch(
    program: &Path,
    listed: &Listed,
    report: &Path,
    home: &Path,
    which: Option<&str>,
    env: &[(String, String)],
) -> Result<(Ended, Vec<String>), String> {
    let scenario = listed.name.as_str();
    if report.exists() {
        files::remove_file(report)?;
    }
    let mut command = Command::new(program);
    command
        .args(&listed.arguments)
        .env("IDE_SELFTEST", scenario)
        .env("IDE_SELFTEST_REPORT", report)
        .env("IDE_SELFTEST_HOME", home)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(which) = which {
        command.env("IDE_SELFTEST_LAUNCH", which);
    }
    let guard = focus::Guard::start()?;
    let mut child = command.spawn().map_err(|e| format!("{}: {e}", program.display()))?;
    guard.guard(child.id());
    let asserted = child.stderr.take().map(watch_errors);
    let ended = wait_within_limit(&mut child, Duration::from_secs(listed.limit_secs))
        .map_err(|e| format!("{}: {e}", program.display()))?;
    let mut problems: Vec<String> = guard.finish().into_iter().collect();
    // A child process the app started may hold the pipe open after the app ended.
    if let Some(Ok(line)) = asserted.map(|found| found.recv_timeout(Duration::from_secs(2))) {
        problems.push(format!("TextKit asserted: {line}"));
    }
    Ok((ended, problems))
}

/// Passes the app's error output on as it comes; the receiver gets the first line with a TextKit assertion.
fn watch_errors(stderr: ChildStderr) -> mpsc::Receiver<String> {
    let (found, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut told = false;
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            eprintln!("{line}");
            if !told && is_textkit_assertion(&line) {
                told = found.send(line).is_ok();
            }
        }
    });
    receiver
}

fn is_textkit_assertion(line: &str) -> bool {
    line.contains(TEXTKIT_ASSERTION)
}

/// Prints the checks of the report at `report` and says what failed, for a launch that should have ended as
/// `expected`.
fn judge(ended: Ended, expected: Ended, report: &Path, scenario: &str) -> Vec<String> {
    let written = std::fs::read(report).ok();
    let report = parse_report(scenario, written.as_deref());
    if let Ok(report) = &report {
        for check in &report.checks {
            println!("  {}", check_line(check));
        }
    }
    problems(ended, expected, &report)
}

/// An empty folder only the user can enter, for the app to use as its home.
fn fresh_home(scenario: &str) -> Result<PathBuf, String> {
    let home = std::env::temp_dir().join(format!("{BUNDLE_ID}.selftest-{scenario}-{}", std::process::id()));
    if home.exists() {
        files::remove_dir_all(&home)?;
    }
    files::create_private_dir(&home)?;
    Ok(home)
}

fn wait_within_limit(child: &mut Child, limit: Duration) -> std::io::Result<Ended> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Ended::of(status));
        }
        if started.elapsed() >= limit {
            child.kill()?;
            child.wait()?;
            return Ok(Ended::TimedOut(limit));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The report of `scenario` from what the app wrote, if it wrote anything. A report without checks is refused, since
/// it would pass a run that checked nothing.
pub(crate) fn parse_report(scenario: &str, written: Option<&[u8]>) -> Result<Report, String> {
    let written = written.ok_or("the app wrote no report")?;
    let report: Report = serde_json::from_slice(written).map_err(|e| format!("the report is unreadable: {e}"))?;
    if report.scenario != scenario {
        return Err(format!("the report is of scenario {:?}", report.scenario));
    }
    if report.checks.is_empty() {
        return Err("the report lists no checks".into());
    }
    Ok(report)
}

/// What failed in a run; none when the app ended by itself as `expected`, with status 0 or by the signal of a crash
/// it made itself, and every reported check held.
pub(crate) fn problems(ended: Ended, expected: Ended, report: &Result<Report, String>) -> Vec<String> {
    let mut problems = Vec::new();
    if ended != expected {
        match ended {
            Ended::Exited(0) => {}
            Ended::Exited(code) => problems.push(format!("the app exited with status {code}")),
            Ended::Signalled(signal) => problems.push(format!("the app crashed with signal {signal}")),
            Ended::TimedOut(limit) => problems.push(format!(
                "the app did not finish within {} s and was killed",
                limit.as_secs()
            )),
        }
        if expected != Ended::Exited(0) {
            problems.push(format!("the app should have ended by itself as {expected:?}"));
        }
    }
    match report {
        Ok(report) => {
            let failed = report.checks.iter().filter(|check| !check.ok).count();
            if failed > 0 {
                problems.push(format!("{failed} of {} checks failed", report.checks.len()));
            }
        }
        Err(e) => problems.push(e.clone()),
    }
    problems
}

pub(crate) fn check_line(check: &Check) -> String {
    let mark = if check.ok { '✓' } else { '✗' };
    if check.detail.is_empty() {
        format!("{mark} {}", check.name)
    } else {
        format!("{mark} {} — {}", check.name, check.detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSING: &str = r#"{"scenario": "probe", "checks": [
        {"name": "window opens", "ok": true, "detail": "1 window"},
        {"name": "settings load", "ok": true, "detail": ""}
    ]}"#;

    const FAILING: &str = r#"{"scenario": "probe", "checks": [
        {"name": "window opens", "ok": true, "detail": "1 window"},
        {"name": "settings load", "ok": false, "detail": "missing key"}
    ]}"#;

    fn judged(ended: Ended, written: Option<&str>) -> Vec<String> {
        problems(
            ended,
            Ended::Exited(0),
            &parse_report("probe", written.map(str::as_bytes)),
        )
    }

    #[test]
    fn a_run_that_ended_well_with_every_check_held_passes() {
        assert!(judged(Ended::Exited(0), Some(PASSING)).is_empty());
        let report = parse_report("probe", Some(PASSING.as_bytes())).unwrap();
        let lines: Vec<String> = report.checks.iter().map(check_line).collect();
        assert_eq!(lines, ["✓ window opens — 1 window", "✓ settings load"]);
    }

    #[test]
    fn a_failed_check_fails_the_run_and_is_marked() {
        assert_eq!(judged(Ended::Exited(0), Some(FAILING)), ["1 of 2 checks failed"]);
        let report = parse_report("probe", Some(FAILING.as_bytes())).unwrap();
        assert_eq!(check_line(&report.checks[1]), "✗ settings load — missing key");
    }

    #[test]
    fn a_missing_report_fails_the_run() {
        assert_eq!(judged(Ended::Exited(0), None), ["the app wrote no report"]);
    }

    #[test]
    fn a_malformed_report_fails_the_run() {
        for written in [
            "",
            "{\"scenario\": \"probe\", \"checks\": [",
            r#"{"scenario": "probe"}"#,
            r#"{"scenario": "probe", "checks": [{"name": "x", "ok": "yes", "detail": ""}]}"#,
            r#"{"scenario": "probe", "checks": [{"name": "x", "ok": true}]}"#,
        ] {
            let problems = judged(Ended::Exited(0), Some(written));
            assert_eq!(problems.len(), 1, "{written}");
            assert!(problems[0].starts_with("the report is unreadable: "), "{written}");
        }
        assert!(parse_report("probe", Some(b"\xff\xfe")).is_err());
    }

    #[test]
    fn a_report_of_another_scenario_or_of_no_checks_fails_the_run() {
        assert_eq!(
            judged(Ended::Exited(0), Some(&PASSING.replace("probe", "editor"))),
            ["the report is of scenario \"editor\""]
        );
        assert_eq!(
            judged(Ended::Exited(0), Some(r#"{"scenario": "probe", "checks": []}"#)),
            ["the report lists no checks"]
        );
    }

    #[test]
    fn a_crash_a_hang_or_an_error_status_fails_even_a_good_report() {
        assert_eq!(
            judged(Ended::Exited(101), Some(PASSING)),
            ["the app exited with status 101"]
        );
        assert_eq!(
            judged(Ended::Signalled(11), Some(PASSING)),
            ["the app crashed with signal 11"]
        );
        assert_eq!(
            judged(Ended::TimedOut(Duration::from_secs(120)), None),
            [
                "the app did not finish within 120 s and was killed",
                "the app wrote no report"
            ]
        );
    }

    #[test]
    fn a_launch_meant_to_crash_passes_only_by_the_signal_it_chose() {
        let report = parse_report("probe", Some(PASSING.as_bytes()));
        assert!(problems(Ended::Signalled(SIGKILL), Ended::Signalled(SIGKILL), &report).is_empty());
        assert_eq!(
            problems(Ended::Exited(0), Ended::Signalled(SIGKILL), &report),
            ["the app should have ended by itself as Signalled(9)"]
        );
        assert_eq!(
            problems(Ended::Signalled(11), Ended::Signalled(SIGKILL), &report),
            [
                "the app crashed with signal 11",
                "the app should have ended by itself as Signalled(9)"
            ]
        );
        let failing = parse_report("probe", Some(FAILING.as_bytes()));
        assert_eq!(
            problems(Ended::Signalled(SIGKILL), Ended::Signalled(SIGKILL), &failing),
            ["1 of 2 checks failed"]
        );
    }

    #[test]
    fn a_signal_counts_as_a_crash_and_an_exit_by_its_status() {
        assert_eq!(Ended::of(ExitStatus::from_raw(0)), Ended::Exited(0));
        assert_eq!(Ended::of(ExitStatus::from_raw(1 << 8)), Ended::Exited(1));
        assert_eq!(Ended::of(ExitStatus::from_raw(11)), Ended::Signalled(11));
    }

    #[test]
    fn only_a_textkit_assertion_in_the_error_output_counts() {
        assert!(is_textkit_assertion(
            "2026-11-12 23:04:52.326 Dango[33986:9957079] Make a symbolic breakpoint at \
             NSTextLayoutManagerBreakOnNilContentManager to catch this in the debugger."
        ));
        assert!(!is_textkit_assertion(
            "[SplitView] Gesture recognizer support has been disabled because NSSplitView subclass IDEDocumentArea \
             overrides either mouseDown: or mouseDragged:"
        ));
    }

    #[test]
    fn a_scenario_name_is_a_plain_word() {
        assert!(is_scenario("probe") && is_scenario("editor-typing") && is_scenario("rtl_2"));
        assert!(!is_scenario("") && !is_scenario("-x") && !is_scenario("../x") && !is_scenario("a/b"));
        assert!(!is_scenario("a b") && !is_scenario("ü"));
    }
}
