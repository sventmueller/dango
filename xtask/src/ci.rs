//! `cargo xtask ci`, the run that seals (docs/development.md, "CI and deploy"): the formatter, the fingerprint of the
//! working tree, then the advisories of the dependencies, `make check`, coverage with every test and offline self-test,
//! `make perf` and a short fuzzing of every target, stopping at the first failure. Only a complete run of unchanged
//! content mints a seal.

use std::process::Command;
use std::time::{Duration, Instant};

use crate::coverage::minutes;
use crate::seal::{Content, Git, Seal, fingerprint, mint};
use crate::*;

/// The stages after the fingerprint, in order, each with its command.
const STAGES: &[(&str, &[&str])] = &[
    ("advisories", &["cargo", "deny", "--locked", "check", "advisories"]),
    ("check", &["make", "check"]),
    ("coverage", &["cargo", "xtask", "coverage", "--check", "--strict"]),
    ("perf", &["make", "perf"]),
    ("fuzz", &["env", "FUZZ_SECONDS=20", "cargo", "xtask", "fuzz"]),
];

const LOCKED: &str = "the screen is locked: the window self-tests cannot capture their windows then, and the run would \
                      seal this content red for a reason outside the code; unlock the Mac and run again";

/// What runs each stage: `caffeinate` keeps the display awake, since the screen locks when it sleeps.
const AWAKE: &[&str] = &["caffeinate", "-di"];

pub(crate) fn ci(args: &[String]) -> Result<(), String> {
    flags(args, [])?;
    ci_in(&mut System, &Git::at(&root()))
}

/// Whether `ioreg -n Root -d1 -a` says the login session's screen is locked.
fn screen_locked(plist: &str) -> bool {
    plist
        .split_once("<key>CGSSessionScreenIsLocked</key>")
        .is_some_and(|(_, rest)| rest.trim_start().starts_with("<true/>"))
}

fn locked(sh: &mut dyn Shell) -> Result<bool, String> {
    let session = sh.stdout(Command::new("ioreg").args(["-n", "Root", "-d1", "-a"]))?;
    Ok(screen_locked(&session))
}

fn command(git: &Git, args: &[&str]) -> Command {
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]).current_dir(git.dir());
    cmd
}

fn ci_in(sh: &mut dyn Shell, git: &Git) -> Result<(), String> {
    if locked(sh)? {
        return Err(LOCKED.into());
    }
    let started = Instant::now();
    sh.run(&mut command(git, &["cargo", "fmt", "--all"]))?;
    let fp = fingerprint(sh, git, Content::WorkingTree)?;
    println!("ci: content {fp}");
    let mut timings: Vec<(&str, Duration)> = Vec::new();
    let mut failure = None;
    for (stage, args) in STAGES {
        println!("ci: {stage}: {}", args.join(" "));
        let step = Instant::now();
        let result = sh.run(&mut command(git, &[AWAKE, args].concat()));
        timings.push((stage, step.elapsed()));
        let took = minutes(step.elapsed());
        if let Err(e) = result {
            println!("ci: {stage} failed after {took}");
            failure = Some(format!("{stage} failed: {e}"));
            break;
        }
        println!("ci: {stage} passed in {took}");
    }
    let mut took: Vec<String> = timings
        .iter()
        .map(|(stage, took)| format!("{stage} {}", minutes(*took)))
        .collect();
    took.push(format!("total {}", minutes(started.elapsed())));
    println!("ci: took {}", took.join(", "));
    let after = fingerprint(sh, git, Content::WorkingTree)?;
    if after != fp {
        return Err(format!(
            "the content changed during the run, from {fp} to {after}, so it seals nothing; `git status` shows what \
             changed"
        ));
    }
    match failure {
        Some(failed) if locked(sh)? => Err(format!("{failed}\nnothing sealed, since {LOCKED}")),
        Some(failed) => {
            let seal = mint(sh, git, Seal::Red, &fp)?;
            Err(format!("{failed}\nsealed red: {seal}"))
        }
        None => {
            let seal = mint(sh, git, Seal::Green, &fp)?;
            println!("ci: sealed green: {seal}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::shell::fake::Fake;

    const FP: &str = "0123456789abcdef0123456789abcdef01234567";

    fn session(locked: bool) -> String {
        let value = if locked { "<true/>" } else { "<false/>" };
        format!("<dict>\n\t<key>CGSSessionScreenIsLocked</key>\n\t\t{value}\n</dict>\n")
    }

    /// A shell whose screen is locked at the nth look when `locks` says so at n or its last entry, whose stages fail
    /// when their line is `failing`, and whose working tree changes once `drift` ran.
    fn shell(
        locks: &'static [bool],
        failing: &'static str,
        drift: &'static str,
    ) -> Fake<impl FnMut(&Command, &str) -> (i32, String)> {
        let mut drifted = false;
        let mut looks = 0;
        Fake::new(move |_: &Command, line: &str| {
            if line == drift {
                drifted = true;
            }
            match line {
                "ioreg -n Root -d1 -a" => {
                    looks += 1;
                    (0, session(locks[(looks - 1).min(locks.len() - 1)]))
                }
                l if l.ends_with("write-tree") && drifted => (0, FP.replace('0', "f")),
                l if l.ends_with("write-tree") => (0, format!("{FP}\n")),
                l if l.contains("commit-tree") => (0, "c0ffee\n".into()),
                l if l == failing => (2, String::new()),
                _ => (0, String::new()),
            }
        })
    }

    fn run(sh: &mut dyn Shell) -> Result<(), String> {
        ci_in(sh, &Git::at(Path::new("/repo")))
    }

    /// The commands that are no part of a fingerprint.
    fn steps(commands: &[String]) -> Vec<&str> {
        commands
            .iter()
            .map(String::as_str)
            .filter(|line| !line.starts_with("git -C /repo read-tree") && !line.contains("--cached"))
            .filter(|line| !line.ends_with("add --all") && !line.contains("ls-files --stage"))
            .collect()
    }

    #[test]
    fn a_locked_screen_refuses_the_run_before_it_starts() {
        assert!(screen_locked(&session(true)) && !screen_locked(&session(false)));
        assert!(!screen_locked("<dict></dict>"), "no key, no lock");
        let mut sh = shell(&[true], "", "");
        assert_eq!(run(&mut sh).unwrap_err(), LOCKED);
        assert_eq!(sh.commands, ["ioreg -n Root -d1 -a"]);
    }

    #[test]
    fn a_complete_run_formats_first_then_runs_every_stage_and_seals_green() {
        let mut sh = shell(&[false], "", "");
        assert_eq!(run(&mut sh), Ok(()));
        assert_eq!(
            steps(&sh.commands),
            [
                "ioreg -n Root -d1 -a",
                "cargo fmt --all",
                "git -C /repo write-tree",
                "caffeinate -di cargo deny --locked check advisories",
                "caffeinate -di make check",
                "caffeinate -di cargo xtask coverage --check --strict",
                "caffeinate -di make perf",
                "caffeinate -di env FUZZ_SECONDS=20 cargo xtask fuzz",
                "git -C /repo write-tree",
                &format!("git -C /repo commit-tree --no-gpg-sign -m CI seal refs/ci-seals/green/{FP} {FP}"),
                &format!("git -C /repo update-ref refs/ci-seals/green/{FP} c0ffee"),
            ]
        );
    }

    #[test]
    fn the_first_failing_stage_ends_the_run_and_seals_red() {
        let mut sh = shell(&[false], "caffeinate -di cargo xtask coverage --check --strict", "");
        let failed = run(&mut sh).unwrap_err();
        assert!(failed.starts_with("coverage failed: "), "{failed}");
        assert!(
            failed.ends_with(&format!("sealed red: refs/ci-seals/red/{FP}")),
            "{failed}"
        );
        let steps = steps(&sh.commands);
        assert!(!steps.contains(&"caffeinate -di make perf"));
        assert_eq!(
            steps.last().copied(),
            Some(format!("git -C /repo update-ref refs/ci-seals/red/{FP} c0ffee").as_str())
        );
    }

    #[test]
    fn content_that_changed_during_the_run_seals_nothing() {
        for failing in ["", "caffeinate -di make perf"] {
            let mut sh = shell(&[false], failing, "caffeinate -di make check");
            let refused = run(&mut sh).unwrap_err();
            assert!(refused.starts_with("the content changed during the run"), "{refused}");
            assert!(!sh.commands.iter().any(|line| line.contains("update-ref")));
        }
    }

    #[test]
    fn a_stage_that_fails_once_the_screen_locked_seals_nothing() {
        let mut sh = shell(&[false, true], "caffeinate -di make perf", "");
        let refused = run(&mut sh).unwrap_err();
        assert!(refused.starts_with("perf failed: "), "{refused}");
        assert!(
            refused.ends_with(&format!("nothing sealed, since {LOCKED}")),
            "{refused}"
        );
        assert!(!sh.commands.iter().any(|line| line.contains("update-ref")));
    }

    #[test]
    fn a_failing_formatter_seals_nothing() {
        let mut sh = shell(&[false], "cargo fmt --all", "");
        assert!(run(&mut sh).unwrap_err().contains("cargo"));
        assert_eq!(sh.commands.len(), 2);
    }
}
