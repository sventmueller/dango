//! The secrets check with gitleaks missing and, where this Mac has it, present.

mod common;

use std::path::{Path, PathBuf};

use common::{Scratch, write};
use ide_git::{FrozenRules, Git, SecretScan};
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

#[test]
fn frozen_rules_find_what_the_scanned_folder_allows() {
    let Some(dir) = ["/opt/homebrew/bin", "/usr/local/bin"]
        .into_iter()
        .find(|d| Path::new(d).join("gitleaks").exists())
    else {
        let scratch = Scratch::new("frozen-no-gitleaks");
        let scan = scratch.git().scan_text_frozen("text", &FrozenRules::default()).unwrap();
        assert_eq!(scan, SecretScan::NotInstalled);
        return;
    };
    let scratch = Scratch::new("frozen-rules");
    let mut environment = scratch.environment();
    environment[0].1 = format!("{dir}:/usr/bin:/bin");
    let git = Git::new(&Supervisor::new(), environment);
    let order = scratch.path("order");
    write(
        &order,
        ".gitleaks.toml",
        "[extend]\nuseDefault = true\n[allowlist]\nregexes = ['''ghp_''']\n",
    );
    let text = format!("token = \"{FAKE_TOKEN}\" # gitleaks:allow\n");
    assert_eq!(
        git.scan_text(&order, &text).unwrap(),
        SecretScan::Clean,
        "what the order's own folder allows passes the plain check"
    );
    let SecretScan::Found(findings) = git.scan_text_frozen(&text, &FrozenRules::default()).unwrap() else {
        panic!("the frozen rules find the token")
    };
    assert_eq!(findings[0].rule, "github-pat");

    let main = scratch.path("main");
    write(&main, ".gitleaks.toml", "[extend]\nuseDefault = true\n");
    write(&main, ".gitleaksignore", "");
    let rules = FrozenRules {
        config: Some(main.join(".gitleaks.toml")),
        ignore: Some(main.join(".gitleaksignore")),
    };
    assert!(matches!(
        git.scan_text_frozen(&text, &rules).unwrap(),
        SecretScan::Found(_)
    ));
    assert_eq!(
        git.scan_text_frozen("nothing here\n", &rules).unwrap(),
        SecretScan::Clean
    );
}
