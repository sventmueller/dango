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
    mark_prepared(&repos, &tree.path, &prepared()).unwrap();
    let ready = TargetState::WorktreeReady {
        branch: "order/o-1".into(),
    };
    assert!(matches!(
        verify(&repos, &root, &order, &action, &ready).unwrap(),
        Some(OpResult::Prepared { setup: Some(_), .. })
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

/// Thirty numbered lines, with the lines of `changed` given instead.
fn numbered(changed: &[(usize, &str)]) -> String {
    (1..=30)
        .map(|n| {
            let line = changed
                .iter()
                .find(|(at, _)| *at == n)
                .map_or(n.to_string(), |(_, text)| text.to_string());
            format!("{line}\n")
        })
        .collect()
}

/// The names of the hunks of the diff since `base`, as the order tab lists them.
fn hunk_ids(repos: &Repos, base: &str) -> Vec<ide_factory::HunkId> {
    repos
        .diff_since(base, &[] as &[PathBuf])
        .unwrap()
        .iter()
        .flat_map(|diff| {
            let path = diff.path().to_string_lossy().into_owned();
            diff.hunks
                .iter()
                .map(move |hunk| hunk_id(&path, hunk))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn a_rejected_hunk_is_reversed_only_while_the_diff_still_holds_it() {
    let (_scratch, root, repos) = repository("hunks");
    std::fs::write(root.join("lines.txt"), numbered(&[])).unwrap();
    raw(&root, &["add", "."]);
    raw(&root, &["commit", "--quiet", "-m", "lines"]);
    let base = repos.head().unwrap().unwrap().to_string();
    std::fs::write(root.join("lines.txt"), numbered(&[(2, "two"), (25, "twenty-five")])).unwrap();
    let shown = hunk_ids(&repos, &base);
    assert_eq!(shown.len(), 2, "{shown:?}");
    assert!(shown.iter().all(|id| id.0.starts_with("lines.txt#")), "{shown:?}");

    std::fs::write(root.join("lines.txt"), numbered(&[(2, "TWO"), (25, "twenty-five")])).unwrap();
    assert_ne!(
        hunk_ids(&repos, &base)[0],
        shown[0],
        "a hunk with other lines has another name"
    );
    let refused = reject_hunks(&repos, &base, &[shown[0].clone(), shown[1].clone()]);
    assert!(
        matches!(refused, Err(ide_git::Error::PatchDoesNotApply { .. })),
        "{refused:?}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("lines.txt")).unwrap(),
        numbered(&[(2, "TWO"), (25, "twenty-five")]),
        "a refused rejection reverses nothing, not even the hunk that still fits"
    );

    reject_hunks(&repos, &base, &[shown[1].clone()]).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("lines.txt")).unwrap(),
        numbered(&[(2, "TWO")]),
        "only the rejected hunk is reversed"
    );
}

/// What a finished preparation reports, with a setup command that ran.
fn prepared() -> OpResult {
    OpResult::Prepared {
        path: String::new(),
        tree: String::new(),
        setup: Some(ide_factory::gates::CommandOutcome {
            exit: Some(0),
            ..ide_factory::gates::CommandOutcome::default()
        }),
        copied: Vec::new(),
    }
}

#[test]
fn a_worktree_cut_off_before_its_setup_is_removed_so_it_is_prepared_again() {
    let (_scratch, root, repos) = repository("unprepared");
    let order = OrderId("o-3".into());
    let tree = make_worktree(&repos, &root, &order, "order/o-3", "main").unwrap();
    let action = Action::Commit {
        branch: "order/o-3".into(),
        parent: String::new(),
    };
    let ready = TargetState::WorktreeReady {
        branch: "order/o-3".into(),
    };
    assert_eq!(verify(&repos, &root, &order, &action, &ready).unwrap(), None);
    assert!(!tree.path.exists(), "the worktree without its setup is gone");
    assert_eq!(repos.resolve("refs/heads/order/o-3").unwrap(), None);
    let again = make_worktree(&repos, &root, &order, "order/o-3", "main").unwrap();
    mark_prepared(&repos, &again.path, &prepared()).unwrap();
    assert!(matches!(
        verify(&repos, &root, &order, &action, &ready).unwrap(),
        Some(OpResult::Prepared { setup: Some(_), .. })
    ));
}

#[test]
fn reversed_hunks_are_found_and_a_reversal_cut_off_part_way_is_undone() {
    let (_scratch, root, repos) = repository("hunks");
    std::fs::write(root.join("b.txt"), "b\n").unwrap();
    raw(&root, &["add", "."]);
    raw(&root, &["commit", "--quiet", "-m", "second"]);
    let base = repos.head().unwrap().unwrap().to_string();
    let order = OrderId("o-4".into());
    let tree = make_worktree(&repos, &root, &order, "order/o-4", "main").unwrap();
    std::fs::write(tree.path.join("a.txt"), "a\nfrom the builder\n").unwrap();
    std::fs::write(tree.path.join("b.txt"), "b\nfrom the builder\n").unwrap();
    let copy = open_in(&repos, &tree.path).unwrap();
    let parent = copy.head().unwrap().unwrap().to_string();
    let op = OperationId("op-4".into());
    let OpResult::Committed { commit: head, .. } = commit(&copy, "order/o-4", &op, "Turn 1", &parent) else {
        panic!("a commit");
    };
    let hunks = vec![
        ide_factory::HunkId("a.txt#0".into()),
        ide_factory::HunkId("b.txt#0".into()),
    ];
    let action = Action::RejectHunks {
        head: head.clone(),
        hunks: hunks.clone(),
    };
    let target = action.target(&op);
    assert_eq!(verify(&repos, &root, &order, &action, &target).unwrap(), None);
    std::fs::write(tree.path.join("a.txt"), "a\n").unwrap();
    assert_eq!(
        verify(&repos, &root, &order, &action, &target).unwrap(),
        None,
        "half a reversal is not the reversal"
    );
    assert_eq!(
        std::fs::read_to_string(tree.path.join("a.txt")).unwrap(),
        "a\nfrom the builder\n",
        "and is undone"
    );
    reject_hunks(&copy, &base, &hunks).unwrap();
    assert_eq!(
        verify(&repos, &root, &order, &action, &target).unwrap(),
        Some(OpResult::Reversed)
    );
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

#[test]
fn the_secrets_scan_reads_undiffed_and_utf16_files_through_the_worktrees_own_git_folder() {
    let (_scratch, root, repos) = repository("scan-text");
    let order = OrderId("o-9".into());
    let tree = make_worktree(&repos, &root, &order, "order/o-9", "main").unwrap();
    let base = open_in(&repos, &tree.path)
        .unwrap()
        .head()
        .unwrap()
        .unwrap()
        .to_string();
    std::fs::write(tree.path.join(".gitattributes"), "*.txt -diff\n").unwrap();
    std::fs::write(tree.path.join("plain.txt"), "key = one\n").unwrap();
    let wide: Vec<u8> = [0xff, 0xfe]
        .into_iter()
        .chain("key = two\n".bytes().flat_map(|byte| [byte, 0]))
        .collect();
    std::fs::write(tree.path.join("wide.bin"), wide).unwrap();
    std::fs::write(tree.path.join(".git"), "gitdir: .x\n").unwrap();
    let copy = open_in(&repos, &tree.path).unwrap();
    assert!(
        copy.git_dir().starts_with(repos.common_dir()),
        "{:?} is found through the shared git folder, whatever the worktree's .git says",
        copy.git_dir()
    );
    let (text, places) = added_text(&copy, &base).unwrap();
    assert!(text.contains("key = one"), "{text:?}");
    assert!(text.contains("key = two"), "{text:?}");
    assert!(places.iter().any(|place| place == "wide.bin:1"), "{places:?}");
    assert_eq!(text.lines().count(), places.len());
}

#[test]
fn a_line_with_nul_bytes_is_also_read_without_them_and_in_its_pieces() {
    assert_eq!(scanned_lines(b"plain\r"), ["plain "]);
    assert_eq!(scanned_lines(b"k\0e\0y\0"), ["key", "k", "e", "y"].map(String::from));
    assert_eq!(
        scanned_lines(b"\x01\x02\0token = x\0\0"),
        ["\u{1}\u{2}token = x", "\u{1}\u{2}", "token = x"].map(String::from)
    );
}
