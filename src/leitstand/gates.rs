//! Gate and setup commands of orders (E187, C67, C68, F53): `/bin/sh -c` in the order's worktree under the gate
//! profile of `sandbox-exec`, through the supervisor as `Kind::Gate`, with E186's environment, the run's own
//! temporary folder, a deadline and an output limit; what the command printed is cut to its end for the builder and
//! the evidence (I8), the tests it ran are counted where its tool's summary can be read, and whatever it left in the
//! worktree fails it. Everything here blocks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::Cancel;
use ide_factory::effect::SandboxRun;
use ide_factory::gates::{CommandOutcome, TestCount};
use ide_process::Supervisor;
use ide_process::gate::{Gate, Limits, Stopped};
use ide_process::sandbox::{Network, Profile};
use ide_process::supervisor::Ending;

/// How many lines of the end of a gate's output the builder and the evidence get.
const EXCERPT_LINES: usize = 200;

/// The variable that names the folder of a gate's pictures for the evidence (C69).
pub(super) const IMAGES_VARIABLE: &str = "GATE_IMAGES_DIR";

/// One run of a gate or setup command.
pub(super) struct Job {
    pub(super) run: SandboxRun,
    /// The worktree it runs in.
    pub(super) worktree: PathBuf,
    /// The run's own folder, for its temporary files.
    pub(super) folder: PathBuf,
    /// Its folder for pictures, outside the worktree.
    pub(super) images: Option<PathBuf>,
    /// The repository's shared git folder, which git in a linked worktree reads.
    pub(super) common_dir: Option<PathBuf>,
    /// The folders of path dependencies beside the repository, which the build reads through the links beside the
    /// worktree (E188).
    pub(super) dependencies: Vec<PathBuf>,
    /// E186's environment.
    pub(super) env: Vec<(String, String)>,
    /// The activity view's name for it.
    pub(super) label: String,
    pub(super) cancel: Cancel,
}

/// Runs `job` until it ended and nothing of its process group is left, and says what it did.
pub(super) fn run(job: &Job, git: Option<&ide_git::Repository>) -> CommandOutcome {
    let temp = job.folder.join("tmp");
    if let Err(error) = std::fs::create_dir_all(&temp) {
        return failed(&error.to_string());
    }
    let mut writable = vec![job.worktree.clone()];
    if let Some(images) = &job.images {
        if let Err(error) = std::fs::create_dir_all(images) {
            return failed(&error.to_string());
        }
        writable.push(images.clone());
    }
    let mut readable = job.dependencies.clone();
    readable.extend(job.common_dir.iter().cloned());
    let mut profile = Profile::gate(&writable, &temp, &ide_core::paths::home()).readable(&readable);
    if !job.run.network.is_empty() {
        profile = profile.network(Network::On);
    }
    let sockets: Vec<PathBuf> = job.run.sockets.iter().map(PathBuf::from).collect();
    profile = profile.sockets(&sockets);
    let mut env = job.env.clone();
    env.extend(job.run.env.iter().map(|(k, v)| (k.clone(), v.clone())));
    env.retain(|(name, _)| name != "TMPDIR");
    env.push(("TMPDIR".into(), temp.to_string_lossy().into_owned()));
    if let Some(images) = &job.images {
        env.push((IMAGES_VARIABLE.into(), images.to_string_lossy().into_owned()));
    }
    let gate = Gate {
        supervisor: Supervisor::global(),
        env: &env,
        sandbox: Some(&profile),
        limits: Limits {
            deadline: Duration::from_millis(job.run.deadline_ms),
            output: job.run.output_limit,
            ..Limits::default()
        },
        cancel: Some(&job.cancel),
    };
    let ran = gate.run(
        Path::new("/bin/sh"),
        &["-c", &job.run.command],
        &job.worktree,
        &job.label,
    );
    let _ = std::fs::remove_dir_all(&temp);
    let Ok(outcome) = ran else {
        return ran
            .err()
            .map_or_else(CommandOutcome::default, |error| failed(&error.to_string()));
    };
    let output = format!("{}{}", outcome.stdout, outcome.stderr);
    CommandOutcome {
        exit: match outcome.ending {
            Ending::Code(code) if outcome.stopped.is_none() => Some(code),
            _ => None,
        },
        timed_out: outcome.stopped == Some(Stopped::Deadline),
        output_cut: outcome.stopped == Some(Stopped::Output),
        tests: count_tests(&output),
        left_behind: git.map(left_behind).unwrap_or_default(),
        duration_ms: u64::try_from(outcome.took.as_millis()).unwrap_or(u64::MAX),
        excerpt: excerpt(&output),
        images: job.images.as_deref().map(pictures).unwrap_or_default(),
    }
}

/// A run that never started, with why in the excerpt.
fn failed(why: &str) -> CommandOutcome {
    CommandOutcome {
        exit: None,
        excerpt: why.to_string(),
        ..CommandOutcome::default()
    }
}

/// The last lines of `output`.
pub(super) fn excerpt(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    lines[lines.len().saturating_sub(EXCERPT_LINES)..].join("\n")
}

/// What `git status` shows in the worktree after a gate, untracked files included and ignored ones not (F53).
fn left_behind(copy: &ide_git::Repository) -> Vec<String> {
    let options = ide_git::StatusOptions {
        untracked: ide_git::Untracked::All,
        ignored: false,
    };
    match copy.status(options) {
        Ok(status) => status
            .entries
            .into_iter()
            .filter(|entry| entry.kind != ide_git::EntryKind::Ignored)
            .map(|entry| entry.path.to_string_lossy().into_owned())
            .collect(),
        Err(error) => vec![format!("git status: {error}")],
    }
}

/// The pictures a gate wrote into its folder.
fn pictures(folder: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|x| ["png", "jpg", "jpeg", "gif", "pdf", "svg"].contains(&&*x.to_string_lossy()))
        })
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

/// The numbers before the words of `words` in `line`, such as `5 passed`, summed per word.
fn numbers(line: &str, words: &[&str]) -> Vec<u32> {
    let tokens: Vec<&str> = line
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '(' || c == ')')
        .filter(|t| !t.is_empty())
        .collect();
    words
        .iter()
        .map(|word| {
            tokens
                .windows(2)
                .filter(|pair| pair[1].trim_end_matches('.') == *word)
                .filter_map(|pair| pair[0].parse::<u32>().ok())
                .sum()
        })
        .collect()
}

/// How many tests ran, failed and were skipped, from the summaries of libtest, nextest, pytest, Jest and Vitest;
/// `None` when the output has none, as for a command whose tool cannot be told (C68).
pub(super) fn count_tests(output: &str) -> Option<TestCount> {
    let mut count = TestCount::default();
    let mut seen = false;
    for line in output.lines().map(str::trim) {
        let summary = line.starts_with("test result:")
            || (line.starts_with("Summary [") && line.contains("tests run"))
            || (line.starts_with('=') && line.ends_with('=') && line.contains(" in "))
            || line.starts_with("Tests:")
            || (line.starts_with("Tests ") && line.contains("passed"));
        if !summary {
            continue;
        }
        seen = true;
        let found = numbers(line, &["passed", "failed", "ignored", "skipped", "todo"]);
        count.ran += found[0] + found[1];
        count.failed += found[1];
        count.skipped += found[2] + found[3] + found[4];
    }
    seen.then_some(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summaries_of_the_usual_tools_are_counted() {
        let cargo = "running 3 tests\ntest result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out\n\
                     test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";
        assert_eq!(
            count_tests(cargo),
            Some(TestCount {
                ran: 3,
                failed: 0,
                skipped: 1
            })
        );
        let nextest = "     Summary [   1.2s] 10 tests run: 9 passed, 1 failed, 2 skipped\n";
        assert_eq!(
            count_tests(nextest).map(|c| (c.ran, c.failed, c.skipped)),
            Some((10, 1, 2))
        );
        let pytest = "===== 4 passed, 1 skipped in 0.12s =====\n";
        assert_eq!(count_tests(pytest).map(|c| (c.ran, c.skipped)), Some((4, 1)));
        let jest = "Tests:       1 failed, 5 passed, 6 total\n";
        assert_eq!(count_tests(jest).map(|c| (c.ran, c.failed)), Some((6, 1)));
        assert_eq!(count_tests("make: Nothing to be done\n"), None);
        let none = "test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured\n";
        assert_eq!(
            count_tests(none).map(|c| c.ran),
            Some(0),
            "only ignored tests count as none run"
        );
    }

    #[test]
    fn the_excerpt_is_the_end_of_the_output() {
        let long: String = (0..500).map(|n| format!("line {n}\n")).collect();
        let cut = excerpt(&long);
        assert!(cut.starts_with("line 300") && cut.ends_with("line 499"));
    }
}
