//! The orchestrator's git on temporary repositories: a worktree made and found again, a commit with its operation in
//! the trailer and without the repository's hooks (C53), the checks a restart makes (C66), the ref watch (E173) and
//! the removal.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use ide_factory::event::{Action, OpResult, TargetState};
use ide_factory::{OperationId, OrderId};

use super::*;

/// Puts the app's data folder, which holds the worktrees, into one scratch folder for every test here.
fn data_home() {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    let home = HOME.get_or_init(|| ide_core::testing::scratch("leitstand-git-home").keep());
    ide_core::paths::set_home(home.clone());
}

fn raw(root: &Path, args: &[&str]) {
    let status = Command::new("/usr/bin/git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn login(scratch: &Path) -> Vec<(String, String)> {
    vec![
        ("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into()),
        ("HOME".into(), scratch.join("home").to_string_lossy().into_owned()),
    ]
}

/// A repository on `main` with a user and one commit, opened as the orchestrator opens it.
fn repository(name: &str) -> (ide_core::testing::Scratch, PathBuf, Repos) {
    data_home();
    let scratch = ide_core::testing::scratch(&format!("leitstand-git-{name}"));
    let root = scratch.join("repo");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(scratch.join("home")).unwrap();
    raw(&root, &["init", "--quiet", "--initial-branch=main"]);
    raw(&root, &["config", "user.name", "Test"]);
    raw(&root, &["config", "user.email", "test@example.org"]);
    std::fs::write(root.join("a.txt"), "a\n").unwrap();
    raw(&root, &["add", "."]);
    raw(&root, &["commit", "--quiet", "-m", "first"]);
    let root = root.canonicalize().unwrap();
    let repos = open(&root, &login(&scratch)).unwrap();
    (scratch, root, repos)
}

#[test]
fn an_order_commits_without_hooks_and_a_restart_finds_its_commit_and_worktree() {
    let (_scratch, root, repos) = repository("commit");
    let order = OrderId("o-1".into());
    let tree = make_worktree(&repos, &root, &order, "order/o-1", "main").unwrap();
    assert_eq!(tree.branch, "order/o-1");
    let again = make_worktree(&repos, &root, &order, "order/o-1", "main").unwrap();
    assert_eq!(
        again.path, tree.path,
        "a second preparation finds the first one's worktree"
    );
    let hook = root.join(".git/hooks/pre-commit");
    std::fs::write(&hook, format!("#!/bin/sh\ntouch {}\n", root.join("hooked").display())).unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(tree.path.join("b.txt"), "from the builder\n").unwrap();
    let copy = open_in(&repos, &tree.path).unwrap();
    let parent = copy.head().unwrap().unwrap().to_string();
    let op = OperationId("op-1".into());
    let OpResult::Committed { commit: made, .. } = commit(&copy, "order/o-1", &op, "Turn 1", &parent) else {
        panic!("a commit");
    };
    assert!(
        !root.join("hooked").exists(),
        "the repository's hook does not run (C53)"
    );
    assert_eq!(
        commit(&copy, "order/o-1", &op, "Turn 2", &parent),
        OpResult::Failed {
            detail: splat_i18n::t("Der Worktree steht nicht mehr auf dem erwarteten Commit.").into(),
        }
    );
    let action = Action::Commit {
        branch: "order/o-1".into(),
        parent: parent.clone(),
    };
    let found = verify(&repos, &root, &order, &action, &action.target(&op)).unwrap();
    assert!(matches!(found, Some(OpResult::Committed { commit: ref c, .. }) if *c == made));
    let other = OperationId("op-2".into());
    assert_eq!(
        verify(&repos, &root, &order, &action, &action.target(&other)).unwrap(),
        None
    );
    let on = TargetState::WorktreeOn {
        branch: "order/o-1".into(),
    };
    assert!(matches!(
        verify(&repos, &root, &order, &action, &on).unwrap(),
        Some(OpResult::Prepared { .. })
    ));
    let names = diff_names(&repos, &parent, &made).unwrap();
    assert_eq!(names.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), ["b.txt"]);

    let path = tree.path.to_string_lossy().into_owned();
    assert_eq!(
        remove_worktree(&repos, &root, &order, &path, "order/o-1", false),
        OpResult::Removed
    );
    let gone = TargetState::WorktreeGone { path };
    assert_eq!(
        verify(&repos, &root, &order, &action, &gone).unwrap(),
        Some(OpResult::Removed)
    );
    assert_eq!(repos.resolve("refs/heads/order/o-1").unwrap(), None);
}

#[test]
fn the_ref_watch_tells_what_dango_made_from_what_others_did() {
    let (_scratch, root, repos) = repository("refs");
    let before = refs(&repos).unwrap();
    raw(&root, &["branch", "order/o-2"]);
    raw(&root, &["update-ref", "refs/ide-checkpoints/o-1-builder/1", "HEAD"]);
    raw(&root, &["branch", "stray"]);
    raw(&root, &["branch", "order/o-1"]);
    let after = refs(&repos).unwrap();
    let changes = ref_changes(&before, &after, "order/o-1");
    let by_dango = |name: &str| changes.iter().find(|c| c.name == name).map(|c| c.by_dango);
    assert_eq!(by_dango("refs/heads/order/o-2"), Some(true));
    assert_eq!(by_dango("refs/ide-checkpoints/o-1-builder/1"), Some(true));
    assert_eq!(by_dango("refs/heads/stray"), Some(false));
    assert_eq!(
        by_dango("refs/heads/order/o-1"),
        Some(false),
        "the order's own branch moves only by its commits"
    );
}
