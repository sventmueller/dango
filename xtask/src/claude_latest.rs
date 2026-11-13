//! `cargo xtask claude-latest`, the nightly check against the newest Claude CLI (R7): `@anthropic-ai/claude-code` at
//! its `latest` tag, installed with npm into `target/claude-latest/` and never onto the PATH, then the contract tests
//! of `ide-claude` against it. Without a sign-in the CLI must keep the recorded contract of a signed-out session, which
//! needs no account and costs nothing; `--signed-in` adds one short turn with haiku, which needs a sign-in (this Mac's,
//! or `CLAUDE_CODE_OAUTH_TOKEN` or `ANTHROPIC_API_KEY` in the environment) and costs a few tokens.

use std::path::Path;
use std::process::Command;

use crate::integration::{Outcome, judge_test, shown};
use crate::*;

/// What npm installs.
const CLI_PACKAGE: &str = "@anthropic-ai/claude-code@latest";

/// The CLI version the `recorded_*` fixtures come from (docs/claude-mapping.md, "Recordings").
const RECORDED: &str = "2.1.319";

/// The test in `crates/ide-claude/tests/session.rs` that needs no sign-in.
const SIGNED_OUT: &str = "the_cli_without_a_sign_in_keeps_the_recorded_contract";

/// The test in the same file that has the CLI answer a prompt.
const SIGNED_IN: &str = "the_real_cli_answers_a_prompt";

/// Installs the newest CLI and runs the contract tests against it; the error names the tests that failed.
pub(crate) fn claude_latest(args: &[String]) -> Result<(), String> {
    let [signed_in] = flags(args, ["--signed-in"])?;
    let prefix = target_dir().join("claude-latest");
    if prefix.exists() {
        files::remove_dir_all(&prefix)?;
    }
    files::create_dir_all(&prefix)?;
    run(&mut install(&prefix))?;
    let cli = prefix.join("node_modules/.bin/claude");
    let version = stdout(Command::new(&cli).arg("--version"))?;
    println!(
        "newest CLI: {} (the fixtures were recorded with {RECORDED})",
        version.trim()
    );
    let mut failed = Vec::new();
    for test in tests(signed_in) {
        println!("-> {test}");
        let (success, lines) = shown(&mut contract_test(&cli, test))?;
        match judge_test(success, &lines) {
            Outcome::Passed => println!("   {test}: passed"),
            Outcome::Skipped(why) => println!("   {test}: skipped: {why}"),
            Outcome::Failed(why) => {
                println!("   {test}: FAILED: {why}");
                failed.push(test);
            }
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "claude {} breaks the contract the app relies on: {}",
            version.trim(),
            failed.join(", ")
        ))
    }
}

/// The tests to run, the one that uses an account only when asked.
fn tests(signed_in: bool) -> Vec<&'static str> {
    std::iter::once(SIGNED_OUT)
        .chain(signed_in.then_some(SIGNED_IN))
        .collect()
}

fn install(prefix: &Path) -> Command {
    let mut command = Command::new("npm");
    command.arg("install").arg("--prefix").arg(prefix).args([
        "--no-save",
        "--no-package-lock",
        "--no-audit",
        "--no-fund",
        CLI_PACKAGE,
    ]);
    command
}

/// One ignored test of `ide-claude` alone, with `cli` as the CLI it starts.
fn contract_test(cli: &Path, test: &str) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .current_dir(root())
        .args([
            "test",
            "-p",
            "ide-claude",
            "--test",
            "session",
            "--",
            "--ignored",
            "--exact",
            test,
        ])
        .args(["--nocapture", "--test-threads", "1"])
        .env("CLAUDE_BIN", cli);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_signed_out_contract_runs_unless_asked() {
        assert_eq!(tests(false), [SIGNED_OUT]);
        assert_eq!(tests(true), [SIGNED_OUT, SIGNED_IN]);
    }

    #[test]
    fn the_tests_it_names_are_there() {
        let source = files::read_to_string(root().join("crates/ide-claude/tests/session.rs")).unwrap();
        for test in tests(true) {
            assert!(source.contains(&format!("fn {test}()")), "{test}");
        }
        let mapping = files::read_to_string(root().join("docs/claude-mapping.md")).unwrap();
        assert!(mapping.contains(&format!("come from CLI {RECORDED}")), "{RECORDED}");
    }

    #[test]
    fn the_cli_goes_into_its_own_folder() {
        let command = install(Path::new("/tmp/x"));
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(&args[..3], ["install", "--prefix", "/tmp/x"]);
        assert_eq!(args.last().copied(), Some(CLI_PACKAGE.as_ref()));
        assert!(!args.iter().any(|a| *a == "-g" || *a == "--global"));
    }

    #[test]
    fn a_contract_test_runs_alone_against_the_installed_cli() {
        let command = contract_test(Path::new("/tmp/x/claude"), SIGNED_OUT);
        let args: Vec<_> = command.get_args().collect();
        assert!(args.windows(2).any(|w| w == ["--exact", SIGNED_OUT]));
        assert!(args.contains(&"--ignored".as_ref()));
        let envs: Vec<_> = command.get_envs().collect();
        assert_eq!(envs, [("CLAUDE_BIN".as_ref(), Some("/tmp/x/claude".as_ref()))]);
    }
}
