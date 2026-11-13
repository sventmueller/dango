//! The crates' fuzz targets (E95): each runs for a fixed time with the nightly that cargo-fuzz needs, its corpus
//! grows in `target/fuzz/corpus/<target>`, and a crash stops the run, leaving the input in the crate's
//! `fuzz/artifacts/`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Shell, System};

/// The nightly that builds the targets; changing the date is the only way a newer one is taken (R18).
const NIGHTLY: &str = "nightly-2026-11-04";

/// How long each target runs unless `FUZZ_SECONDS` says otherwise.
const SECONDS: u64 = 60;

/// Each target with its crate and the folders whose files start its corpus.
const TARGETS: &[(&str, &str, &[&str])] = &[
    ("ide-terminal", "scan", &["crates/ide-terminal/fuzz/seeds/scan"]),
    ("ide-terminal", "procargs", &["crates/ide-terminal/fuzz/seeds/procargs"]),
    ("ide-lsp", "messages", &["crates/ide-lsp/fuzz/seeds/messages"]),
    ("ide-acp", "updates", &["crates/ide-acp/fuzz/seeds/updates"]),
    ("ide-git", "diff", &["crates/ide-git/fuzz/seeds/diff"]),
    ("ide-claude", "stream_json", &["crates/ide-claude/tests/fixtures"]),
    ("ide-agent", "stream", &["crates/ide-agent/tests/fixtures"]),
    ("ide-predict", "answer", &["crates/ide-predict/tests/fixtures"]),
    ("ide-dap", "dap_messages", &["crates/ide-dap/fuzz/seeds/dap_messages"]),
    ("ide-dap", "launch_json", &["crates/ide-dap/fuzz/seeds/launch_json"]),
];

/// Runs the targets `names` names, or all of them.
pub(crate) fn fuzz(names: &[String]) -> Result<(), String> {
    fuzz_in(
        &mut System,
        names,
        std::env::var("FUZZ_SECONDS").ok(),
        &crate::target_dir(),
    )
}

/// `fuzz` for `seconds`, the value of `FUZZ_SECONDS`, with `target_dir` in place of the target folder.
fn fuzz_in(sh: &mut dyn Shell, names: &[String], seconds: Option<String>, target_dir: &Path) -> Result<(), String> {
    let seconds = match seconds {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| format!("FUZZ_SECONDS={value} is no number of seconds"))?,
        None => SECONDS,
    };
    if let Some(unknown) = names
        .iter()
        .find(|name| !TARGETS.iter().any(|(_, target, _)| target == name))
    {
        let known: Vec<&str> = TARGETS.iter().map(|(_, target, _)| *target).collect();
        return Err(format!("{unknown} is no fuzz target; there are {}", known.join(", ")));
    }
    let root = crate::root();
    let path = toolchain_path(sh, NIGHTLY)?;
    let chosen = TARGETS
        .iter()
        .filter(|(_, target, _)| names.is_empty() || names.iter().any(|name| name == target));
    for (krate, target, seeds) in chosen {
        let corpus = target_dir.join("fuzz/corpus").join(target);
        std::fs::create_dir_all(&corpus).map_err(|e| format!("{}: {e}", corpus.display()))?;
        println!("fuzz {krate} {target} for {seconds} s");
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&root)
            .env("PATH", &path)
            .args(["fuzz", "run", "--fuzz-dir"])
            .arg(root.join("crates").join(krate).join("fuzz"))
            .arg(target)
            .arg(&corpus)
            .args(seeds.iter().map(|seed| root.join(seed)))
            .arg("--")
            .arg(format!("-max_total_time={seconds}"))
            .env("CARGO_TARGET_DIR", target_dir.join("fuzz/build"))
            .env_remove("CARGO")
            .env_remove("RUSTC");
        sh.run(&mut cmd)?;
    }
    Ok(())
}

/// `PATH` with the programs of rustup's `toolchain` first: cargo subcommands run whichever `cargo` comes first, and
/// Homebrew's does without rustup's proxies and components.
pub(crate) fn toolchain_path(sh: &mut dyn Shell, toolchain: &str) -> Result<std::ffi::OsString, String> {
    let output = sh
        .output(Command::new("rustup").args(["which", "--toolchain", toolchain, "cargo"]))
        .map_err(|e| format!("rustup: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{toolchain} is missing: rustup toolchain install {toolchain} --profile minimal"
        ));
    }
    let cargo = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let toolchain = cargo.parent().ok_or("rustup named no folder")?.to_path_buf();
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .ok_or("neither CARGO_HOME nor HOME is set")?;
    let rest = std::env::var_os("PATH").unwrap_or_default();
    let dirs = [toolchain, home.join("bin")]
        .into_iter()
        .chain(std::env::split_paths(&rest));
    std::env::join_paths(dirs).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::fake::{Fake, env, scratch};

    fn rustup(line: &str) -> (i32, String) {
        if line.starts_with("rustup which") {
            (0, "/rustup/toolchains/nightly/bin/cargo\n".into())
        } else {
            (0, String::new())
        }
    }

    #[test]
    fn each_chosen_target_runs_for_its_time_with_its_seeds_and_the_nightly_first_on_the_path() {
        let target = scratch("fuzz-targets");
        let mut paths = Vec::new();
        let mut sh = Fake::new(|cmd: &Command, line: &str| {
            paths.push(env(cmd, "PATH"));
            rustup(line)
        });
        let names = ["diff".to_string(), "scan".to_string()];
        fuzz_in(&mut sh, &names, Some("5".into()), &target).unwrap();
        let root = crate::root();
        assert_eq!(sh.commands[0], format!("rustup which --toolchain {NIGHTLY} cargo"));
        let runs = sh.of("cargo");
        assert_eq!(runs.len(), 2, "in the order of TARGETS");
        assert_eq!(
            runs[0],
            format!(
                "cargo fuzz run --fuzz-dir {} scan {} {} -- -max_total_time=5",
                root.join("crates/ide-terminal/fuzz").display(),
                target.join("fuzz/corpus/scan").display(),
                root.join("crates/ide-terminal/fuzz/seeds/scan").display()
            )
        );
        assert!(runs[1].contains(" diff ") && runs[1].contains("crates/ide-git/fuzz"));
        assert!(target.join("fuzz/corpus/diff").is_dir());
        drop(sh);
        let path = paths[1].clone().unwrap();
        assert!(path.starts_with("/rustup/toolchains/nightly/bin:"), "{path}");
        std::fs::remove_dir_all(target).unwrap();
    }

    #[test]
    fn a_wrong_time_an_unknown_target_or_a_missing_nightly_stops_before_fuzzing() {
        let target = Path::new("/nonexistent");
        let mut sh = Fake::new(|_: &Command, line: &str| rustup(line));
        let odd = fuzz_in(&mut sh, &[], Some("a minute".into()), target).unwrap_err();
        assert_eq!(odd, "FUZZ_SECONDS=a minute is no number of seconds");
        let unknown = fuzz_in(&mut sh, &["json".to_string()], None, target).unwrap_err();
        assert!(
            unknown.starts_with("json is no fuzz target; there are scan, procargs,"),
            "{unknown}"
        );
        assert!(sh.commands.is_empty());

        let mut without = Fake::new(|_: &Command, _: &str| (1, String::new()));
        let missing = toolchain_path(&mut without, NIGHTLY).unwrap_err();
        assert_eq!(
            missing,
            format!("{NIGHTLY} is missing: rustup toolchain install {NIGHTLY} --profile minimal")
        );
    }

    #[test]
    fn every_target_has_its_fuzz_folder_and_seeds() {
        let root = crate::root();
        for (krate, target, seeds) in TARGETS {
            let fuzz = root.join("crates").join(krate).join("fuzz");
            assert!(
                fuzz.join(format!("fuzz_targets/{target}.rs")).exists(),
                "{krate} {target}"
            );
            for seed in *seeds {
                assert!(root.join(seed).is_dir(), "{seed}");
            }
        }
    }
}
