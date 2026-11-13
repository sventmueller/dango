//! Line coverage of the workspace: `cargo test` and the offline selftest scenarios, built with LLVM's instrumentation
//! by cargo-llvm-cov in `target/llvm-cov-target`, reported together in `target/coverage/`.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::*;

/// The least line coverage of all counted files together, in percent, that `--check` accepts.
const TOTAL_GOAL: f64 = 90.0;

/// The least line coverage of each counted file, in percent, that `--check` accepts.
const FILE_GOAL: f64 = 50.0;

/// Scenarios that need the network, an account, a real agent CLI or someone looking at the screen.
const OUTSIDE: &[&str] = &["agent-live", "ci-live", "ci-login", "paste-cli", "show"];

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
    toolchain: String,
    path: OsString,
    target: PathBuf,
    build_dir: Option<PathBuf>,
    vars: Vec<(String, String)>,
}

impl Instrumented {
    fn prepare() -> Result<Instrumented, String> {
        let toolchain = toolchain(&files::read_to_string(root().join("rust-toolchain.toml"))?)?;
        let path = crate::fuzz::toolchain_path(&toolchain)?;
        let mut env = Instrumented {
            toolchain,
            path,
            target: target_dir().join("llvm-cov-target"),
            build_dir: None,
            vars: Vec::new(),
        };
        env.vars = env.show_env()?;
        // cargo cannot see the flags cargo-llvm-cov's wrapper adds, so a shared build folder would mix instrumented
        // and plain artifacts under the same names.
        if let Some((_, dir)) = env.vars.iter().find(|(key, _)| key == "CARGO_LLVM_COV_BUILD_DIR")
            && !Path::new(dir).starts_with(&env.target)
        {
            env.build_dir = Some(PathBuf::from(format!("{dir}-coverage")));
            env.vars = env.show_env()?;
        }
        Ok(env)
    }

    fn show_env(&self) -> Result<Vec<(String, String)>, String> {
        let shown = stdout(&mut self.plain(&["llvm-cov", "show-env"]))?;
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
        cmd.current_dir(root())
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

/// Measures, writes the report and, with `--check`, fails below the goals.
pub(crate) fn coverage(args: &[String]) -> Result<(), String> {
    let [check] = flags(args, ["--check"])?;
    let started = Instant::now();
    let root = root();
    let out = target_dir().join("coverage");
    files::create_dir_all(&out)?;
    let env = Instrumented::prepare()?;
    run(&mut env.command(&["llvm-cov", "clean", "--workspace"]))?;

    let mut failed = Vec::new();
    let mut timings = Vec::new();
    let step = Instant::now();
    let tests = ["test", "--workspace", "--no-fail-fast", "--lib", "--bins", "--tests"];
    if run(&mut env.command(&tests)).is_err() {
        failed.push("cargo test".to_string());
    }
    timings.push(("cargo test", step.elapsed()));

    let step = Instant::now();
    let scenarios = offline_scenarios(&files::read_to_string(root.join("src/selftest.rs"))?);
    let mut selftest: Vec<&str> = vec!["xtask", "selftest"];
    selftest.extend(scenarios.iter().map(String::as_str));
    let mut cmd = env.command(&selftest);
    cmd.env("LLVM_PROFILE_FILE", env.target.join("selftest-%p.profraw"));
    if run(&mut cmd).is_err() {
        failed.push("selftest (see its output for the scenarios)".to_string());
    }
    timings.push(("selftest", step.elapsed()));

    let step = Instant::now();
    let export = out.join("llvm-cov.json");
    let export_arg = export.to_string_lossy().into_owned();
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
    cmd.arg(&export_arg).env("LLVM_COV_FLAGS", env.bundle_objects()?);
    run(&mut cmd)?;
    timings.push(("report", step.elapsed()));

    let measured = parse_export(&std::fs::read(&export).map_err(|e| format!("{}: {e}", export.display()))?)?;
    let compiled = compiled(env.build_dir.as_ref().unwrap_or(&env.target), &root)?;
    let counted = counted(&root, &measured, &compiled)?;
    timings.push(("total", started.elapsed()));
    let run = Run {
        scenarios,
        failed,
        timings,
    };
    files::write(out.join("files.json"), files_json(&counted))?;
    let table = table(&counted, &run);
    files::write(out.join("coverage.txt"), &table)?;
    print!("{}", summary(&counted, &run));
    println!("{}", out.join("coverage.txt").display());
    if check { verdict(&counted) } else { Ok(()) }
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

/// The scenarios `after_launch` in `src/selftest.rs` dispatches, without those in `OUTSIDE`.
fn offline_scenarios(selftest: &str) -> Vec<String> {
    let mut scenarios: Vec<String> = Vec::new();
    let body = selftest
        .lines()
        .skip_while(|line| !line.contains("fn after_launch("))
        .take_while(|line| *line != "}");
    for line in body {
        let Some((names, _)) = line.trim().split_once("=>") else {
            continue;
        };
        if !names.starts_with('"') {
            continue;
        }
        for name in names.split('|') {
            let name = name.trim().trim_matches('"');
            if is_scenario(name) && !OUTSIDE.contains(&name) && !scenarios.iter().any(|s| s == name) {
                scenarios.push(name.to_string());
            }
        }
    }
    scenarios
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
        rust_files(folder, &mut sources)?;
    }
    let mut counted = Vec::new();
    for source in sources {
        let file = source
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
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

fn rust_files(folder: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in files::read_dir(folder)? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            if !NOT_COUNTED.contains(&name.as_ref()) {
                rust_files(&path, found)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") && !is_not_counted(&name) {
            found.push(path);
        }
    }
    Ok(())
}

/// A build script, a file of unit tests by cargo-llvm-cov's convention (`tests.rs`, `*_tests.rs`, `*-tests.rs`),
/// which its report leaves out, or a crate's entry points for its fuzz targets, built only with the feature `fuzzing`.
fn is_not_counted(name: &str) -> bool {
    matches!(name, "build.rs" | "tests.rs" | "fuzzing.rs") || name.ends_with("_tests.rs") || name.ends_with("-tests.rs")
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

fn files_json(files: &[FileCoverage]) -> String {
    let (covered, lines) = totals(files);
    let files: Vec<serde_json::Value> = sorted(files)
        .iter()
        .map(|file| {
            serde_json::json!({
                "file": file.file,
                "covered": file.covered,
                "lines": file.lines,
                "percent": (file.percent() * 100.0).round() / 100.0,
                "built": file.built,
            })
        })
        .collect();
    let json = serde_json::json!({
        "total": {"covered": covered, "lines": lines, "percent": (percent(covered, lines) * 100.0).round() / 100.0},
        "goals": {"total": TOTAL_GOAL, "file": FILE_GOAL},
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

fn summary(files: &[FileCoverage], run: &Run) -> String {
    let (covered, lines) = totals(files);
    let below = files.iter().filter(|file| file.percent() < FILE_GOAL).count();
    let mut text = format!(
        "line coverage {:.1} % ({covered} of {lines} lines) in {} files, {below} below {FILE_GOAL} %; goals {TOTAL_GOAL} \
         % in total, {FILE_GOAL} % in each file\n",
        percent(covered, lines),
        files.len(),
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

fn minutes(took: Duration) -> String {
    let seconds = took.as_secs();
    format!("{}:{:02} min", seconds / 60, seconds % 60)
}

/// The report's text: the summary, the folders and then every file, lowest coverage first.
fn table(files: &[FileCoverage], run: &Run) -> String {
    let mut text = summary(files, run);
    let mut groups: BTreeMap<String, Vec<&FileCoverage>> = BTreeMap::new();
    for file in files {
        groups.entry(group(&file.file)).or_default().push(file);
    }
    let mut groups: Vec<(String, u64, u64, usize)> = groups
        .into_iter()
        .map(|(name, members)| {
            let below = members.iter().filter(|file| file.percent() < FILE_GOAL).count();
            let (covered, lines) = totals(members);
            (name, covered, lines, below)
        })
        .collect();
    groups.sort_by(|a, b| {
        percent(a.1, a.2)
            .total_cmp(&percent(b.1, b.2))
            .then_with(|| a.0.cmp(&b.0))
    });
    let _ = writeln!(
        text,
        "\nby folder, lowest first\n percent  covered    lines  <{FILE_GOAL}%  folder"
    );
    for (name, covered, lines, below) in &groups {
        let _ = writeln!(
            text,
            "{:>7.1}  {covered:>7}  {lines:>7}  {below:>5}  {name}",
            percent(*covered, *lines)
        );
    }
    let _ = writeln!(text, "\nby file, lowest first\n percent  covered    lines  file");
    for file in sorted(files) {
        let unbuilt = if file.built {
            ""
        } else {
            "  (in no instrumented program)"
        };
        let _ = writeln!(
            text,
            "{:>7.1}  {:>7}  {:>7}  {}{unbuilt}",
            file.percent(),
            file.covered,
            file.lines,
            file.file
        );
    }
    text
}

/// Fails when the total or any file is below its goal, naming them.
fn verdict(files: &[FileCoverage]) -> Result<(), String> {
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
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

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
    fn the_offline_scenarios_are_those_of_the_app_without_the_outside_ones() {
        let selftest = files::read_to_string(root().join("src/selftest.rs")).unwrap();
        let scenarios = offline_scenarios(&selftest);
        for expected in ["probe", "a11y", "a11y-rtl", "recovery", "restore", "dictation"] {
            assert!(scenarios.iter().any(|s| s == expected), "{expected} is missing");
        }
        for outside in OUTSIDE {
            assert!(
                !scenarios.iter().any(|s| s == outside),
                "{outside} needs the outside world"
            );
        }
        assert!(scenarios.len() > 30, "{scenarios:?}");
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
        assert!(verdict(&[file("src/a.rs", 95, 100), file("src/b.rs", 10, 10)]).is_ok());
        let failed = verdict(&[file("src/a.rs", 40, 100), file("src/b.rs", 10, 10)]).unwrap_err();
        assert!(failed.contains("the total is 45.5 %"), "{failed}");
        assert!(
            failed.contains("1 files are below 50 %") && failed.contains("40.0 %  src/a.rs"),
            "{failed}"
        );
        assert!(!failed.contains("src/b.rs"), "{failed}");
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
        let text = table(&files, &run);
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
