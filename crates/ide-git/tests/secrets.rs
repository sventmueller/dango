//! The secrets check with gitleaks missing and, where this Mac has it, present.

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

mod common;

use std::path::{Path, PathBuf};

use common::{Scratch, write, write_bytes};
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
    let SecretScan::Found(findings) = git.scan_text_frozen(&text, &FrozenRules::default()).unwrap() else {
        panic!("the frozen rules find the token")
    };
    assert_eq!(findings[0].rule, "github-pat");
    let SecretScan::Found(findings) = git.scan_text(&text).unwrap() else {
        panic!("the check before a prediction ignores the allowlist and the allow comment too")
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

#[test]
fn the_diff_a_secrets_check_reads_shows_binary_files_and_those_marked_not_to_be_diffed() {
    let scratch = Scratch::new("diff-as-text");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let first = repository.head().unwrap().unwrap().to_string();
    let root = repository.root();
    let marks = scratch.path("textconv-ran");
    write(root, ".gitattributes", "hidden.txt -diff\n*.conv diff=conv\n");
    let textconv = format!("touch '{}'; cat", marks.display());
    scratch.raw(root, &["config", "diff.conv.textconv", &textconv]);
    write(root, "hidden.txt", "token = x1\n");
    write_bytes(root, "blob.bin", b"\x00\x00\x01token = x2\n\x00\xff");
    scratch.raw(root, &["add", "--all"]);
    scratch.raw(root, &["commit", "--quiet", "-m", "builder"]);
    write(root, "late.conv", "token = x3\n");
    write_bytes(root, "wide.txt", b"\xff\xfet\x00o\x00k\x00=\x00x\x004\x00\n\x00");

    let added: Vec<Vec<u8>> = repository
        .diff_since_as_text(&first)
        .unwrap()
        .iter()
        .flat_map(|file| file.hunks.iter().flat_map(|h| &h.lines))
        .filter(|line| line.kind == ide_git::LineKind::Added)
        .map(|line| line.content.clone())
        .collect();
    let text: String = added.iter().map(|line| String::from_utf8_lossy(line)).collect();
    for token in ["token = x1", "token = x2", "token = x3"] {
        assert!(text.contains(token), "{token} in {text:?}");
    }
    assert!(
        added.iter().any(|line| line.starts_with(b"\xff\xfet\x00o\x00k")),
        "a UTF-16 file comes as its bytes: {added:?}"
    );
    assert!(!marks.exists(), "no textconv ran");

    let ordinary = repository.diff_since(&first, &[] as &[PathBuf]).unwrap();
    let hidden = ordinary.iter().find(|f| f.path() == Path::new("hidden.txt")).unwrap();
    assert!(
        hidden.binary && hidden.hunks.is_empty(),
        "the ordinary diff hides it: {hidden:?}"
    );
}
