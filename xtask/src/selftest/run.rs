//! Runs the chosen scenarios on virtual displays of their own, `LANES` beside each other and those that measure time
//! alone, so that none of them shows on the user's screen; each one's output comes as a block once it ended.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::{Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use super::*;

#[cfg(target_os = "macos")]
use crate::displays::{Displays, outside};

/// How many scenarios run at once, each on a display of its own.
const LANES: u32 = 3;

/// The variable that has the run show on the user's screen, one scenario at a time, for a look at what it does.
const ON_SCREEN: &str = "IDE_SELFTEST_ON_SCREEN";

fn on_screen() -> Option<String> {
    std::env::var_os(ON_SCREEN).map(|_| format!("{ON_SCREEN} is set"))
}

/// How often the runner looks whether a window of the app shows on the user's screen.
const LOOK_EVERY: Duration = Duration::from_millis(100);

/// Where a scenario runs: on a virtual display, or on the user's screen when macOS made none.
#[derive(Clone, Copy)]
struct Stage<'a> {
    display: Option<u32>,
    /// Every virtual display of the run, none of which is the user's.
    virtual_displays: &'a [u32],
}

/// One scenario's launches: the app, the scenario, the variables for the app on top of this process's, and where.
struct Launcher<'a> {
    program: &'a Path,
    listed: &'a Listed,
    env: &'a [(String, String)],
    stage: Stage<'a>,
}

/// Runs `chosen` and returns the names of those that failed, in the order of `chosen`.
pub(super) fn run_all(program: &Path, chosen: &[Listed], reports: &Path, env: &[(String, String)]) -> Vec<String> {
    let lanes = u32::try_from(chosen.len()).unwrap_or(LANES).clamp(1, LANES);
    let made = match on_screen().map_or_else(|| Displays::make(lanes), Err) {
        Ok(made) => Some(made),
        Err(why) => {
            println!("selftest runs on your screen, one scenario at a time: {why}");
            None
        }
    };
    let stages: Vec<Stage> = match &made {
        Some(made) => made
            .ids
            .iter()
            .map(|id| Stage {
                display: Some(*id),
                virtual_displays: &made.ids,
            })
            .collect(),
        None => vec![Stage {
            display: None,
            virtual_displays: &[],
        }],
    };
    let (alone, beside): (Vec<&Listed>, Vec<&Listed>) = chosen.iter().partition(|s| s.alone);
    let waiting = Mutex::new(beside.into_iter().collect::<VecDeque<_>>());
    let failed = Mutex::new(Vec::new());
    let run = |listed: &Listed, stage: Stage| {
        if !run_one(program, listed, reports, env, stage) {
            failed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(listed.name.clone());
        }
    };
    std::thread::scope(|scope| {
        for stage in &stages {
            scope.spawn(|| {
                // The guard of a `while let` condition lives through the loop's body, which would run one scenario
                // at a time.
                let next = || waiting.lock().unwrap_or_else(PoisonError::into_inner).pop_front();
                while let Some(listed) = next() {
                    run(listed, *stage);
                }
            });
        }
    });
    if !alone.is_empty() && std::env::var_os("LLVM_PROFILE_FILE").is_none() {
        // Without coverage the app holds these to frame budgets, which the load of the scenarios before would break.
        crate::perf::settle();
    }
    for listed in alone {
        run(listed, stages[0]);
    }
    let failed = failed.into_inner().unwrap_or_else(PoisonError::into_inner);
    chosen
        .iter()
        .filter(|s| failed.contains(&s.name))
        .map(|s| s.name.clone())
        .collect()
}

/// Runs one scenario and prints what it did as one block; false if it failed.
fn run_one(program: &Path, listed: &Listed, reports: &Path, env: &[(String, String)], stage: Stage) -> bool {
    let name = listed.name.as_str();
    let started = Instant::now();
    let mut out = String::new();
    let launcher = Launcher {
        program,
        listed,
        env,
        stage,
    };
    let outcome = run_scenario(&launcher, &reports.join(format!("{name}.json")), &mut out)
        .and_then(|()| baseline::compare(name, reports, &mut out));
    let took = started.elapsed().as_secs();
    let mut stdout = std::io::stdout().lock();
    let _ = write!(stdout, "selftest {name}\n{out}");
    match outcome {
        Ok(()) => {
            let _ = writeln!(stdout, "selftest {name}: passed in {took} s");
            true
        }
        Err(e) => {
            drop(stdout);
            eprintln!("selftest {name} failed after {took} s: {e}");
            false
        }
    }
}

/// Runs the app on one scenario with a home of its own, twice for a crashing one, and writes the checks it reports
/// to `out`.
fn run_scenario(launcher: &Launcher, report_file: &Path, out: &mut String) -> Result<(), String> {
    let listed = launcher.listed;
    let scenario = listed.name.as_str();
    let home = fresh_home(scenario)?;
    let mut problems = Vec::new();
    for (launches, which, expected) in [
        (Launches::Quit, "quit", Ended::Exited(0)),
        (Launches::Crash, "crash", Ended::Signalled(SIGKILL)),
    ] {
        if listed.launches != launches {
            continue;
        }
        let first = report_file.with_file_name(format!("{scenario}-{which}.json"));
        let _ = writeln!(
            out,
            "  first launch, which {}:",
            if which == "quit" { "quits" } else { "crashes" }
        );
        let (ended, seen) = launcher.launch(&first, &home, Some(which), out)?;
        problems.extend(
            seen.into_iter()
                .chain(judge(ended, expected, &first, scenario, out))
                .map(|problem| format!("first launch: {problem}")),
        );
        if problems.is_empty() {
            let _ = writeln!(out, "  second launch:");
        }
    }
    if problems.is_empty() {
        let (ended, seen) = launcher.launch(report_file, &home, None, out)?;
        problems = seen
            .into_iter()
            .chain(judge(ended, Ended::Exited(0), report_file, scenario, out))
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

impl Launcher<'_> {
    /// Starts the app on its scenario in `home` on the stage's display, telling it which `launch` this is if it is one of
    /// several, and waits for it to end; with it, the problems if it took the keyboard, showed a window on the user's
    /// screen or TextKit asserted.
    fn launch(
        &self,
        report: &Path,
        home: &Path,
        which: Option<&str>,
        out: &mut String,
    ) -> Result<(Ended, Vec<String>), String> {
        let Launcher {
            program,
            listed,
            env,
            stage,
        } = *self;
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
            .env(crate::perf::DIR_VARIABLE, crate::perf::dir())
            .envs(env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .stderr(Stdio::piped());
        for (key, _) in std::env::vars_os() {
            if key.to_str().is_some_and(is_build_only) {
                command.env_remove(key);
            }
        }
        if let Some(display) = stage.display {
            command.env("IDE_SELFTEST_DISPLAY", display.to_string());
        }
        if let Some(which) = which {
            command.env("IDE_SELFTEST_LAUNCH", which);
        }
        let guard = focus::Guard::start()?;
        let mut child = command.spawn().map_err(|e| format!("{}: {e}", program.display()))?;
        guard.guard(child.id());
        let errors = child.stderr.take().map(read_errors);
        let pid = child.id();
        let mut shown = None;
        let ended = wait_within_limit(&mut child, Duration::from_secs(listed.limit_secs), || {
            if shown.is_none() {
                shown = stage.display.and_then(|_| outside(pid, stage.virtual_displays));
            }
        })
        .map_err(|e| format!("{}: {e}", program.display()))?;
        let mut problems: Vec<String> = guard.finish().into_iter().collect();
        problems.extend(shown.map(|frame| format!("a window showed on the user's screen at {frame}")));
        let lines = errors.map(collect_errors).unwrap_or_default();
        for line in &lines {
            let _ = writeln!(out, "{line}");
        }
        if let Some(line) = lines.iter().find(|line| is_textkit_assertion(line)) {
            problems.push(format!("TextKit asserted: {line}"));
        }
        Ok((ended, problems))
    }
}

/// Reads the app's error output line by line on a thread of its own.
fn read_errors(stderr: ChildStderr) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

/// The app's error output once it ended; a child process the app started may hold the pipe open after it, so the
/// rest is waited for two seconds at most.
fn collect_errors(receiver: mpsc::Receiver<String>) -> Vec<String> {
    let until = Instant::now() + Duration::from_secs(2);
    let mut lines = Vec::new();
    while let Ok(line) = receiver.recv_timeout(until.saturating_duration_since(Instant::now())) {
        lines.push(line);
    }
    lines
}

/// Writes the checks of the report at `report` to `out` and says what failed, for a launch that should have ended as
/// `expected`.
fn judge(ended: Ended, expected: Ended, report: &Path, scenario: &str, out: &mut String) -> Vec<String> {
    let written = std::fs::read(report).ok();
    let report = parse_report(scenario, written.as_deref());
    if let Ok(report) = &report {
        for check in &report.checks {
            let _ = writeln!(out, "  {}", check_line(check));
        }
        if let Ending::Skipped(why) = &report.ending {
            let _ = writeln!(out, "  skipped the rest: {why}");
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

/// Waits for `child` to end, killing it at `limit`, and calls `look` every `LOOK_EVERY` meanwhile.
fn wait_within_limit(child: &mut Child, limit: Duration, mut look: impl FnMut()) -> std::io::Result<Ended> {
    let started = Instant::now();
    let mut looked = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Ended::of(status));
        }
        if started.elapsed() >= limit {
            child.kill()?;
            child.wait()?;
            return Ok(Ended::TimedOut(limit));
        }
        if looked.elapsed() >= LOOK_EVERY {
            look();
            looked = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Stands in for the virtual displays where macOS's are not.
#[cfg(not(target_os = "macos"))]
struct Displays {
    ids: Vec<u32>,
}

#[cfg(not(target_os = "macos"))]
impl Displays {
    fn make(_count: u32) -> Result<Displays, String> {
        Err("virtual displays exist only on macOS".into())
    }
}

#[cfg(not(target_os = "macos"))]
fn outside(_pid: u32, _ids: &[u32]) -> Option<String> {
    None
}
