//! The request for a commit message (E88): what it holds, and the secrets check over exactly its text, with gitleaks
//! missing and, where this Mac has it, present.

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

mod common;

use std::path::{Path, PathBuf};

use common::{Scratch, write};
use ide_git::{Git, Place, RequestScan, SUBJECTS};
use ide_process::Supervisor;

/// An access key in the shape gitleaks' `aws-access-token` rule looks for; it opens nothing.
const FAKE_KEY: &str = "AKIAQYLPMN5HHHFPZAM2";

/// git with gitleaks on its PATH, or `None` where this Mac has no gitleaks.
fn with_gitleaks(scratch: &Scratch) -> Option<Git> {
    let dir = ["/opt/homebrew/bin", "/usr/local/bin"]
        .into_iter()
        .find(|d| Path::new(d).join("gitleaks").exists())?;
    let mut environment = scratch.environment();
    environment[0].1 = format!("{dir}:/usr/bin:/bin");
    Some(Git::new(&Supervisor::new(), environment))
}

#[test]
fn the_request_holds_what_is_staged_or_else_every_change_with_branch_and_subjects() {
    let scratch = Scratch::new("message-input");
    let repository = scratch.repository("r", &[("a.txt", "a\n"), ("b.txt", "b\n")]);
    for n in 1..=SUBJECTS + 2 {
        write(repository.root(), "a.txt", &format!("a{n}\n"));
        scratch.raw(
            repository.root(),
            &["commit", "--quiet", "-am", &format!("Subject {n}")],
        );
    }
    write(repository.root(), "b.txt", "b unstaged\n");
    write(repository.root(), "new.txt", "brand new\n");

    let all = repository.message_request().unwrap();
    assert!(!all.staged());
    assert_eq!(all.files(), 2);
    let text = all.text();
    assert!(text.contains("Branch: main"), "{text}");
    assert!(text.contains("+b unstaged") && text.contains("+brand new"), "{text}");
    assert!(text.contains(&format!("- Subject {}", SUBJECTS + 2)));
    assert!(
        text.contains("- Subject 3") && !text.contains("- Subject 2\n"),
        "{text}"
    );

    write(repository.root(), "a.txt", "a staged\n");
    repository.stage(&["a.txt"]).unwrap();
    let staged = repository.message_request().unwrap();
    assert!(staged.staged());
    assert_eq!(staged.files(), 1);
    assert!(staged.text().contains("+a staged") && !staged.text().contains("b unstaged"));
}

#[test]
fn without_gitleaks_nothing_may_go_out() {
    let scratch = Scratch::new("message-no-gitleaks");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    write(repository.root(), "a.txt", &format!("key = {FAKE_KEY}\n"));
    let request = repository.message_request().unwrap();
    assert_eq!(repository.scan_request(request).unwrap(), RequestScan::NotInstalled);
}

#[test]
fn a_secret_only_in_unstaged_changes_is_found_with_its_file_and_line() {
    let scratch = Scratch::new("message-secret");
    let Some(git) = with_gitleaks(&scratch) else {
        eprintln!("gitleaks is not installed here; only the missing case is checked");
        return;
    };
    scratch.repository("r", &[("config.env", "one\ntwo\nthree\n")]);
    let repository = git.discover(&scratch.path("r")).unwrap();
    assert!(matches!(repository.scan_staged().unwrap(), ide_git::SecretScan::Clean));
    write(
        repository.root(),
        "config.env",
        &format!("one\ntwo\naws_access_key_id = {FAKE_KEY}\n"),
    );
    assert!(matches!(repository.scan_staged().unwrap(), ide_git::SecretScan::Clean));
    let request = repository.message_request().unwrap();
    assert!(!request.staged());
    match repository.scan_request(request).unwrap() {
        RequestScan::Found(findings) => {
            assert_eq!(findings.len(), 1);
            assert_eq!(
                findings[0].place,
                Place::File {
                    path: PathBuf::from("config.env"),
                    line: 3
                }
            );
            assert!(!format!("{findings:?}").contains(FAKE_KEY));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_checked_text_stays_as_checked_when_the_file_changes_after() {
    let scratch = Scratch::new("message-changed-after");
    let Some(git) = with_gitleaks(&scratch) else {
        eprintln!("gitleaks is not installed here; only the missing case is checked");
        return;
    };
    scratch.repository("r", &[("a.txt", "a\n")]);
    let repository = git.discover(&scratch.path("r")).unwrap();
    write(repository.root(), "a.txt", "checked line\n");
    let request = repository.message_request().unwrap();
    let built = request.text().to_string();
    let RequestScan::Clean(checked) = repository.scan_request(request).unwrap() else {
        panic!("the change holds no secret");
    };
    write(repository.root(), "a.txt", &format!("aws_access_key_id = {FAKE_KEY}\n"));
    assert_eq!(checked.text(), built);
    assert!(checked.text().contains("+checked line") && !checked.text().contains(FAKE_KEY));
}
