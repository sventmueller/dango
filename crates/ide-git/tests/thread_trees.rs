//! The worktrees of AI threads and of work orders on temporary repositories: made, listed, taken over and removed as
//! threads always were; an order made from its base, taken over into that base only, kept up to date by merging, its
//! diff compared byte for byte, and removed with its checkpoints also after its folder is gone.

mod common;

use std::path::PathBuf;
use std::sync::OnceLock;

use common::{Scratch, read, write, write_bytes};
use ide_git::thread_trees::order::{self, BaseMerge, Refusal, TakeOverError};
use ide_git::thread_trees::{self, TakeOver, TreeError};
use ide_git::{Change, CheckpointThread, Checkpoints, CommitId, CommitOptions, Operation, Repository, ThreadTree};

/// Puts the app's data folder, which holds the worktrees, into one scratch folder for every test of this file.
fn data_home() {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    let home = HOME.get_or_init(|| ide_core::testing::scratch("git-thread-trees-home").keep());
    ide_core::paths::set_home(home.clone());
}

/// A scratch folder with a repository `r` on `main` holding `files`.
fn repository(name: &str, files: &[(&str, &str)]) -> (Scratch, Repository) {
    data_home();
    let scratch = Scratch::new(name);
    let repository = scratch.repository("r", files);
    (scratch, repository)
}

/// Commits everything in `tree`'s worktree with `message` and returns the commit.
fn commit_in(scratch: &Scratch, tree: &ThreadTree, message: &str) -> CommitId {
    let copy = scratch.git().discover(&tree.path).unwrap();
    copy.stage_all().unwrap();
    copy.commit(message, CommitOptions::default()).unwrap()
}

fn order_tree(repository: &Repository, title: &str, base: Option<&str>) -> ThreadTree {
    order::create(repository, repository.root(), title, "codex", "o-1", base).unwrap()
}

#[test]
fn a_thread_tree_is_made_listed_taken_over_and_removed_as_before() {
    let (scratch, repository) = repository("thread", &[("a.txt", "a\n")]);
    let root = repository.root().to_path_buf();
    let tree = thread_trees::create(&repository, &root, "Fix the Login", "claude").unwrap();
    assert_eq!(tree.branch, "thread/fix-the-login");
    assert_eq!(
        (tree.order.as_deref(), tree.base_branch.as_deref()),
        (None, Some("main"))
    );
    assert_eq!(thread_trees::list(&root), std::slice::from_ref(&tree));
    assert_eq!(thread_trees::tree_of(&tree.path.join("a.txt")), Some(tree.clone()));
    let stored = std::fs::read_to_string(tree.path.with_extension("json")).unwrap();
    assert!(
        !stored.contains("order"),
        "a thread's metadata is stored as before: {stored}"
    );

    write(&tree.path, "b.txt", "from the thread\n");
    let git = scratch.git();
    thread_trees::take_over(&git, &repository, &tree, TakeOver::Merge).unwrap();
    assert_eq!(read(&root, "b.txt"), "from the thread\n");

    thread_trees::remove(&repository, &tree, true).unwrap();
    assert!(!tree.path.exists());
    assert!(thread_trees::list(&root).is_empty());
    assert_eq!(repository.resolve("refs/heads/thread/fix-the-login").unwrap(), None);
}

#[test]
fn an_order_tree_starts_from_its_base_on_an_order_branch() {
    let (scratch, repository) = repository("order-made", &[("a.txt", "a\n")]);
    scratch.raw(repository.root(), &["branch", "dev"]);
    let tree = order_tree(&repository, "CSV-Export", Some("dev"));
    assert_eq!(tree.branch, "order/csv-export");
    assert_eq!(tree.order.as_deref(), Some("o-1"));
    assert_eq!(tree.base_branch.as_deref(), Some("dev"));
    assert_eq!(Some(tree.base.clone()), repository.resolve("dev").unwrap());
    assert_eq!(thread_trees::list(repository.root())[0].order.as_deref(), Some("o-1"));
    let missing = order::create(&repository, repository.root(), "X", "codex", "o-2", Some("gone"));
    assert!(matches!(missing, Err(TreeError::NoBranch(branch)) if branch == "gone"));
}

fn refused(result: Result<CommitId, TakeOverError>) -> Refusal {
    match result {
        Err(TakeOverError::Refused(refusal)) => refusal,
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_order_is_taken_over_into_its_base_only_once_it_contains_the_base() {
    let (scratch, repository) = repository("order-take", &[("a.txt", "a\n")]);
    let root = repository.root().to_path_buf();
    let tree = order_tree(&repository, "Work", None);
    write(&tree.path, "c.txt", "from the order\n");
    let accepted = commit_in(&scratch, &tree, "the order's work");

    scratch.raw(&root, &["switch", "--quiet", "-c", "other"]);
    let other = refused(order::take_over(&repository, &tree, &accepted));
    assert_eq!(
        other,
        Refusal::OtherBranch {
            current: Some("other".into()),
            base: "main".into()
        }
    );
    scratch.raw(&root, &["switch", "--quiet", "main"]);

    write(&root, "a.txt", "the user's edit\n");
    let unclean = refused(order::take_over(&repository, &tree, &accepted));
    assert_eq!(
        unclean,
        Refusal::NotClean {
            paths: vec![PathBuf::from("a.txt")]
        }
    );
    scratch.raw(&root, &["checkout", "--", "a.txt"]);
    write(&root, "untracked.txt", "lies around\n");

    let moved = refused(order::take_over(&repository, &tree, &tree.base));
    assert!(matches!(moved, Refusal::Moved { actual: Some(_), .. }), "{moved:?}");

    write(&root, "d.txt", "the user's own work meanwhile\n");
    scratch.raw(&root, &["add", "d.txt"]);
    scratch.raw(&root, &["commit", "--quiet", "-m", "user"]);
    let ahead = refused(order::take_over(&repository, &tree, &accepted));
    assert!(matches!(ahead, Refusal::BaseAhead { .. }), "{ahead:?}");

    let copy = scratch.git().discover(&tree.path).unwrap();
    let BaseMerge::Merged(merged) = order::merge_base(&copy, &tree).unwrap() else {
        panic!("the base should come in")
    };
    assert_eq!(
        copy.branch_head("order/work").unwrap().unwrap().parents[0],
        accepted,
        "no commit changed"
    );
    assert_eq!(order::merge_base(&copy, &tree).unwrap(), BaseMerge::UpToDate);

    let landed = order::take_over(&repository, &tree, &merged).unwrap();
    assert_eq!(repository.head().unwrap(), Some(landed.clone()));
    assert_eq!(read(&root, "c.txt"), "from the order\n");
    assert_eq!(read(&root, "d.txt"), "the user's own work meanwhile\n");
    let head = repository.branch_head("main").unwrap().unwrap();
    assert_eq!(head.parents.len(), 2, "one merge commit to undo");
}

#[test]
fn an_order_without_its_metadata_is_not_taken_over() {
    let (_scratch, repository) = repository("order-none", &[("a.txt", "a\n")]);
    let tree = thread_trees::create(&repository, repository.root(), "Thread", "claude").unwrap();
    let head = repository.head().unwrap().unwrap();
    assert_eq!(refused(order::take_over(&repository, &tree, &head)), Refusal::NoBase);
}

#[test]
fn merging_the_base_into_an_order_reports_its_conflicts() {
    let (scratch, repository) = repository("order-conflict", &[("a.txt", "a\n")]);
    let root = repository.root().to_path_buf();
    let tree = order_tree(&repository, "Clash", None);
    write(&tree.path, "a.txt", "the order's line\n");
    commit_in(&scratch, &tree, "order");
    write(&root, "a.txt", "the user's line\n");
    scratch.raw(&root, &["commit", "--quiet", "-am", "user"]);

    let copy = scratch.git().discover(&tree.path).unwrap();
    assert_eq!(
        order::merge_base(&copy, &tree).unwrap(),
        BaseMerge::Conflicts(vec![PathBuf::from("a.txt")])
    );
    assert_eq!(
        copy.operation().unwrap(),
        Some(Operation::Merge),
        "the merge waits for the builder"
    );
}

#[test]
fn the_full_diff_tells_whitespace_and_binary_bytes_and_survives_an_unrelated_base_change() {
    let (scratch, repository) = repository("full-diff", &[("a.txt", "x\n"), ("b.txt", "b\n")]);
    let root = repository.root().to_path_buf();
    let base = repository.head().unwrap().unwrap();
    let tree = order_tree(&repository, "Diff", None);
    write(&tree.path, "a.txt", "x \n");
    write_bytes(&tree.path, "blob.bin", &[0, 1, 2, 255, 0]);
    let first_tip = commit_in(&scratch, &tree, "spaces and a binary");
    let first = repository.full_diff(base.as_str(), first_tip.as_str()).unwrap();
    assert!(String::from_utf8_lossy(&first).contains("GIT binary patch"));
    assert_eq!(first, repository.full_diff(base.as_str(), first_tip.as_str()).unwrap());

    scratch.raw(&root, &["config", "diff.noprefix", "true"]);
    scratch.raw(&root, &["config", "diff.algorithm", "patience"]);
    assert_eq!(
        first,
        repository.full_diff(base.as_str(), first_tip.as_str()).unwrap(),
        "the user's diff settings change nothing"
    );

    write(&root, "b.txt", "the user changed another file\n");
    scratch.raw(&root, &["commit", "--quiet", "-am", "user"]);
    let new_base = repository.head().unwrap().unwrap();
    let copy = scratch.git().discover(&tree.path).unwrap();
    let BaseMerge::Merged(merged) = order::merge_base(&copy, &tree).unwrap() else {
        panic!("the base should come in")
    };
    assert_eq!(
        repository.full_diff(new_base.as_str(), merged.as_str()).unwrap(),
        first,
        "the order's diff against the new base is the same bytes (C56)"
    );

    write(&tree.path, "a.txt", "x\t\n");
    let tabbed = commit_in(&scratch, &tree, "a tab instead");
    assert_ne!(repository.full_diff(new_base.as_str(), tabbed.as_str()).unwrap(), first);
}

#[test]
fn a_branch_head_names_its_parents_tree_and_message() {
    let (scratch, repository) = repository("branch-head", &[("a.txt", "a\n")]);
    let root = repository.root().to_path_buf();
    let first = repository.head().unwrap().unwrap();
    write(&root, "a.txt", "b\n");
    scratch.raw(&root, &["commit", "--quiet", "-am", "work", "-m", "Operation: op-7"]);
    let head = repository.branch_head("main").unwrap().unwrap();
    assert_eq!(head.parents, [first]);
    assert_eq!(head.tree, scratch.raw(&root, &["rev-parse", "HEAD^{tree}"]).trim());
    assert!(
        head.message.starts_with("work\n") && head.message.contains("Operation: op-7"),
        "{}",
        head.message
    );
    assert_eq!(repository.branch_head("nothing").unwrap(), None);
}

#[test]
fn the_changes_between_two_commits_name_each_file_with_its_status() {
    let files = [
        ("a.txt", "a\n"),
        ("c.txt", "c\n"),
        ("d.txt", "a file long enough to be found again\n"),
    ];
    let (scratch, repository) = repository("between", &files);
    let root = repository.root().to_path_buf();
    let base = repository.head().unwrap().unwrap();
    write(&root, "a.txt", "changed\n");
    write(&root, "b.txt", "new\n");
    std::fs::remove_file(root.join("c.txt")).unwrap();
    std::fs::rename(root.join("d.txt"), root.join("e.txt")).unwrap();
    scratch.raw(&root, &["add", "--all"]);
    scratch.raw(&root, &["commit", "--quiet", "-m", "changes"]);
    let changes = repository.changes_between(base.as_str(), "HEAD").unwrap();
    let seen: Vec<(String, Change)> = changes
        .iter()
        .map(|c| (c.path.display().to_string(), c.change))
        .collect();
    assert_eq!(
        seen,
        [
            ("a.txt".to_string(), Change::Modified),
            ("b.txt".to_string(), Change::Added),
            ("c.txt".to_string(), Change::Deleted),
            ("e.txt".to_string(), Change::Renamed { similarity: 100 }),
        ]
    );
    assert_eq!(changes[3].from, Some(PathBuf::from("d.txt")));
}

fn thread(name: &str) -> CheckpointThread {
    CheckpointThread::new(name).unwrap()
}

fn checkpoint_threads(repository: &Repository) -> Vec<String> {
    let mut names: Vec<String> = Checkpoints::for_repository(repository)
        .list(None)
        .unwrap()
        .into_iter()
        .map(|c| c.thread.as_str().to_string())
        .collect();
    names.dedup();
    names
}

#[test]
fn an_order_goes_with_its_checkpoints_also_once_its_folder_is_gone() {
    let (scratch, repository) = repository("order-remove", &[("a.txt", "a\n")]);
    let builder = thread("o-1.builder");
    Checkpoints::for_repository(&repository)
        .create(&thread("someone-else"), "kept", &[])
        .unwrap();

    let tree = order_tree(&repository, "First", None);
    let copy = scratch.git().discover(&tree.path).unwrap();
    Checkpoints::for_repository(&copy)
        .create(&builder, "turn", &[])
        .unwrap();
    write(&tree.path, "left.txt", "uncommitted\n");
    assert_eq!(
        order::remove(&repository, &tree, std::slice::from_ref(&builder), true).unwrap(),
        1
    );
    assert!(!tree.path.exists());
    assert_eq!(repository.resolve("refs/heads/order/first").unwrap(), None);
    assert_eq!(checkpoint_threads(&repository), ["someone-else"]);

    let crashed = order_tree(&repository, "Second", None);
    let copy = scratch.git().discover(&crashed.path).unwrap();
    Checkpoints::for_repository(&copy)
        .create(&builder, "turn", &[])
        .unwrap();
    Checkpoints::for_repository(&copy)
        .create(&builder, "turn", &[])
        .unwrap();
    std::fs::remove_dir_all(&crashed.path).unwrap();
    assert_eq!(order::remove(&repository, &crashed, &[builder], false).unwrap(), 2);
    assert_eq!(checkpoint_threads(&repository), ["someone-else"]);
    assert!(thread_trees::list(repository.root()).is_empty());
    let listed = repository.worktrees().unwrap();
    assert!(
        listed.iter().all(|w| w.path.file_name() != crashed.path.file_name()),
        "{listed:?}"
    );
    assert!(
        repository.resolve("refs/heads/order/second").unwrap().is_some(),
        "the branch stays unless asked"
    );
}

#[test]
fn an_order_whose_git_file_a_builder_redirected_is_removed_all_the_same() {
    let (scratch, repository) = repository("order-redirected", &[("a.txt", "a\n")]);
    let tree = order_tree(&repository, "Redirected", None);
    let listed = repository
        .worktrees()
        .unwrap()
        .into_iter()
        .find(|w| w.branch.as_deref() == Some("refs/heads/order/redirected"))
        .map(|w| w.path)
        .unwrap();
    assert_eq!(listed.canonicalize().unwrap(), tree.path.canonicalize().unwrap());
    let elsewhere = scratch.path("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    write(&elsewhere, "keep.txt", "not the worktree's\n");
    write(&tree.path, ".git", &format!("gitdir: {}\n", elsewhere.display()));

    order::remove(&repository, &tree, &[], true).unwrap();
    assert!(!tree.path.exists());
    assert!(elsewhere.join("keep.txt").exists(), "only the recorded folder goes");
    let common = repository.common_dir().join("worktrees");
    let left: Vec<_> = std::fs::read_dir(&common)
        .map(|entries| entries.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "the worktree's record is gone: {left:?}");
    assert_eq!(repository.resolve("refs/heads/order/redirected").unwrap(), None);
    assert!(thread_trees::list(repository.root()).is_empty());
}
