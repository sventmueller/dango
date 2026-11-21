//! The crates' fuzz targets (E95): each runs for a fixed time with the nightly that cargo-fuzz needs, its corpus
//! grows in `target/fuzz/corpus/<target>`, and a crash stops the run, leaving the input in
//! `fuzz/<crate>/artifacts/`. Beside them the property tests (E96) run with a fresh seed and many more cases than in
//! `make check`, which keeps its fixed ones.

use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Shell, System};

/// The nightly that builds the targets; changing the date is the only way a newer one is taken (R18).
const NIGHTLY: &str = "nightly-2026-11-04";

/// How long each target runs unless `FUZZ_SECONDS` says otherwise.
const SECONDS: u64 = 60;

/// Each target with its crate and the folders whose files start its corpus.
const TARGETS: &[(&str, &str, &[&str])] = &[
    ("ide-terminal", "scan", &["fuzz/ide-terminal/seeds/scan"]),
    ("ide-terminal", "procargs", &["fuzz/ide-terminal/seeds/procargs"]),
    ("ide-lsp", "messages", &["fuzz/ide-lsp/seeds/messages"]),
    ("ide-acp", "updates", &["fuzz/ide-acp/seeds/updates"]),
    ("ide-git", "diff", &["fuzz/ide-git/seeds/diff"]),
    ("ide-claude", "stream_json", &["crates/ide-claude/tests/fixtures"]),
    ("ide-agent", "stream", &["crates/ide-agent/tests/fixtures"]),
    ("ide-predict", "answer", &["crates/ide-predict/tests/fixtures"]),
    ("ide-dap", "dap_messages", &["fuzz/ide-dap/seeds/dap_messages"]),
    ("ide-dap", "launch_json", &["fuzz/ide-dap/seeds/launch_json"]),
];

/// The name that picks the property tests instead of a target.
const PROPERTIES: &str = "properties";

/// The crates with property tests, each with the arguments of `cargo test` that pick them: the file
/// `tests/properties.rs`, or the unit test modules `property_tests`.
const PROPERTY_TESTS: &[(&str, &[&str])] = &[
    ("ide-document", &["--test", "properties"]),
    ("ide-git", &["--lib", "--", "property_tests::"]),
    ("ide-project", &["--lib", "--", "property_tests::"]),
    ("ide-terminal", &["--lib", "--", "property_tests::"]),
    ("ide-vim", &["--lib", "--", "property_tests::"]),
];

/// How many cases each property test tries with a fresh seed unless `PROPTEST_CASES` says otherwise.
const CASES: u32 = 10_000;

/// Runs the targets `names` names and the property tests if it names them, or all of it.
pub(crate) fn fuzz(names: &[String]) -> Result<(), String> {
    fuzz_in(
        &mut System,
        names,
        &|name: &str| std::env::var(name).ok(),
        &crate::target_dir(),
    )
}

/// `fuzz` with `env` in place of the environment and `target_dir` in place of the target folder.
fn fuzz_in(
    sh: &mut dyn Shell,
    names: &[String],
    env: &dyn Fn(&str) -> Option<String>,
    target_dir: &Path,
) -> Result<(), String> {
    let seconds = match env("FUZZ_SECONDS") {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| format!("FUZZ_SECONDS={value} is no number of seconds"))?,
        None => SECONDS,
    };
    if let Some(unknown) = names
        .iter()
        .find(|name| *name != PROPERTIES && !TARGETS.iter().any(|(_, target, _)| target == name))
    {
        let known: Vec<&str> = TARGETS.iter().map(|(_, target, _)| *target).collect();
        return Err(format!(
            "{unknown} is no fuzz target; there are {} and {PROPERTIES}",
            known.join(", ")
        ));
    }
    if names.is_empty() || names.iter().any(|name| name == PROPERTIES) {
        properties(sh, env)?;
    }
    let chosen: Vec<_> = TARGETS
        .iter()
        .filter(|(_, target, _)| names.is_empty() || names.iter().any(|name| name == target))
        .collect();
    if chosen.is_empty() {
        return Ok(());
    }
    let root = crate::root();
    let path = toolchain_path(sh, NIGHTLY)?;
    for (krate, target, seeds) in chosen {
        let corpus = target_dir.join("fuzz/corpus").join(target);
        std::fs::create_dir_all(&corpus).map_err(|e| format!("{}: {e}", corpus.display()))?;
        println!("fuzz {krate} {target} for {seconds} s");
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&root)
            .env("PATH", &path)
            .args(["fuzz", "run", "--fuzz-dir"])
            .arg(root.join("fuzz").join(krate))
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

/// Runs the property tests with the seed `PROPTEST_RNG_SEED` names, or a fresh one, and `PROPTEST_CASES` cases each;
/// proptest reads both variables over the fixed values the tests set. A failure names the command that repeats it.
fn properties(sh: &mut dyn Shell, env: &dyn Fn(&str) -> Option<String>) -> Result<(), String> {
    let seed = match env("PROPTEST_RNG_SEED") {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| format!("PROPTEST_RNG_SEED={value} is no seed"))?,
        None => RandomState::new().hash_one(std::time::SystemTime::now()),
    };
    let cases = match env("PROPTEST_CASES") {
        Some(value) => value
            .parse::<u32>()
            .map_err(|_| format!("PROPTEST_CASES={value} is no number of cases"))?,
        None => CASES,
    };
    println!("property tests with PROPTEST_RNG_SEED={seed} PROPTEST_CASES={cases}");
    for (krate, args) in PROPERTY_TESTS {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(crate::root())
            .args(["test", "--package", krate])
            .args(*args)
            .env("PROPTEST_RNG_SEED", seed.to_string())
            .env("PROPTEST_CASES", cases.to_string());
        sh.run(&mut cmd).map_err(|e| {
            format!(
                "{e}\nrepeat it with PROPTEST_RNG_SEED={seed} PROPTEST_CASES={cases} cargo test --package {krate} {}",
                args.join(" ")
            )
        })?;
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

    fn vars(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let pairs = pairs.to_vec();
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        }
    }

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
        fuzz_in(&mut sh, &names, &vars(&[("FUZZ_SECONDS", "5")]), &target).unwrap();
        let root = crate::root();
        assert_eq!(sh.commands[0], format!("rustup which --toolchain {NIGHTLY} cargo"));
        let runs = sh.of("cargo");
        assert_eq!(runs.len(), 2, "in the order of TARGETS");
        assert_eq!(
            runs[0],
            format!(
                "cargo fuzz run --fuzz-dir {} scan {} {} -- -max_total_time=5",
                root.join("fuzz/ide-terminal").display(),
                target.join("fuzz/corpus/scan").display(),
                root.join("fuzz/ide-terminal/seeds/scan").display()
            )
        );
        assert!(runs[1].contains(" diff ") && runs[1].contains("fuzz/ide-git"));
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
        let odd = fuzz_in(&mut sh, &[], &vars(&[("FUZZ_SECONDS", "a minute")]), target).unwrap_err();
        assert_eq!(odd, "FUZZ_SECONDS=a minute is no number of seconds");
        let unknown = fuzz_in(&mut sh, &["json".to_string()], &vars(&[]), target).unwrap_err();
        assert!(
            unknown.starts_with("json is no fuzz target; there are scan, procargs,")
                && unknown.ends_with(" and properties"),
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
            let fuzz = root.join("fuzz").join(krate);
            assert!(
                fuzz.join(format!("fuzz_targets/{target}.rs")).exists(),
                "{krate} {target}"
            );
            for seed in *seeds {
                assert!(root.join(seed).is_dir(), "{seed}");
            }
        }
    }

    fn property_runs() -> Vec<String> {
        PROPERTY_TESTS
            .iter()
            .map(|(krate, args)| format!("cargo test --package {krate} {}", args.join(" ")))
            .collect()
    }

    #[test]
    fn the_property_tests_run_with_the_seed_and_cases_asked_for_without_the_nightly() {
        let mut settings = Vec::new();
        let mut sh = Fake::new(|cmd: &Command, _: &str| {
            settings.push((env(cmd, "PROPTEST_RNG_SEED"), env(cmd, "PROPTEST_CASES")));
            (0, String::new())
        });
        let asked = vars(&[("PROPTEST_RNG_SEED", "42"), ("PROPTEST_CASES", "7")]);
        fuzz_in(&mut sh, &[PROPERTIES.to_string()], &asked, Path::new("/nonexistent")).unwrap();
        assert_eq!(sh.commands, property_runs());
        drop(sh);
        assert!(
            settings.iter().all(|s| *s == (Some("42".into()), Some("7".into()))),
            "{settings:?}"
        );
    }

    #[test]
    fn without_a_seed_every_crate_gets_the_same_fresh_one_and_many_cases_before_the_targets_run() {
        let mut settings = Vec::new();
        let mut sh = Fake::new(|cmd: &Command, line: &str| {
            settings.push((env(cmd, "PROPTEST_RNG_SEED"), env(cmd, "PROPTEST_CASES")));
            rustup(line)
        });
        let target = scratch("fuzz-properties");
        fuzz_in(&mut sh, &[], &vars(&[("FUZZ_SECONDS", "1")]), &target).unwrap();
        let runs = property_runs();
        assert_eq!(sh.commands[..runs.len()], runs[..]);
        assert_eq!(
            sh.commands[runs.len()],
            format!("rustup which --toolchain {NIGHTLY} cargo")
        );
        assert_eq!(sh.of("cargo").len(), runs.len() + TARGETS.len());
        drop(sh);
        let (seed, cases) = settings[0].clone();
        assert!(
            seed.as_deref().is_some_and(|seed| seed.parse::<u64>().is_ok()),
            "{seed:?}"
        );
        assert_eq!(cases, Some(CASES.to_string()));
        assert!(
            settings[..runs.len()]
                .iter()
                .all(|s| *s == (seed.clone(), cases.clone()))
        );
        std::fs::remove_dir_all(target).unwrap();
    }

    #[test]
    fn a_failing_property_names_the_command_that_repeats_it_and_odd_settings_stop_before_it() {
        let mut sh = Fake::new(|_: &Command, _: &str| (101, String::new()));
        let asked = vars(&[("PROPTEST_RNG_SEED", "42"), ("PROPTEST_CASES", "7")]);
        let failed = fuzz_in(&mut sh, &[PROPERTIES.to_string()], &asked, Path::new("/nonexistent")).unwrap_err();
        assert!(
            failed.ends_with(&format!(
                "\nrepeat it with PROPTEST_RNG_SEED=42 PROPTEST_CASES=7 {}",
                property_runs()[0]
            )),
            "{failed}"
        );
        assert_eq!(sh.commands.len(), 1, "the first failure stops the run");

        let mut sh = Fake::new(|_: &Command, _: &str| (0, String::new()));
        let names = [PROPERTIES.to_string()];
        let odd = fuzz_in(
            &mut sh,
            &names,
            &vars(&[("PROPTEST_CASES", "many")]),
            Path::new("/nonexistent"),
        );
        assert_eq!(odd.unwrap_err(), "PROPTEST_CASES=many is no number of cases");
        let odd = fuzz_in(
            &mut sh,
            &names,
            &vars(&[("PROPTEST_RNG_SEED", "x")]),
            Path::new("/nonexistent"),
        );
        assert_eq!(odd.unwrap_err(), "PROPTEST_RNG_SEED=x is no seed");
        assert!(sh.commands.is_empty());
    }

    #[test]
    fn every_crate_with_property_tests_has_them_where_its_arguments_look() {
        fn has_module(dir: &Path) -> bool {
            std::fs::read_dir(dir).unwrap().flatten().any(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    has_module(&path)
                } else {
                    path.ends_with("property_tests.rs")
                }
            })
        }
        let root = crate::root();
        for (krate, args) in PROPERTY_TESTS {
            let dir = root.join("crates").join(krate);
            match *args {
                ["--test", name] => assert!(dir.join(format!("tests/{name}.rs")).is_file(), "{krate}"),
                ["--lib", "--", "property_tests::"] => assert!(has_module(&dir.join("src")), "{krate}"),
                _ => panic!("{krate}: {args:?}"),
            }
        }
    }
}
