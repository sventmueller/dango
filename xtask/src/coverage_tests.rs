//! A whole coverage run over a made-up workspace, with a shell that answers for rustup, cargo-llvm-cov and the
//! instrumented cargo.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::*;
use crate::shell::fake::{Fake, arg, env, scratch};

fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// A workspace with one measured file, one built without code, one in no build and the files that are not counted.
fn workspace(dir: &Path) -> PathBuf {
    let root = dir.join("workspace");
    write(&root.join("rust-toolchain.toml"), "[toolchain]\nchannel = \"1.99.1\"\n");
    write(
        &root.join("src/selftest.rs"),
        "fn after_launch(name: &str) {\n    match name {\n        \"probe\" => probe(),\n        \"show\" => show(),\n    }\n}\n",
    );
    write(&root.join("src/main.rs"), "fn main() {}\n");
    write(&root.join("src/selftest/probe.rs"), "fn probe() {}\n");
    write(&root.join("xtask/src/lib.rs"), "mod a;\n");
    write(
        &root.join("crates/ide-x/src/lib.rs"),
        "//! X.\n\npub fn x() {\n    // why\n    let _ = 1;\n}\n",
    );
    write(&root.join("crates/ide-x/src/tests.rs"), "fn t() {}\n");
    write(&root.join("crates/ide-x/src/fuzz/target.rs"), "fn f() {}\n");
    root
}

/// Every command with the variables a coverage run sets on it.
type Seen = Vec<(String, [Option<String>; 5])>;

#[test]
fn a_run_measures_tests_and_scenarios_and_reports_every_counted_file() {
    let dir = scratch("coverage-run");
    let root = workspace(&dir);
    let target = dir.join("target");
    let shared = dir.join("shared");
    write(&target.join("llvm-cov-target/release/ide"), "program");
    write(
        &PathBuf::from(format!("{}-coverage/debug/deps/xtask.d", shared.display())),
        "/b/libxtask.rlib: xtask/src/lib.rs\n",
    );
    let measured = root.join("src/main.rs");
    let mut seen: Seen = Vec::new();
    let mut sh = Fake::new(|cmd: &Command, line: &str| {
        let keys = [
            "RUSTUP_TOOLCHAIN",
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_BUILD_DIR",
            "LLVM_PROFILE_FILE",
            "LLVM_COV_FLAGS",
        ];
        seen.push((line.to_string(), keys.map(|key| env(cmd, key))));
        match line {
            l if l.starts_with("rustup which") => (0, "/tc/bin/cargo\n".into()),
            "cargo llvm-cov show-env" => (
                0,
                format!(
                    "CARGO_LLVM_COV_BUILD_DIR='{}'\nRUSTFLAGS='-C instrument-coverage'\n",
                    shared.display()
                ),
            ),
            l if l.starts_with("cargo test") => (101, String::new()),
            l if l.starts_with("cargo llvm-cov report") => {
                let export = serde_json::json!({"data": [{"files": [
                    {"filename": measured, "summary": {"lines": {"count": 10, "covered": 8}}}
                ]}]});
                std::fs::write(arg(cmd, Some("--output-path")), export.to_string()).unwrap();
                (0, String::new())
            }
            _ => (0, String::new()),
        }
    });
    let failed = coverage_in(&mut sh, &["--check".to_string()], &root, &target).unwrap_err();
    assert!(failed.starts_with("the total is 61.5 %, below 90 %"), "{failed}");
    assert!(failed.contains("\n    0.0 %  crates/ide-x/src/lib.rs"), "{failed}");

    let lines: Vec<&str> = sh.commands.iter().map(String::as_str).collect();
    assert_eq!(lines[0], "rustup which --toolchain 1.99.1 cargo");
    assert_eq!(&lines[1..3], ["cargo llvm-cov show-env"; 2]);
    assert_eq!(lines[3], "cargo llvm-cov clean --workspace");
    assert_eq!(lines[4], "cargo test --workspace --no-fail-fast --lib --bins --tests");
    assert_eq!(lines[5], "cargo xtask selftest probe", "the outside scenario stays out");
    assert!(lines[6].starts_with("cargo llvm-cov report --workspace --json --summary-only --failure-mode all"));
    drop(sh);
    let instrumented = Some(target.join("llvm-cov-target").display().to_string());
    let own_build = Some(format!("{}-coverage", shared.display()));
    let toolchain = Some("1.99.1".to_string());
    assert_eq!(seen[1].1, [toolchain.clone(), instrumented.clone(), None, None, None]);
    assert_eq!(seen[2].1[2], own_build, "show-env again with a build folder of its own");
    let selftest = &seen[5].1;
    assert_eq!(selftest[..3], [toolchain, instrumented, own_build]);
    let profiles = target.join("llvm-cov-target/selftest-%p.profraw").display().to_string();
    assert_eq!(selftest[3].as_deref(), Some(profiles.as_str()));
    let objects = format!("-object={}", target.join("llvm-cov-target/release/ide").display());
    assert_eq!(seen[6].1[4].as_deref(), Some(objects.as_str()));

    let report = std::fs::read_to_string(target.join("coverage/coverage.txt")).unwrap();
    assert!(
        report.starts_with("line coverage 61.5 % (8 of 13 lines) in 3 files, 1 below 50 %;"),
        "{report}"
    );
    assert!(report.contains("1 files are in no instrumented program and count as uncovered\n"));
    assert!(report.contains("selftest scenarios: probe\n"));
    assert!(report.contains("failed, so the numbers miss what it did not reach: cargo test\n"));
    assert!(report.contains("    0.0        0        3  crates/ide-x/src/lib.rs  (in no instrumented program)\n"));
    assert!(
        report.contains("  100.0        0        0  xtask/src/lib.rs\n"),
        "{report}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(target.join("coverage/files.json")).unwrap()).unwrap();
    assert_eq!(json["total"]["lines"], 13);
    let files: Vec<&str> = json["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    assert_eq!(files, ["crates/ide-x/src/lib.rs", "src/main.rs", "xtask/src/lib.rs"]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_build_folder_inside_the_coverage_target_is_kept_and_objects_need_paths_without_spaces() {
    let dir = scratch("coverage-env");
    let root = workspace(&dir);
    let target = dir.join("target");
    let inside = target.join("llvm-cov-target/build");
    let mut sh = Fake::new(|_: &Command, line: &str| match line {
        l if l.starts_with("rustup which") => (0, "/tc/bin/cargo\n".into()),
        _ => (0, format!("CARGO_LLVM_COV_BUILD_DIR={}\n", inside.display())),
    });
    let env = Instrumented::prepare(&mut sh, &root, &target).unwrap();
    assert_eq!(env.build_dir, None);
    assert_eq!(sh.commands.len(), 2, "show-env once");
    assert_eq!(env.bundle_objects().unwrap(), "", "no bundle was built");
    let plain = env.plain(&["llvm-cov", "show-env"]);
    assert_eq!(plain.get_current_dir(), Some(root.as_path()));
    assert!(
        plain
            .get_envs()
            .any(|(key, value)| key == "RUSTFLAGS" && value.is_none())
    );
    let wrapped = env.command(&["test"]);
    assert_eq!(
        crate::shell::fake::env(&wrapped, "CARGO_LLVM_COV_BUILD_DIR"),
        Some(inside.display().to_string())
    );

    let spaced = Instrumented {
        target: dir.join("with space"),
        ..env
    };
    write(&spaced.target.join("release/ide"), "program");
    let refused = spaced.bundle_objects().unwrap_err();
    assert!(
        refused.ends_with("LLVM_COV_FLAGS splits at spaces, so the path must have none"),
        "{refused}"
    );
    assert!(coverage_in(&mut sh, &["--chek".to_string()], &root, &target).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_workspace_without_a_toolchain_channel_is_no_coverage_run() {
    assert!(toolchain("[toolchain]\nprofile = \"minimal\"\n").is_err());
    let missing = Path::new("/nonexistent/workspace");
    let mut sh = Fake::new(|_: &Command, _: &str| (0, String::new()));
    assert!(Instrumented::prepare(&mut sh, missing, missing).is_err());
    assert!(sh.commands.is_empty());
}
