//! git for the orchestrator of work orders (E192, C53): its commits, merges and worktree commands run none of the
//! repository's hooks, also not those a builder points `core.hooksPath` at, and stop at their deadline; the user's own
//! commits still run them.

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{Scratch, write};
use ide_git::{CommitOptions, Error, Git};
use ide_process::Supervisor;

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

/// Points the worktree at `root`'s `.git` file to a git folder `.x` of its own, whose configuration has a clean filter
/// and a textconv for every file that leave `marks/filter` and `marks/textconv` when they run, as a gate or a builder
/// could (E192).
fn redirect(scratch: &Scratch, root: &Path, marks: &Path) {
    let planted = scratch.path("planted");
    std::fs::create_dir_all(&planted).unwrap();
    scratch.raw(&planted, &["init", "--quiet"]);
    std::fs::rename(planted.join(".git"), root.join(".x")).unwrap();
    let x = root.join(".x");
    let x = x.to_str().unwrap();
    let filter = format!("touch '{}'; cat", marks.join("filter").display());
    let textconv = format!("touch '{}'; cat", marks.join("textconv").display());
    scratch.raw(root, &["--git-dir", x, "config", "filter.evil.clean", &filter]);
    scratch.raw(root, &["--git-dir", x, "config", "diff.evil.textconv", &textconv]);
    write(root, ".gitattributes", "* filter=evil diff=evil\n");
    write(root, ".git", "gitdir: .x\n");
}

#[test]
fn a_redirected_worktree_points_the_orchestrators_git_nowhere_else() {
    let scratch = Scratch::new("unattended-redirect");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let marks = scratch.path("marks");
    std::fs::create_dir_all(&marks).unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let unattended = repository.unattended(scratch.environment(), deadline);
    let copy = unattended.add_worktree(&scratch.path("wt"), "order/x", None).unwrap();
    let admin = copy.git_dir().to_path_buf();
    assert!(admin.starts_with(repository.common_dir().canonicalize().unwrap()));

    redirect(&scratch, copy.root(), &marks);
    write(copy.root(), "a.txt", "changed by the builder\n");
    copy.status(ide_git::StatusOptions::default()).unwrap();
    copy.diff_since_as_text("HEAD").unwrap();
    copy.diff(ide_git::DiffSource::Unstaged, &["a.txt"]).unwrap();
    copy.stage_all().unwrap();
    copy.commit("orchestrator", CommitOptions::default()).unwrap();

    let bound = Git::new(&Supervisor::new(), scratch.environment())
        .unattended(scratch.environment(), deadline)
        .bound_to(repository.root());
    let found = bound.discover(copy.root()).unwrap();
    assert_eq!(found.git_dir(), admin, "found through the shared git folder's records");
    found.diff_since_as_text("HEAD~1").unwrap();
    assert!(
        matches!(
            bound.discover(&scratch.path("planted")),
            Err(Error::NotARepository { .. })
        ),
        "a bound git finds nothing outside its repository"
    );
    let loose = Git::new(&Supervisor::new(), scratch.environment()).unattended(scratch.environment(), deadline);
    assert!(
        matches!(loose.discover(copy.root()), Err(Error::NotARepository { .. })),
        "a git folder inside the worktree is not taken"
    );

    let ran: Vec<String> = std::fs::read_dir(&marks)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(ran.is_empty(), "the planted configuration ran: {ran:?}");
    let log = scratch.raw(repository.root(), &["log", "--format=%s", "-1", "order/x"]);
    assert_eq!(log.trim(), "orchestrator", "the commit landed on the order's branch");
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
