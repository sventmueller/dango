//! Gate and setup commands of orders (E187, C67, C68, F53): `/bin/sh -c` in the order's worktree under the gate
//! profile of `sandbox-exec`, through the supervisor as `Kind::Gate`, with E186's environment, the run's own
//! temporary folder and private git folder (`ide_git::GateGitDir`), a deadline and an output limit; what the command printed is cut to its end for the builder and
//! the evidence (I8), the tests it ran are counted where its tool's summary can be read, and whatever it left in the
//! worktree fails it. Everything here blocks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::Cancel;
use ide_factory::effect::SandboxRun;
use ide_factory::gates::{CommandOutcome, TestCount};
use ide_factory::workflow::GateSandbox;
use ide_process::Supervisor;
use ide_process::gate::{Gate, Limits, Stopped};
use ide_process::sandbox::{Network, Profile};
use ide_process::supervisor::Ending;

/// How many lines of the end of a gate's output the builder and the evidence get.
const EXCERPT_LINES: usize = 200;

/// What stands in a gate's output for a line with a secret; for the model and the evidence, so not translated.
const REMOVED: &str = "[secret removed]";

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
    /// The links beside the worktree that lead to them.
    pub(super) links: Vec<PathBuf>,
    /// E186's environment.
    pub(super) env: Vec<(String, String)>,
    /// The order's gitleaks rules, frozen from the main checkout (C52), which its output is cleaned with.
    pub(super) rules: ide_git::FrozenRules,
    /// The activity view's name for it.
    pub(super) label: String,
    pub(super) cancel: Cancel,
}

/// Runs `job` until it ended and nothing of its process group is left, and says what it did.
pub(super) fn run(job: &Job, git: Option<&ide_git::Repository>) -> CommandOutcome {
    if let Some(images) = &job.images
        && let Err(error) = std::fs::create_dir_all(images)
    {
        return failed(&error.to_string());
    }
    let temp = match short_temp() {
        Ok(temp) => temp,
        Err(error) => return failed(&error.to_string()),
    };
    // The private copy's `GIT_DIR` would lead a test suite's own repositories into it, so only a configuration that
    // may hold credentials gets one; without a sandbox git reads everything itself.
    let sandboxed = job.run.sandbox != GateSandbox::Off;
    let hidden_config = sandboxed && git.is_none_or(ide_git::GateGitDir::needed);
    let private = git
        .filter(|_| hidden_config)
        .and_then(|repo| private_git(repo, &job.folder.join("git")));
    let neutral = if sandboxed && private.is_none() {
        neutral_git(&temp)
    } else {
        Vec::new()
    };
    let profile = profile(job, &temp, private.as_ref(), hidden_config);
    let env = environment(job, &temp, private.as_ref(), neutral);
    let gate = Gate {
        supervisor: Supervisor::global(),
        env: &env,
        sandbox: (job.run.sandbox != GateSandbox::Off).then_some(&profile),
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
    if let Some(private) = private {
        let _ = private.remove();
    }
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
        excerpt: without_secrets(excerpt(&output), &job.env, &job.rules),
        images: job.images.as_deref().map(pictures).unwrap_or_default(),
    }
}

/// The gate profile for `job` with its temporary folder `temp`: writing in the worktree, the pictures' folder and the
/// private git folder; reading the path dependencies and the shared git folder except its records of remotes, and its
/// configuration when `hidden_config`; local ports and pseudo-terminals when the project relaxed it.
fn profile(job: &Job, temp: &Path, private: Option<&ide_git::GateGitDir>, hidden_config: bool) -> Profile {
    let mut writable = vec![job.worktree.clone()];
    writable.extend(job.images.iter().cloned());
    writable.extend(private.map(|private| private.folder().to_path_buf()));
    let mut readable = job.dependencies.clone();
    readable.extend(job.common_dir.iter().cloned());
    let mut profile = Profile::gate(&writable, temp, &ide_core::paths::home())
        .readable(&readable)
        .links(&job.links)
        .unreadable(&remote_records(job.common_dir.as_deref(), hidden_config));
    if !job.run.network.is_empty() {
        profile = profile.network(Network::On);
    }
    let sockets: Vec<PathBuf> = job.run.sockets.iter().map(PathBuf::from).collect();
    profile = profile.sockets(&sockets);
    if job.run.sandbox == GateSandbox::Relaxed {
        profile = profile.loopback().terminals();
    }
    profile
}

/// A new folder for a run's temporary files below `/private/tmp`, which no other user may enter. Below the run folder
/// in Application Support, the path of a Unix socket a test makes there passes the 104 bytes macOS allows.
fn short_temp() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    let mut last = std::io::Error::other("no free name");
    for attempt in 0..16u32 {
        let path = Path::new("/private/tmp").join(format!("gate-{}-{nanos:x}-{attempt}", std::process::id()));
        match std::fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => last = error,
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

/// E186's environment with the run's own variables, then the temporary folder `temp`, the pictures' folder and the
/// variables that point git at the private git folder or, without one, the `neutral` ones, which the run's variables
/// cannot replace.
fn environment(
    job: &Job,
    temp: &Path,
    private: Option<&ide_git::GateGitDir>,
    neutral: Vec<(String, String)>,
) -> Vec<(String, String)> {
    let mut env = job.env.clone();
    env.extend(job.run.env.iter().map(|(k, v)| (k.clone(), v.clone())));
    let mut own = vec![
        ("TMPDIR".to_string(), temp.to_string_lossy().into_owned()),
        // xcrun keeps its tool cache under the user's temp folder, which the gate cannot write; without its own
        // cache every call of Apple's git, cc or ld through xcrun costs about a second.
        (
            "xcrun_db".to_string(),
            temp.join("xcrun_db").to_string_lossy().into_owned(),
        ),
        // Go fails without a build cache it can write, and its default lies in the home folder, which a gate only reads.
        (
            "GOCACHE".to_string(),
            temp.join("go-build").to_string_lossy().into_owned(),
        ),
    ];
    if let Some(images) = &job.images {
        own.push((IMAGES_VARIABLE.into(), images.to_string_lossy().into_owned()));
    }
    own.extend(private.map_or(neutral, ide_git::GateGitDir::environment));
    env.retain(|(name, _)| !own.iter().any(|(set, _)| set == name));
    env.extend(own);
    env
}

/// The variables of git in a gate without the private folder, with its neutral global configuration in `temp`;
/// without them git fails on the user's configuration, which the sandbox hides.
fn neutral_git(temp: &Path) -> Vec<(String, String)> {
    ide_git::GateGitDir::neutral_environment(&temp.join("git")).unwrap_or_else(|error| {
        ide_core::diag::report("writing a gate's git configuration", error);
        Vec::new()
    })
}

/// The private git folder of a run in `folder`; without it git inside the gate fails, as the shared git folder's
/// configuration is unreadable there.
fn private_git(repo: &ide_git::Repository, folder: &Path) -> Option<ide_git::GateGitDir> {
    match ide_git::GateGitDir::build(repo, folder) {
        Ok(private) => Some(private),
        Err(error) => {
            ide_core::diag::report("making a gate's private git folder", error);
            None
        }
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

/// The files of the shared git folder `common_dir` that can hold a remote's address with its token: the record of the
/// last fetch and the reflogs, whose entry for the clone names where it came from, and with `config` the
/// configuration.
fn remote_records(common_dir: Option<&Path>, config: bool) -> Vec<PathBuf> {
    let names: &[&str] = if config {
        &["config", "FETCH_HEAD", "logs"]
    } else {
        &["FETCH_HEAD", "logs"]
    };
    common_dir
        .map(|dir| names.iter().map(|name| dir.join(name)).collect())
        .unwrap_or_default()
}

/// The gitleaks rules a gate's output is cleaned with: the order's frozen ones in `frozen`, or before the order was
/// frozen, those of the main checkout at `root`, which the builder cannot reach either.
pub(super) fn rules(frozen: Option<&Path>, root: &Path) -> ide_git::FrozenRules {
    super::freeze::frozen_rules(frozen.unwrap_or(root))
}

/// `text` with every line gitleaks finds a secret on replaced by [`REMOVED`], before it reaches the builder, its
/// vendor and the evidence (C50), with the order's frozen `rules`, never a configuration in the worktree; withheld
/// whole when the check fails, as it then cannot tell.
fn without_secrets(text: String, env: &[(String, String)], rules: &ide_git::FrozenRules) -> String {
    if text.trim().is_empty() {
        return text;
    }
    let git = ide_git::Git::new(Supervisor::global(), env.to_vec());
    match git.scan_text_frozen(&text, rules) {
        Ok(ide_git::SecretScan::Found(findings)) => removed(&text, &findings),
        Ok(ide_git::SecretScan::Clean | ide_git::SecretScan::NotInstalled) => text,
        Err(error) => format!("[output withheld: the secrets check of it failed: {error}]"),
    }
}

/// `text` with the lines of `findings`, counted from 1, replaced by [`REMOVED`].
fn removed(text: &str, findings: &[ide_git::Finding]) -> String {
    text.lines()
        .zip(1u32..)
        .map(|(line, number)| {
            if findings
                .iter()
                .any(|f| (f.line..=f.end_line.max(f.line)).contains(&number))
            {
                REMOVED
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

/// The folder for a gate's pictures as the workflow file names it, a leading `~/` standing for `home` (C69).
pub(super) fn images_folder(images: &str, home: &Path) -> PathBuf {
    match images.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(images),
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
    #[test]
    fn a_runs_temporary_folder_is_short_new_and_closed_to_other_users() {
        use std::os::unix::fs::PermissionsExt;
        let (first, second) = (super::short_temp().unwrap(), super::short_temp().unwrap());
        assert_ne!(first, second);
        assert!(first.as_os_str().len() < 48, "{first:?}");
        let mode = std::fs::metadata(&first).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        for folder in [first, second] {
            std::fs::remove_dir(folder).unwrap();
        }
    }

    use super::*;

    #[test]
    fn a_pictures_folder_under_the_home_folder_is_expanded() {
        let home = Path::new("/Users/me");
        assert_eq!(
            images_folder("~/gate-pictures/o1/5", home),
            PathBuf::from("/Users/me/gate-pictures/o1/5")
        );
        assert_eq!(images_folder("/tmp/pictures", home), PathBuf::from("/tmp/pictures"));
    }

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

    fn finding(line: u32, end_line: u32) -> ide_git::Finding {
        ide_git::Finding {
            rule: "github-pat".into(),
            description: String::new(),
            file: PathBuf::new(),
            line,
            end_line,
            commit: None,
        }
    }

    #[test]
    fn the_lines_of_a_finding_are_removed_and_the_rest_stays() {
        let text = "compiling\nTOKEN=a\n-----BEGIN KEY\nb\n-----END KEY\ndone";
        assert_eq!(
            removed(text, &[finding(2, 2), finding(3, 5)]),
            "compiling\n[secret removed]\n[secret removed]\n[secret removed]\n[secret removed]\ndone"
        );
        assert_eq!(removed(text, &[]), text);
    }

    #[test]
    fn a_token_a_gate_printed_does_not_reach_the_builder() {
        let Some(dir) = ["/opt/homebrew/bin", "/usr/local/bin"]
            .into_iter()
            .find(|d| Path::new(d).join("gitleaks").exists())
        else {
            eprintln!("gitleaks is not installed here; only the removal of found lines is checked");
            return;
        };
        let env = vec![("PATH".to_string(), format!("{dir}:/usr/bin:/bin"))];
        let token = ["ghp_", "1234567890abcdefghij", "ABCDEFGHIJ123456"].concat();
        let own = ["ORDER", "_SECRET_", "424242"].concat();
        let output = format!("running 1 test\nleaked = \"{token}\"\nalso {own}\ntest result: ok. 1 passed\n");
        let frozen = ide_core::testing::scratch("gate-frozen-rules");
        std::fs::write(
            frozen.join(".gitleaks.toml"),
            "[extend]\nuseDefault = true\n[[rules]]\nid = \"order-secret\"\nregex = '''ORDER_SECRET_[0-9]{6}'''\n",
        )
        .unwrap();
        let rules = crate::leitstand::freeze::frozen_rules(&frozen);
        let cleaned = without_secrets(output.clone(), &env, &rules);
        assert!(!cleaned.contains(&token) && !cleaned.contains(&own), "{cleaned}");
        assert!(
            cleaned.contains(REMOVED) && cleaned.contains("test result: ok"),
            "{cleaned}"
        );
        let by_default = without_secrets(output.clone(), &env, &ide_git::FrozenRules::default());
        assert!(
            by_default.contains(&own),
            "the order's own rule comes only from its frozen rules: {by_default}"
        );

        std::fs::write(frozen.join(".gitleaks.toml"), "[[rules]\nnot toml").unwrap();
        let withheld = without_secrets(output, &env, &rules);
        assert!(
            withheld.starts_with("[output withheld") && !withheld.contains(&token),
            "{withheld}"
        );
    }

    #[test]
    fn the_shared_git_folders_records_of_remotes_stay_unreadable() {
        assert_eq!(
            remote_records(Some(Path::new("/r/.git")), true),
            ["/r/.git/config", "/r/.git/FETCH_HEAD", "/r/.git/logs"].map(PathBuf::from)
        );
        assert_eq!(
            remote_records(Some(Path::new("/r/.git")), false),
            ["/r/.git/FETCH_HEAD", "/r/.git/logs"].map(PathBuf::from)
        );
        assert!(remote_records(None, true).is_empty());
    }

    #[test]
    fn the_excerpt_is_the_end_of_the_output() {
        let long: String = (0..500).map(|n| format!("line {n}\n")).collect();
        let cut = excerpt(&long);
        assert!(cut.starts_with("line 300") && cut.ends_with("line 499"));
    }
}
