//! git for the orchestrator of work orders (E192, C53): its commits, merges and worktree commands run none of the
//! repository's hooks, also not those a builder points `core.hooksPath` at, and stop at their deadline; the user's own
//! commits still run them.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{Scratch, write};
use ide_git::{CommitOptions, Error};

/// Installs `hook` in `folder`; it leaves a file named after itself in `marks` when it runs, and refuses.
fn hook(folder: &Path, hook: &str, marks: &Path) {
    std::fs::create_dir_all(folder).unwrap();
    let script = format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marks.join(hook).display());
    let path = folder.join(hook);
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn the_orchestrators_commits_merges_and_worktrees_run_no_hook() {
    let scratch = Scratch::new("unattended-hooks");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let root = repository.root().to_path_buf();
    let marks = scratch.path("marks");
    std::fs::create_dir_all(&marks).unwrap();
    for name in ["pre-commit", "commit-msg", "post-checkout", "pre-merge-commit"] {
        hook(&root.join(".git/hooks"), name, &marks);
    }

    write(&root, "a.txt", "the user's\n");
    repository.stage(&["a.txt"]).unwrap();
    let users = repository.commit("user", CommitOptions::default());
    assert!(
        matches!(users, Err(Error::HookFailed { ref hook, .. }) if hook == "pre-commit"),
        "{users:?}"
    );
    assert!(
        marks.join("pre-commit").exists(),
        "the user's commit still runs the hooks"
    );
    std::fs::remove_file(marks.join("pre-commit")).unwrap();

    let deadline = Instant::now() + Duration::from_secs(120);
    let unattended = repository.unattended(scratch.environment(), deadline);
    unattended.commit("orchestrator", CommitOptions::default()).unwrap();

    hook(&root.join("planted"), "pre-commit", &marks);
    scratch.raw(&root, &["config", "core.hooksPath", "planted"]);
    write(&root, "a.txt", "again\n");
    unattended.stage(&["a.txt"]).unwrap();
    unattended
        .commit("orchestrator again", CommitOptions::default())
        .unwrap();

    let added = unattended.add_worktree(&scratch.path("wt"), "order/x", None).unwrap();
    write(added.root(), "w.txt", "w\n");
    added.stage(&["w.txt"]).unwrap();
    added.commit("in the worktree", CommitOptions::default()).unwrap();
    unattended.merge_branch("order/x", "Merge branch 'order/x'").unwrap();

    let ran: Vec<String> = std::fs::read_dir(&marks)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(ran.is_empty(), "hooks ran: {ran:?}");
    assert!(unattended.is_unattended() && added.is_unattended() && !repository.is_unattended());
}

#[test]
fn the_orchestrators_git_stops_at_its_deadline() {
    let scratch = Scratch::new("unattended-deadline");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let late = repository.unattended(scratch.environment(), Instant::now());
    assert!(matches!(late.head(), Err(Error::TimedOut)));
    assert!(
        repository.head().unwrap().is_some(),
        "the user's git has no such deadline"
    );
}
