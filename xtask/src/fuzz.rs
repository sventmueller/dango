//! The crates' fuzz targets (E95): each runs for a fixed time with the nightly that cargo-fuzz needs, its corpus
//! grows in `target/fuzz/corpus/<target>`, and a crash stops the run, leaving the input in the crate's
//! `fuzz/artifacts/`.

use std::path::PathBuf;
use std::process::Command;

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
    let seconds = match std::env::var("FUZZ_SECONDS") {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| format!("FUZZ_SECONDS={value} is no number of seconds"))?,
        Err(_) => SECONDS,
    };
    if let Some(unknown) = names
        .iter()
        .find(|name| !TARGETS.iter().any(|(_, target, _)| target == name))
    {
        let known: Vec<&str> = TARGETS.iter().map(|(_, target, _)| *target).collect();
        return Err(format!("{unknown} is no fuzz target; there are {}", known.join(", ")));
    }
    let root = crate::root();
    let path = search_path()?;
    let chosen = TARGETS
        .iter()
        .filter(|(_, target, _)| names.is_empty() || names.iter().any(|name| name == target));
    for (krate, target, seeds) in chosen {
        let corpus = crate::target_dir().join("fuzz/corpus").join(target);
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
            .env("CARGO_TARGET_DIR", crate::target_dir().join("fuzz/build"))
            .env_remove("CARGO")
            .env_remove("RUSTC");
        crate::run(&mut cmd)?;
    }
    Ok(())
}

/// `PATH` with the pinned nightly's programs first: cargo-fuzz runs whichever `cargo` comes first, and Homebrew's
/// stable one does without rustup's proxies.
fn search_path() -> Result<std::ffi::OsString, String> {
    let output = Command::new("rustup")
        .args(["which", "--toolchain", NIGHTLY, "cargo"])
        .output()
        .map_err(|e| format!("rustup: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{NIGHTLY} is missing: rustup toolchain install {NIGHTLY} --profile minimal"
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
