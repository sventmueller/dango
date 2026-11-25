//! Runs the scenarios the app carries with the `selftest` feature and judges each by the report the app writes.

use std::os::unix::process::ExitStatusExt as _;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;

use ide_product::BUNDLE_ID;
use serde::Deserialize;

use crate::*;

mod run;

pub(crate) const DEFAULT_SCENARIO: &str = "probe";

/// The argument of `cargo xtask selftest` that stands for every scenario the app marks offline.
pub(crate) const OFFLINE: &str = "--offline";

/// The argument of `cargo xtask selftest` that leaves out the scenario named after it.
pub(crate) const EXCEPT: &str = "--except";

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
    /// Measures time, so it runs while no other scenario does.
    #[serde(default)]
    alone: bool,
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

/// The scenarios `asked` names out of `table`, with `OFFLINE` standing for every offline one and the name after
/// `EXCEPT` left out.
pub(crate) fn resolve(asked: &[String], table: &[Listed]) -> Result<Vec<Listed>, String> {
    let find = |name: &String| {
        table
            .iter()
            .find(|s| &s.name == name)
            .ok_or_else(|| format!("the app has no scenario {name:?}"))
    };
    let mut chosen: Vec<Listed> = Vec::new();
    let mut left_out: Vec<&str> = Vec::new();
    let mut names = asked.iter();
    while let Some(name) = names.next() {
        let found: Vec<&Listed> = if name == OFFLINE {
            table.iter().filter(|s| s.offline).collect()
        } else if name == EXCEPT {
            let name = names
                .next()
                .ok_or_else(|| format!("{EXCEPT} needs a scenario after it"))?;
            left_out.push(&find(name)?.name);
            continue;
        } else {
            vec![find(name)?]
        };
        for scenario in found {
            if !chosen.iter().any(|s| s.name == scenario.name) {
                chosen.push(scenario.clone());
            }
        }
    }
    chosen.retain(|s| !left_out.contains(&s.name.as_str()));
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
    pub(crate) ending: Ending,
    pub(crate) checks: Vec<Check>,
    /// `light` or `dark`, which the window pictures were taken in, with `-dictation` from a build with dictation.
    #[serde(default)]
    pub(crate) appearance: Option<String>,
}

/// How a scenario ended, as `finish`, `skip` and `give_up` in `src/selftest.rs` tell.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Ending {
    /// It ran to its end.
    Ran,
    /// It ended before its end on purpose, for the reason given.
    Skipped(String),
    /// It ended before its end because something it needed was missing, which a failed check names.
    GaveUp(String),
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

/// Builds the release with the selftest and runs `scenarios` on virtual displays; the error names those that failed.
pub(crate) fn selftest(scenarios: &[String]) -> Result<(), String> {
    selftest_with(scenarios, &[])
}

/// `selftest`, with the variables `env` set for the app on top of this process's.
pub(crate) fn selftest_with(scenarios: &[String], env: &[(String, String)]) -> Result<(), String> {
    // The offline scenarios include dictation, so building without it first would only serve the listing.
    let dictation = scenarios.iter().any(|name| name == OFFLINE);
    let mut app = bundle(Build::Selftest {
        release: true,
        dictation,
    })?;
    let chosen = resolve(scenarios, &list(&executable(&app))?)?;
    if !dictation && chosen.iter().any(|s| s.dictation) {
        app = bundle(Build::Selftest {
            release: true,
            dictation: true,
        })?;
    }
    let program = executable(&app);
    let reports = target_dir().join("selftest");
    files::create_dir_all(&reports)?;
    let failed = run::run_all(&program, &chosen, &reports, env);
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("selftest failed: {}", failed.join(", ")))
    }
}

/// Whether the variable `name` only configures the build of the app, as coverage's instrumentation does: the cargo
/// and rust-analyzer a scenario starts would otherwise build the scenario's projects instrumented, in the coverage
/// build's folders and after its cleaning, which slows them by minutes.
fn is_build_only(name: &str) -> bool {
    matches!(
        name,
        "RUSTC_WRAPPER" | "RUSTFLAGS" | "CARGO_ENCODED_RUSTFLAGS" | "CARGO_TARGET_DIR" | "CARGO_BUILD_BUILD_DIR"
    ) || name.starts_with("CARGO_LLVM_COV")
        || name.starts_with("__CARGO_LLVM_COV")
        || name.starts_with("CARGO_PROFILE_")
}

fn is_textkit_assertion(line: &str) -> bool {
    line.contains(TEXTKIT_ASSERTION)
}

/// The report of `scenario` from what the app wrote, if it wrote anything. A report without checks or without how
/// the scenario ended is refused, since it would pass a run that checked nothing or stopped halfway.
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
/// it made itself, the scenario did not give up, and every reported check held.
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
            if let Ending::GaveUp(why) = &report.ending {
                problems.push(format!("the scenario gave up before its end: {why}"));
            }
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

    const PASSING: &str = r#"{"scenario": "probe", "ending": "ran", "checks": [
        {"name": "window opens", "ok": true, "detail": "1 window"},
        {"name": "settings load", "ok": true, "detail": ""}
    ]}"#;

    const FAILING: &str = r#"{"scenario": "probe", "ending": "ran", "checks": [
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
    fn the_offline_scenarios_leave_out_those_named_after_except() {
        let row = |name: &str, offline: bool| {
            format!(
                r#"{{"name": "{name}", "limit_secs": 120, "offline": {offline}, "launches": "once", "arguments": [], "dictation": false}}"#
            )
        };
        let table = parse_listing(&[row("probe", true), row("live", false), row("slow", true)].join("\n")).unwrap();
        let names = |asked: &[&str]| -> Result<Vec<String>, String> {
            let asked: Vec<String> = asked.iter().map(ToString::to_string).collect();
            Ok(resolve(&asked, &table)?.into_iter().map(|s| s.name).collect())
        };
        assert_eq!(names(&[OFFLINE]).unwrap(), ["probe", "slow"]);
        assert_eq!(names(&[OFFLINE, EXCEPT, "slow"]).unwrap(), ["probe"]);
        assert_eq!(names(&[EXCEPT, "slow", "slow", "live"]).unwrap(), ["live"]);
        assert!(names(&[OFFLINE, EXCEPT, "missing"]).is_err());
        assert!(names(&[OFFLINE, EXCEPT]).is_err());
        assert!(
            table.iter().all(|s| !s.alone),
            "a row without `alone` runs beside others"
        );
        let alone = parse_listing(&row("timed", true).replace("}", r#", "alone": true}"#)).unwrap();
        assert!(alone[0].alone);
    }

    #[test]
    fn the_app_gets_no_variable_of_the_instrumented_build_but_its_profile_file() {
        for name in [
            "RUSTC_WRAPPER",
            "RUSTFLAGS",
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_BUILD_DIR",
            "CARGO_LLVM_COV_TARGET_DIR",
            "__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS",
            "CARGO_PROFILE_RELEASE_LTO",
        ] {
            assert!(is_build_only(name), "{name}");
        }
        for name in ["LLVM_PROFILE_FILE", "PATH", "CARGO_HOME", "RUSTUP_HOME", "IDE_SELFTEST"] {
            assert!(!is_build_only(name), "{name}");
        }
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
            r#"{"scenario": "probe", "ending": "ran"}"#,
            r#"{"scenario": "probe", "ending": "ran", "checks": [{"name": "x", "ok": "yes", "detail": ""}]}"#,
            r#"{"scenario": "probe", "ending": "ran", "checks": [{"name": "x", "ok": true}]}"#,
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
            judged(
                Ended::Exited(0),
                Some(r#"{"scenario": "probe", "ending": "ran", "checks": []}"#)
            ),
            ["the report lists no checks"]
        );
    }

    #[test]
    fn a_report_that_does_not_say_how_the_scenario_ended_fails_the_run() {
        for written in [
            r#"{"scenario": "probe", "checks": [{"name": "x", "ok": true, "detail": ""}]}"#,
            r#"{"scenario": "probe", "ending": "stopped", "checks": [{"name": "x", "ok": true, "detail": ""}]}"#,
            r#"{"scenario": "probe", "ending": {"gave_up": 3}, "checks": [{"name": "x", "ok": true, "detail": ""}]}"#,
        ] {
            let problems = judged(Ended::Exited(0), Some(written));
            assert_eq!(problems.len(), 1, "{written}");
            assert!(problems[0].starts_with("the report is unreadable: "), "{written}");
        }
    }

    #[test]
    fn a_scenario_that_gave_up_fails_even_with_every_check_held() {
        let gave_up = r#"{"scenario": "probe", "ending": {"gave_up": "pairs.rs opens in an editor (gave up at src/selftest/editor_commands.rs:129)"},
            "checks": [{"name": "window opens", "ok": true, "detail": "1 window"}]}"#;
        assert_eq!(
            judged(Ended::Exited(0), Some(gave_up)),
            [
                "the scenario gave up before its end: pairs.rs opens in an editor (gave up at \
              src/selftest/editor_commands.rs:129)"
            ]
        );
        let with_its_check = r#"{"scenario": "probe", "ending": {"gave_up": "pairs.rs opens in an editor"}, "checks": [
            {"name": "window opens", "ok": true, "detail": "1 window"},
            {"name": "pairs.rs opens in an editor", "ok": false, "detail": "gave up at src/selftest/editor_commands.rs:129"}
        ]}"#;
        assert_eq!(
            judged(Ended::Exited(0), Some(with_its_check)),
            [
                "the scenario gave up before its end: pairs.rs opens in an editor",
                "1 of 2 checks failed"
            ]
        );
        let crashed = parse_report("probe", Some(gave_up.as_bytes()));
        assert_eq!(
            problems(Ended::Signalled(SIGKILL), Ended::Signalled(SIGKILL), &crashed).len(),
            1
        );
    }

    #[test]
    fn a_scenario_that_skipped_its_rest_on_purpose_passes() {
        let skipped = r#"{"scenario": "probe", "ending": {"skipped": "rust-analyzer missing"}, "checks": [
            {"name": "rust-analyzer is there", "ok": true, "detail": "skipped: rust-analyzer missing"}
        ]}"#;
        assert!(judged(Ended::Exited(0), Some(skipped)).is_empty());
        let report = parse_report("probe", Some(skipped.as_bytes())).unwrap();
        assert_eq!(report.ending, Ending::Skipped("rust-analyzer missing".into()));
        assert_eq!(
            parse_report("probe", Some(PASSING.as_bytes())).unwrap().ending,
            Ending::Ran
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
