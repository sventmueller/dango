//! Line coverage of the workspace: `cargo test` and the offline selftest scenarios, built with LLVM's instrumentation
//! by cargo-llvm-cov in `target/llvm-cov-target`, reported together in `target/coverage/`, with `cargo test` alone
//! and the self-test alone beside it, so that code only a window self-test reaches shows as such.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::source::rust_files;
use crate::*;

/// The least line coverage of all counted files together, in percent, that `--check` accepts.
const TOTAL_GOAL: f64 = 90.0;

/// The least line coverage of each counted file, in percent, that `--check` accepts.
const FILE_GOAL: f64 = 50.0;

/// The line coverage, in percent to one decimal, that `cargo test` alone reaches in the app crate's `src/` outside its
/// self-test, whose logic otherwise only the window self-tests run. A ratchet held like the others: `--check` fails
/// below it and names the value to raise it to above it.
const APP_UNIT_FLOOR: f64 = 8.4;

/// Where the profiles of `cargo test` wait while the self-test alone is reported.
const UNIT_PROFILES: &str = "unit-profiles";

/// Folders under a counted source folder whose files are not counted.
const NOT_COUNTED: &[&str] = &["tests", "benches", "examples", "fuzz"];

/// What one counted file's lines came to.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FileCoverage {
    /// The path from the workspace root.
    pub(crate) file: String,
    pub(crate) covered: u64,
    pub(crate) lines: u64,
    /// False for a file no instrumented program contained; its lines are then its non-blank lines without comments.
    pub(crate) built: bool,
}

impl FileCoverage {
    pub(crate) fn percent(&self) -> f64 {
        percent(self.covered, self.lines)
    }
}

/// The cargo commands of a coverage run: rustup's toolchain, which has llvm-tools, with cargo-llvm-cov's
/// instrumenting environment and folders apart from the normal build's.
struct Instrumented {
    root: PathBuf,
    toolchain: String,
    path: OsString,
    target: PathBuf,
    build_dir: Option<PathBuf>,
    vars: Vec<(String, String)>,
}

impl Instrumented {
    fn prepare(sh: &mut dyn Shell, root: &Path, target: &Path) -> Result<Instrumented, String> {
        let toolchain = toolchain(&files::read_to_string(root.join("rust-toolchain.toml"))?)?;
        let path = crate::fuzz::toolchain_path(sh, &toolchain)?;
        let mut env = Instrumented {
            root: root.to_path_buf(),
            toolchain,
            path,
            target: target.join("llvm-cov-target"),
            build_dir: None,
            vars: Vec::new(),
        };
        env.vars = env.show_env(sh)?;
        // cargo cannot see the flags cargo-llvm-cov's wrapper adds, so a shared build folder would mix instrumented
        // and plain artifacts under the same names.
        if let Some((_, dir)) = env.vars.iter().find(|(key, _)| key == "CARGO_LLVM_COV_BUILD_DIR")
            && !Path::new(dir).starts_with(&env.target)
        {
            env.build_dir = Some(PathBuf::from(format!("{dir}-coverage")));
            env.vars = env.show_env(sh)?;
        }
        Ok(env)
    }

    fn show_env(&self, sh: &mut dyn Shell) -> Result<Vec<(String, String)>, String> {
        let shown = sh.stdout(&mut self.plain(&["llvm-cov", "show-env"]))?;
        Ok(parse_shown(&shown))
    }

    /// A cargo command with the instrumenting environment.
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = self.plain(args);
        cmd.envs(self.vars.iter().map(|(key, value)| (key, value)));
        cmd
    }

    /// A cargo command of the toolchain in the coverage folders, without the instrumenting environment, which
    /// `show-env` would otherwise wrap a second time.
    fn plain(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.root)
            .env("PATH", &self.path)
            .env("RUSTUP_TOOLCHAIN", &self.toolchain)
            .env("CARGO_TARGET_DIR", &self.target)
            .env_remove("CARGO")
            .env_remove("RUSTC")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .args(args);
        match &self.build_dir {
            Some(dir) => cmd.env("CARGO_BUILD_BUILD_DIR", dir),
            None => cmd.env_remove("CARGO_BUILD_BUILD_DIR"),
        };
        cmd
    }

    /// The release programs of the selftest bundle, which cargo-llvm-cov's report does not look for by itself.
    fn bundle_objects(&self) -> Result<String, String> {
        let programs = std::iter::once(PROGRAM).chain(SERVICES.iter().map(|service| service.program));
        let mut flags = Vec::new();
        for program in programs {
            let object = self.target.join("release").join(program);
            if !object.exists() {
                continue;
            }
            let object = object.to_string_lossy().into_owned();
            if object.contains(char::is_whitespace) {
                return Err(format!(
                    "{object}: LLVM_COV_FLAGS splits at spaces, so the path must have none"
                ));
            }
            flags.push(format!("-object={object}"));
        }
        Ok(flags.join(" "))
    }
}

/// Measures, writes the report and, with `--check`, fails below the goals; with `--strict` also when a test or a
/// selftest scenario failed.
pub(crate) fn coverage(args: &[String]) -> Result<(), String> {
    coverage_in(&mut System, args, &root(), &target_dir())
}

/// `coverage` of the workspace at `root` with the target folder `target`.
fn coverage_in(sh: &mut dyn Shell, args: &[String], root: &Path, target: &Path) -> Result<(), String> {
    let [check, strict] = flags(args, ["--check", "--strict"])?;
    let started = Instant::now();
    let out = target.join("coverage");
    files::create_dir_all(&out)?;
    let env = Instrumented::prepare(sh, root, target)?;
    sh.run(&mut env.command(&["llvm-cov", "clean", "--workspace"]))?;

    let mut failed = Vec::new();
    let mut timings = Vec::new();
    let step = Instant::now();
    let tests = ["test", "--workspace", "--no-fail-fast", "--lib", "--bins", "--tests"];
    if sh.run(&mut env.command(&tests)).is_err() {
        failed.push("cargo test".to_string());
    }
    timings.push(("cargo test", step.elapsed()));
    let unit_export = out.join("llvm-cov-unit.json");
    export(sh, &env, &unit_export, "")?;
    let waiting = env.target.join(UNIT_PROFILES);
    move_profiles(&env.target, &waiting)?;

    let step = Instant::now();
    let scenarios = vec![OFFLINE.to_string()];
    let mut cmd = env.command(&["xtask", "selftest", OFFLINE]);
    cmd.env("LLVM_PROFILE_FILE", env.target.join("selftest-%p.profraw"));
    if sh.run(&mut cmd).is_err() {
        failed.push("selftest (see its output for the scenarios)".to_string());
    }
    timings.push(("selftest", step.elapsed()));

    let step = Instant::now();
    let objects = env.bundle_objects()?;
    let selftest_export = out.join("llvm-cov-selftest.json");
    let selftest_reported = export(sh, &env, &selftest_export, &objects);
    move_profiles(&waiting, &env.target)?;
    if selftest_reported.is_err() {
        failed.push("the report of the self-test alone".to_string());
    }
    let combined_export = out.join("llvm-cov.json");
    export(sh, &env, &combined_export, &objects)?;
    timings.push(("report", step.elapsed()));

    let compiled = compiled(env.build_dir.as_ref().unwrap_or(&env.target), root)?;
    let selftest = match selftest_reported {
        Ok(()) => covered_lines(counted(root, &parse_export(&read(&selftest_export)?)?, &compiled)?),
        Err(_) => BTreeMap::new(),
    };
    let alone = Alone {
        unit: covered_lines(counted(root, &parse_export(&read(&unit_export)?)?, &compiled)?),
        selftest,
    };
    let together = counted(root, &parse_export(&read(&combined_export)?)?, &compiled)?;
    timings.push(("total", started.elapsed()));
    let run = Run {
        scenarios,
        failed,
        timings,
    };
    report(&out, &together, &alone, &run, check, strict)
}

/// Moves the profiles in `from` into `to`, so that a report reads only those recorded after; none without `from`.
fn move_profiles(from: &Path, to: &Path) -> Result<(), String> {
    if !from.is_dir() {
        return Ok(());
    }
    files::create_dir_all(to)?;
    for entry in files::read_dir(from)? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|ext| ext == "profraw")
            && let Some(name) = path.file_name()
        {
            files::rename(&path, to.join(name))?;
        }
    }
    Ok(())
}

/// Writes the report into `out`, prints its summary and, with `check` and `strict`, fails as `coverage` describes.
fn report(
    out: &Path,
    counted: &[FileCoverage],
    alone: &Alone,
    run: &Run,
    check: bool,
    strict: bool,
) -> Result<(), String> {
    files::write(out.join("files.json"), files_json(counted, alone))?;
    let table = table(counted, alone, run);
    files::write(out.join("coverage.txt"), &table)?;
    print!("{}", summary(counted, alone, run));
    println!("{}", out.join("coverage.txt").display());
    let mut problems = Vec::new();
    if strict && !run.failed.is_empty() {
        problems.push(format!("failed: {}", run.failed.join(", ")));
    }
    if check && let Err(below) = verdict(counted, alone, APP_UNIT_FLOOR) {
        problems.push(below);
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Writes llvm-cov's summary of the profiles recorded so far to `path`; `objects` names programs beyond cargo's own.
fn export(sh: &mut dyn Shell, env: &Instrumented, path: &Path, objects: &str) -> Result<(), String> {
    let report = [
        "llvm-cov",
        "report",
        "--workspace",
        "--json",
        "--summary-only",
        "--failure-mode",
        "all",
        "--output-path",
    ];
    let mut cmd = env.command(&report);
    cmd.arg(path).env("LLVM_COV_FLAGS", objects);
    sh.run(&mut cmd)
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Covered lines by file.
type Lines = BTreeMap<String, u64>;

/// Covered lines by file from each kind of test alone: `cargo test`, so code that the self-test only runs without
/// asserting shows as a gap, and the self-test, so code that only a window reaches shows as such.
#[derive(Default)]
struct Alone {
    unit: Lines,
    selftest: Lines,
}

fn covered_lines(files: Vec<FileCoverage>) -> Lines {
    files.into_iter().map(|file| (file.file, file.covered)).collect()
}

/// The lines of `files` that the run whose lines are `alone` covered.
fn covered_by<'a>(files: impl IntoIterator<Item = &'a FileCoverage>, alone: &Lines) -> u64 {
    files
        .into_iter()
        .map(|file| alone.get(&file.file).copied().unwrap_or(0).min(file.lines))
        .sum()
}

/// The covered lines and lines of the app crate's counted files, which leave its self-test out, by `cargo test` alone.
fn app_unit(files: &[FileCoverage], alone: &Alone) -> (u64, u64) {
    let app: Vec<&FileCoverage> = files.iter().filter(|file| file.file.starts_with("src/")).collect();
    let lines = app.iter().map(|file| file.lines).sum();
    (covered_by(app, &alone.unit), lines)
}

/// What a run did besides measuring.
struct Run {
    scenarios: Vec<String>,
    failed: Vec<String>,
    timings: Vec<(&'static str, Duration)>,
}

/// The channel `rust-toolchain.toml` names.
fn toolchain(file: &str) -> Result<String, String> {
    file.lines()
        .filter_map(|line| line.trim().strip_prefix("channel"))
        .filter_map(|rest| rest.trim().strip_prefix('='))
        .map(|value| value.trim().trim_matches('"').to_string())
        .find(|value| !value.is_empty())
        .ok_or_else(|| "rust-toolchain.toml names no channel".to_string())
}

/// The variables of `cargo llvm-cov show-env`'s plain output, `KEY=value` with the value in single quotes where
/// the shell needs them.
fn parse_shown(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let is_name = !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
            if !is_name {
                return None;
            }
            let value = match value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
                Some(quoted) => quoted.replace("'\\''", "'"),
                None => value.to_string(),
            };
            Some((key.to_string(), value))
        })
        .collect()
}

#[derive(Deserialize)]
struct Export {
    data: Vec<ExportData>,
}

#[derive(Deserialize)]
struct ExportData {
    files: Vec<ExportFile>,
}

#[derive(Deserialize)]
struct ExportFile {
    filename: String,
    summary: ExportSummary,
}

#[derive(Deserialize)]
struct ExportSummary {
    lines: ExportLines,
}

#[derive(Deserialize)]
struct ExportLines {
    count: u64,
    covered: u64,
}

/// Covered and total lines by absolute file name, from `llvm-cov export -summary-only`.
fn parse_export(json: &[u8]) -> Result<BTreeMap<PathBuf, (u64, u64)>, String> {
    let export: Export = serde_json::from_slice(json).map_err(|e| format!("the coverage export is unreadable: {e}"))?;
    let mut files = BTreeMap::new();
    for file in export.data.into_iter().flat_map(|data| data.files) {
        files.insert(
            PathBuf::from(file.filename),
            (file.summary.lines.covered, file.summary.lines.count),
        );
    }
    Ok(files)
}

/// Every counted Rust file in `src/`, `crates/*/src` and `xtask/src` with what `measured` says about it. llvm-cov
/// leaves out a file without code to run, such as a `lib.rs` of only `mod` lines; one that `compiled` lists has none.
fn counted(
    root: &Path,
    measured: &BTreeMap<PathBuf, (u64, u64)>,
    compiled: &BTreeSet<PathBuf>,
) -> Result<Vec<FileCoverage>, String> {
    let mut folders = vec![root.join("src"), root.join("xtask/src")];
    for entry in files::read_dir(root.join("crates"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        folders.push(entry.path().join("src"));
    }
    let mut sources = Vec::new();
    for folder in folders.iter().filter(|folder| folder.is_dir()) {
        sources.extend(rust_files(folder).into_iter().filter(|file| is_counted(folder, file)));
    }
    let mut counted = Vec::new();
    for source in sources {
        let file = source
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        if is_test_only(&file) || UNMEASURABLE.contains(&file.as_str()) {
            continue;
        }
        counted.push(match measured.get(&source) {
            Some(&(covered, lines)) => FileCoverage {
                file,
                covered,
                lines,
                built: true,
            },
            None if compiled.contains(&source) => FileCoverage {
                file,
                covered: 0,
                lines: 0,
                built: true,
            },
            None => FileCoverage {
                file,
                covered: 0,
                lines: code_lines(&files::read_to_string(&source)?),
                built: false,
            },
        });
    }
    Ok(counted)
}

/// Whether `file` below the counted source folder `folder` counts: not in a `NOT_COUNTED` folder below it, and no
/// file `is_not_counted` names.
fn is_counted(folder: &Path, file: &Path) -> bool {
    let below = file.strip_prefix(folder).unwrap_or(file);
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    let in_folder_not_counted = below
        .parent()
        .is_some_and(|parent| parent.iter().any(|part| NOT_COUNTED.iter().any(|skip| part == *skip)));
    !in_folder_not_counted && !is_not_counted(&name)
}

/// A build script, a file of unit tests by cargo-llvm-cov's convention (`tests.rs`, `*_tests.rs`, `*-tests.rs`),
/// which its report leaves out, or a crate's entry points for its fuzz targets, built only with the feature `fuzzing`.
fn is_not_counted(name: &str) -> bool {
    matches!(name, "build.rs" | "tests.rs" | "fuzzing.rs") || name.ends_with("_tests.rs") || name.ends_with("-tests.rs")
}

/// Entry points no instrumented run can record: launchd starts the decoding service in an App Sandbox that cannot
/// write profile data. The `preview` scenario exercises it through the app.
const UNMEASURABLE: &[&str] = &["src/decoder_main.rs"];

/// Code that exists only to test the rest, at `file` from the workspace root: the window self-test scenarios and their
/// hooks, and the ACP test agent with its recorder. xtask's self-test runner is tooling and counts.
fn is_test_only(file: &str) -> bool {
    file == "src/selftest.rs"
        || file.starts_with("src/selftest/")
        || (file.ends_with("/selftest.rs") && !file.starts_with("xtask/"))
        || file.contains("/selftest/")
        || file == "crates/ide-ui-terminal/src/inspect.rs"
        || file == "crates/ide-acp/src/test_agent.rs"
        || file.starts_with("crates/ide-acp/src/bin/")
}

/// Every source file the instrumented build compiled, from the dep-info files cargo writes beside each artifact in
/// `build/<profile>/deps`, which name the workspace's files from `root`.
fn compiled(build: &Path, root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut sources = BTreeSet::new();
    for profile in ["debug", "release"] {
        let deps = build.join(profile).join("deps");
        if !deps.is_dir() {
            continue;
        }
        for entry in files::read_dir(&deps)? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "d") {
                sources.extend(dep_info_sources(&files::read_to_string(&path)?).map(|source| root.join(source)));
            }
        }
    }
    Ok(sources)
}

/// The Rust files a dep-info file names, in make's syntax with spaces escaped as `\ `.
fn dep_info_sources(text: &str) -> impl Iterator<Item = PathBuf> + '_ {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .flat_map(|line| {
            let escaped = line.replace("\\ ", "\u{0}");
            escaped
                .split_whitespace()
                .map(|word| word.trim_end_matches(':').replace('\u{0}', " "))
                .collect::<Vec<_>>()
        })
        .filter(|word| word.ends_with(".rs"))
        .map(PathBuf::from)
}

/// The lines of a file that are neither blank nor only a comment.
fn code_lines(text: &str) -> u64 {
    let count = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .count();
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// Covered lines as a percentage of `lines`; a file without lines has nothing left to cover.
fn percent(covered: u64, lines: u64) -> f64 {
    if lines == 0 {
        100.0
    } else {
        #[expect(clippy::cast_precision_loss, reason = "line counts stay far below 2^52")]
        let ratio = covered as f64 / lines as f64;
        ratio * 100.0
    }
}

fn totals<'a>(files: impl IntoIterator<Item = &'a FileCoverage>) -> (u64, u64) {
    files.into_iter().fold((0, 0), |(covered, lines), file| {
        (covered + file.covered, lines + file.lines)
    })
}

/// The folder a file is reported under: the crate, `xtask`, or the app's top folder below `src/`.
fn group(file: &str) -> String {
    let parts: Vec<&str> = file.split('/').collect();
    match parts.as_slice() {
        ["crates", name, ..] => format!("crates/{name}"),
        ["src", folder, _, ..] => format!("src/{folder}"),
        ["src", _] => "src".to_string(),
        [first, ..] => (*first).to_string(),
        [] => String::new(),
    }
}

fn rounded(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn files_json(files: &[FileCoverage], alone: &Alone) -> String {
    let (covered, lines) = totals(files);
    let (unit_total, selftest_total) = (covered_by(files, &alone.unit), covered_by(files, &alone.selftest));
    let (app_covered, app_lines) = app_unit(files, alone);
    let files: Vec<serde_json::Value> = sorted(files)
        .iter()
        .map(|file| {
            let (unit, selftest) = (covered_by([*file], &alone.unit), covered_by([*file], &alone.selftest));
            serde_json::json!({
                "file": file.file,
                "covered": file.covered,
                "lines": file.lines,
                "percent": rounded(file.percent()),
                "unit_covered": unit,
                "unit_percent": rounded(percent(unit, file.lines)),
                "selftest_covered": selftest,
                "selftest_percent": rounded(percent(selftest, file.lines)),
                "built": file.built,
            })
        })
        .collect();
    let json = serde_json::json!({
        "total": {"covered": covered, "lines": lines, "percent": rounded(percent(covered, lines)),
            "unit_covered": unit_total, "unit_percent": rounded(percent(unit_total, lines)),
            "selftest_covered": selftest_total, "selftest_percent": rounded(percent(selftest_total, lines))},
        "app_unit": {"covered": app_covered, "lines": app_lines, "percent": rounded(percent(app_covered, app_lines))},
        "goals": {"total": TOTAL_GOAL, "file": FILE_GOAL, "app_unit": APP_UNIT_FLOOR},
        "files": files,
    });
    serde_json::to_string_pretty(&json).unwrap_or_default()
}

/// The files, lowest coverage first, then by name.
fn sorted(files: &[FileCoverage]) -> Vec<&FileCoverage> {
    let mut sorted: Vec<&FileCoverage> = files.iter().collect();
    sorted.sort_by(|a, b| a.percent().total_cmp(&b.percent()).then_with(|| a.file.cmp(&b.file)));
    sorted
}

fn summary(files: &[FileCoverage], alone: &Alone, run: &Run) -> String {
    let (covered, lines) = totals(files);
    let below = files.iter().filter(|file| file.percent() < FILE_GOAL).count();
    let mut text = format!(
        "line coverage {:.1} % ({covered} of {lines} lines) in {} files, {below} below {FILE_GOAL} %; goals {TOTAL_GOAL} \
         % in total, {FILE_GOAL} % in each file\n",
        percent(covered, lines),
        files.len(),
    );
    for (name, lines_alone) in [("cargo test", &alone.unit), ("self-test", &alone.selftest)] {
        let total = covered_by(files, lines_alone);
        let _ = writeln!(
            text,
            "{name} alone {:.1} % ({total} of {lines} lines)",
            percent(total, lines)
        );
    }
    let (app_covered, app_lines) = app_unit(files, alone);
    let _ = writeln!(
        text,
        "cargo test alone in the app crate's src/ {:.1} % ({app_covered} of {app_lines} lines), floor {APP_UNIT_FLOOR} %",
        percent(app_covered, app_lines)
    );
    let unbuilt = files.iter().filter(|file| !file.built).count();
    if unbuilt > 0 {
        let _ = writeln!(
            text,
            "{unbuilt} files are in no instrumented program and count as uncovered"
        );
    }
    let _ = writeln!(text, "selftest scenarios: {}", run.scenarios.join(" "));
    if !run.failed.is_empty() {
        let _ = writeln!(
            text,
            "failed, so the numbers miss what it did not reach: {}",
            run.failed.join(", ")
        );
    }
    let timings: Vec<String> = run
        .timings
        .iter()
        .map(|(step, took)| format!("{step} {}", minutes(*took)))
        .collect();
    let _ = writeln!(text, "took {}", timings.join(", "));
    text
}

pub(crate) fn minutes(took: Duration) -> String {
    let seconds = took.as_secs();
    format!("{}:{:02} min", seconds / 60, seconds % 60)
}

/// The crate a file belongs to: `crates/<name>`, `xtask`, or `src` for the app.
fn crate_of(file: &str) -> String {
    match file.split('/').collect::<Vec<_>>().as_slice() {
        ["crates", name, ..] => format!("crates/{name}"),
        [first, ..] => (*first).to_string(),
        [] => String::new(),
    }
}

/// A row of the report for several files.
struct Group {
    name: String,
    covered: u64,
    lines: u64,
    /// The files below `FILE_GOAL`.
    below: usize,
    /// The lines `cargo test` covered alone.
    unit: u64,
    /// The lines the self-test covered alone.
    selftest: u64,
}

/// The rows of `files` grouped by `key`, lowest coverage first.
fn grouped(files: &[FileCoverage], alone: &Alone, key: fn(&str) -> String) -> Vec<Group> {
    let mut groups: BTreeMap<String, Vec<&FileCoverage>> = BTreeMap::new();
    for file in files {
        groups.entry(key(&file.file)).or_default().push(file);
    }
    let mut rows: Vec<Group> = groups
        .into_iter()
        .map(|(name, members)| {
            let below = members.iter().filter(|file| file.percent() < FILE_GOAL).count();
            let unit = covered_by(members.iter().copied(), &alone.unit);
            let selftest = covered_by(members.iter().copied(), &alone.selftest);
            let (covered, lines) = totals(members);
            Group {
                name,
                covered,
                lines,
                below,
                unit,
                selftest,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        percent(a.covered, a.lines)
            .total_cmp(&percent(b.covered, b.lines))
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

/// The report's text: the summary, the crates, the folders and then every file, lowest coverage first.
fn table(files: &[FileCoverage], alone: &Alone, run: &Run) -> String {
    let mut text = summary(files, alone, run);
    for (title, key) in [("crate", crate_of as fn(&str) -> String), ("folder", group)] {
        let _ = writeln!(
            text,
            "\nby {title}, lowest first\n percent  covered    lines  <{FILE_GOAL}%     unit  selftest  {title}"
        );
        for row in grouped(files, alone, key) {
            let _ = writeln!(
                text,
                "{:>7.1}  {:>7}  {:>7}  {:>5}  {:>7.1}  {:>8.1}  {}",
                percent(row.covered, row.lines),
                row.covered,
                row.lines,
                row.below,
                percent(row.unit, row.lines),
                percent(row.selftest, row.lines),
                row.name
            );
        }
    }
    let _ = writeln!(
        text,
        "\nby file, lowest first\n percent  covered    lines     unit  selftest  file"
    );
    for file in sorted(files) {
        let unbuilt = if file.built {
            ""
        } else {
            "  (in no instrumented program)"
        };
        let _ = writeln!(
            text,
            "{:>7.1}  {:>7}  {:>7}  {:>7.1}  {:>8.1}  {}{unbuilt}",
            file.percent(),
            file.covered,
            file.lines,
            percent(covered_by([file], &alone.unit), file.lines),
            percent(covered_by([file], &alone.selftest), file.lines),
            file.file
        );
    }
    text
}

/// Fails when the total or any file is below its goal, naming them, and when `cargo test` alone covers the app crate
/// other than `floor` says, naming the value to put in.
fn verdict(files: &[FileCoverage], alone: &Alone, floor: f64) -> Result<(), String> {
    let (covered, lines) = totals(files);
    let mut problems = Vec::new();
    let total = percent(covered, lines);
    if total < TOTAL_GOAL {
        problems.push(format!("the total is {total:.1} %, below {TOTAL_GOAL} %"));
    }
    let below: Vec<String> = sorted(files)
        .into_iter()
        .filter(|file| file.percent() < FILE_GOAL)
        .map(|file| format!("  {:>5.1} %  {}", file.percent(), file.file))
        .collect();
    if !below.is_empty() {
        problems.push(format!(
            "{} files are below {FILE_GOAL} %:\n{}",
            below.len(),
            below.join("\n")
        ));
    }
    problems.extend(floor_problem(app_unit(files, alone), floor));
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// The ratchet on the app crate's coverage by `cargo test` alone, `covered` of `lines`, against `floor`, both to one
/// decimal: below it is a regression, above it must raise it.
fn floor_problem((covered, lines): (u64, u64), floor: f64) -> Option<String> {
    let measured = (percent(covered, lines) * 10.0).floor() / 10.0;
    let floor = (floor * 10.0).round() / 10.0;
    let what = format!("cargo test alone covers {measured:.1} % of the app crate's src/ ({covered} of {lines} lines)");
    if measured < floor {
        Some(format!(
            "{what}, below its floor {floor:.1} %: test the new logic with unit tests"
        ))
    } else if measured > floor {
        Some(format!(
            "{what}, above its floor {floor:.1} %: raise APP_UNIT_FLOOR in xtask/src/coverage.rs to {measured:.1}, so it \
             cannot fall again"
        ))
    } else {
        None
    }
}

#[cfg(test)]
#[path = "coverage_tests.rs"]
mod run_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, covered: u64, lines: u64) -> FileCoverage {
        FileCoverage {
            file: name.to_string(),
            covered,
            lines,
            built: true,
        }
    }

    #[test]
    fn the_toolchain_is_the_channel_of_the_toolchain_file() {
        assert_eq!(toolchain("[toolchain]\nchannel = \"1.99.1\"\n").unwrap(), "1.99.1");
        assert!(toolchain("[toolchain]\nprofile = \"minimal\"\n").is_err());
        let file = files::read_to_string(root().join("rust-toolchain.toml")).unwrap();
        assert!(toolchain(&file).is_ok());
    }

    #[test]
    fn show_env_lines_become_variables_with_their_quotes_removed() {
        let shown = "LLVM_PROFILE_FILE='/a b/dango-%p-%8m.profraw'\nRUSTC_WRAPPER=/opt/homebrew/bin/cargo-llvm-cov\n\
                     FLAGS='-C\u{1f}it'\\''s'\ninfo: not a variable\n";
        assert_eq!(
            parse_shown(shown),
            [
                ("LLVM_PROFILE_FILE".to_string(), "/a b/dango-%p-%8m.profraw".to_string()),
                (
                    "RUSTC_WRAPPER".to_string(),
                    "/opt/homebrew/bin/cargo-llvm-cov".to_string()
                ),
                ("FLAGS".to_string(), "-C\u{1f}it's".to_string()),
            ]
        );
    }

    #[test]
    fn the_export_gives_lines_per_file() {
        let json = br#"{"type": "llvm.coverage.json.export", "data": [{"files": [
            {"filename": "/r/src/a.rs", "summary": {"lines": {"count": 10, "covered": 7, "percent": 70.0}}}
        ], "totals": {}}]}"#;
        let files = parse_export(json).unwrap();
        assert_eq!(files.get(Path::new("/r/src/a.rs")), Some(&(7, 10)));
        assert!(parse_export(b"{}").is_err());
    }

    #[test]
    fn files_group_by_crate_and_app_folder() {
        assert_eq!(group("crates/ide-git/src/lib.rs"), "crates/ide-git");
        assert_eq!(group("src/editor/mod.rs"), "src/editor");
        assert_eq!(group("src/selftest/git/stash.rs"), "src/selftest");
        assert_eq!(group("src/menu.rs"), "src");
        assert_eq!(group("xtask/src/main.rs"), "xtask");
    }

    #[test]
    fn test_modules_build_scripts_and_fuzz_entry_points_are_not_counted() {
        for name in ["tests.rs", "view_tests.rs", "a-tests.rs", "build.rs", "fuzzing.rs"] {
            assert!(is_not_counted(name), "{name}");
        }
        for name in ["contests.rs", "tests_support.rs", "lib.rs"] {
            assert!(!is_not_counted(name), "{name}");
        }
    }

    #[test]
    fn self_test_code_and_the_acp_test_agent_are_not_counted() {
        for file in [
            "src/selftest.rs",
            "src/selftest/git/stash.rs",
            "src/debug/selftest.rs",
            "crates/ide-ui-dictation/src/hub/selftest.rs",
            "crates/ide-ui-terminal/src/inspect.rs",
            "crates/ide-acp/src/test_agent.rs",
            "crates/ide-acp/src/bin/acp-record.rs",
        ] {
            assert!(is_test_only(file), "{file}");
        }
        for file in [
            "xtask/src/selftest.rs",
            "src/debug/session.rs",
            "crates/ide-acp/src/session.rs",
        ] {
            assert!(!is_test_only(file), "{file}");
        }
    }

    #[test]
    fn dep_info_names_the_compiled_sources() {
        let text = "/b/debug/deps/libx.rlib: /r/src/lib.rs /r/src/a\\ b.rs /r/README.md\n\n/r/src/lib.rs:\n\
                    # env-dep:CARGO_PKG_NAME=x\n";
        let sources: Vec<PathBuf> = dep_info_sources(text).collect();
        assert_eq!(
            sources,
            [
                PathBuf::from("/r/src/lib.rs"),
                PathBuf::from("/r/src/a b.rs"),
                PathBuf::from("/r/src/lib.rs")
            ]
        );
    }

    #[test]
    fn a_file_without_code_counts_as_covered_and_comments_are_no_code() {
        assert!((percent(0, 0) - 100.0).abs() < f64::EPSILON);
        assert!((percent(1, 4) - 25.0).abs() < f64::EPSILON);
        assert_eq!(code_lines("//! doc\n\nfn a() {\n    // why\n}\n"), 2);
    }

    #[test]
    fn the_check_names_the_files_below_their_goal_and_a_low_total() {
        let none = Alone::default();
        assert!(verdict(&[file("src/a.rs", 95, 100), file("src/b.rs", 10, 10)], &none, 0.0).is_ok());
        let failed = verdict(&[file("src/a.rs", 40, 100), file("src/b.rs", 10, 10)], &none, 0.0).unwrap_err();
        assert!(failed.contains("the total is 45.5 %"), "{failed}");
        assert!(
            failed.contains("1 files are below 50 %") && failed.contains("40.0 %  src/a.rs"),
            "{failed}"
        );
        assert!(!failed.contains("src/b.rs"), "{failed}");
    }

    #[test]
    fn the_app_crate_is_held_to_its_floor_of_unit_test_coverage_both_ways() {
        let files = [file("src/a.rs", 90, 100), file("crates/ide-x/src/lib.rs", 100, 100)];
        let alone = Alone {
            unit: Lines::from([
                ("src/a.rs".to_string(), 34),
                ("crates/ide-x/src/lib.rs".to_string(), 100),
            ]),
            selftest: Lines::new(),
        };
        assert_eq!(app_unit(&files, &alone), (34, 100));
        assert!(verdict(&files, &alone, 34.0).is_ok());
        let below = verdict(&files, &alone, 40.0).unwrap_err();
        assert!(
            below.contains("covers 34.0 % of the app crate's src/ (34 of 100 lines), below its floor 40.0 %"),
            "{below}"
        );
        let above = verdict(&files, &alone, 0.0).unwrap_err();
        assert!(
            above.contains("raise APP_UNIT_FLOOR in xtask/src/coverage.rs to 34.0"),
            "{above}"
        );
        assert_eq!(
            floor_problem((3427, 10_000), 34.2),
            None,
            "measured to one decimal, rounded down"
        );
    }

    #[test]
    fn the_table_lists_files_lowest_first_under_their_folders() {
        let files = [
            file("src/b.rs", 9, 10),
            file("src/a.rs", 1, 10),
            file("crates/ide-x/src/lib.rs", 5, 10),
        ];
        let run = Run {
            scenarios: vec!["probe".into()],
            failed: Vec::new(),
            timings: vec![("total", Duration::from_secs(75))],
        };
        let alone = Alone {
            unit: Lines::from([("src/b.rs".to_string(), 3)]),
            selftest: Lines::from([("src/b.rs".to_string(), 9)]),
        };
        let text = table(&files, &alone, &run);
        assert!(text.contains("cargo test alone 10.0 % (3 of 30 lines)\n"), "{text}");
        assert!(text.contains("self-test alone 30.0 % (9 of 30 lines)\n"), "{text}");
        assert!(
            text.contains("cargo test alone in the app crate's src/ 15.0 % (3 of 20 lines)"),
            "{text}"
        );
        assert!(
            text.contains("   90.0        9       10     30.0      90.0  src/b.rs\n"),
            "{text}"
        );
        assert!(text.contains("\nby crate, lowest first\n"), "{text}");
        assert!(
            text.contains("   50.0       10       20      1     15.0      45.0  src\n"),
            "{text}"
        );
        let a = text.find("src/a.rs").unwrap();
        let x = text.find("crates/ide-x/src/lib.rs").unwrap();
        let b = text.find("src/b.rs").unwrap();
        assert!(a < x && x < b, "{text}");
        assert!(text.contains("total 1:15 min"), "{text}");
        assert!(
            text.starts_with("line coverage 50.0 % (15 of 30 lines) in 3 files, 1 below 50 %"),
            "{text}"
        );
    }
}
