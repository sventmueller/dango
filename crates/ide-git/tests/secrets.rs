//! The secrets check with gitleaks missing and, where this Mac has it, present.

mod common;

use std::path::{Path, PathBuf};

use common::{Scratch, write};
use ide_git::{Git, SecretScan};
use ide_process::Supervisor;

/// A token in the shape gitleaks' `github-pat` rule looks for; it grants nothing.
const FAKE_TOKEN: &str = "ghp_1234567890abcdefghijABCDEFGHIJ123456";

#[test]
fn without_gitleaks_on_the_path_the_check_says_so() {
    let scratch = Scratch::new("no-gitleaks");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    write(repository.root(), "a.txt", &format!("token = \"{FAKE_TOKEN}\"\n"));
    repository.stage(&["a.txt"]).unwrap();
    assert_eq!(repository.scan_staged().unwrap(), SecretScan::NotInstalled);
    assert_eq!(repository.scan_unpushed().unwrap(), SecretScan::NotInstalled);
}

#[test]
fn gitleaks_finds_a_staged_token_and_a_committed_one_without_revealing_it() {
    let Some(dir) = ["/opt/homebrew/bin", "/usr/local/bin"]
        .into_iter()
        .find(|d| Path::new(d).join("gitleaks").exists())
    else {
        eprintln!("gitleaks is not installed here; only the missing case is checked");
        return;
    };
    let scratch = Scratch::new("gitleaks");
    let mut environment = scratch.environment();
    environment[0].1 = format!("{dir}:/usr/bin:/bin");
    let git = Git::new(&Supervisor::new(), environment);
    scratch.repository("r", &[("a.txt", "a\n")]);
    let repository = git.discover(&scratch.path("r")).unwrap();
    assert_eq!(repository.scan_staged().unwrap(), SecretScan::Clean);

    write(
        repository.root(),
        "config dir/s p.env",
        &format!("token = \"{FAKE_TOKEN}\"\n"),
    );
    repository.stage(&["config dir/s p.env"]).unwrap();
    match repository.scan_staged().unwrap() {
        SecretScan::Found(findings) => {
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].rule, "github-pat");
            assert_eq!(findings[0].file, PathBuf::from("config dir/s p.env"));
            assert_eq!(findings[0].line, 1);
            assert!(!format!("{findings:?}").contains(FAKE_TOKEN));
        }
        other => panic!("{other:?}"),
    }
    repository.commit("leak", ide_git::CommitOptions::default()).unwrap();
    match repository.scan_unpushed().unwrap() {
        SecretScan::Found(findings) => assert!(findings[0].commit.is_some()),
        other => panic!("{other:?}"),
    }
}
