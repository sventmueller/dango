//! Checkpoints in repositories and in folders outside git: taking them without touching the user's state, restoring
//! them around the user's own edits, diffs, buffers, pruning, and that they stay local.

mod common;

use std::path::{Path, PathBuf};

use common::{Scratch, numbered, read, write, write_bytes};
use ide_git::{
    Cancel, Checkpoint, CheckpointKind, Checkpoints, Error, Prune, PushOptions, Restore, RestoreOptions, Restored,
};

fn restored(outcome: Restore) -> Restored {
    match outcome {
        Restore::Restored(restored) => *restored,
        Restore::UserChanges(paths) => panic!("unexpected user changes: {paths:?}"),
    }
}

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

/// Two turns of an agent with the user's edits between and after them, as the app would record them.
fn two_turns(checkpoints: &Checkpoints, root: &Path) -> (Checkpoint, Checkpoint) {
    let first = checkpoints.create("thread-1", "first prompt", &[]).unwrap().checkpoint;
    write(root, "a.txt", "agent 1\n");
    write(root, "agent.txt", "made by the agent\n");
    std::fs::remove_file(root.join("b.txt")).unwrap();
    let first = checkpoints.finish(&first).unwrap();

    write(root, "notes.txt", "the user's notes\n");
    write(root, "mine/new.txt", "the user's file\n");

    let second = checkpoints.create("thread-1", "second prompt", &[]).unwrap().checkpoint;
    write(root, "a.txt", "agent 2\n");
    write(root, "agent2.txt", "more from the agent\n");
    let second = checkpoints.finish(&second).unwrap();
    (first, second)
}

/// The scenario of I6, run against either kind of store.
fn restore_around_the_users_edits(checkpoints: &Checkpoints, root: &Path) {
    let (first, second) = two_turns(checkpoints, root);
    assert!(first.finished && second.finished);
    assert_eq!((first.number, second.number), (1, 2));

    let reported = checkpoints.restore(&first, &RestoreOptions::default()).unwrap();
    assert_eq!(reported, Restore::UserChanges(paths(&["mine/new.txt", "notes.txt"])));
    assert_eq!(read(root, "a.txt"), "agent 2\n", "a report changes nothing");

    let keep = RestoreOptions {
        keep: paths(&["mine/new.txt", "notes.txt"]),
        ..RestoreOptions::default()
    };
    let done = restored(checkpoints.restore(&first, &keep).unwrap());
    assert_eq!(read(root, "a.txt"), "a\n");
    assert_eq!(read(root, "b.txt"), "b\n");
    assert!(!root.join("agent.txt").exists() && !root.join("agent2.txt").exists());
    assert_eq!(read(root, "notes.txt"), "the user's notes\n");
    assert_eq!(read(root, "mine/new.txt"), "the user's file\n");
    assert_eq!(done.kept, paths(&["mine/new.txt", "notes.txt"]));
    assert_eq!(done.removed, paths(&["agent.txt", "agent2.txt"]));
    assert_eq!(done.undo.kind, CheckpointKind::BeforeRestore);
    assert!(done.undo.finished);

    let back = restored(checkpoints.restore(&done.undo, &RestoreOptions::default()).unwrap());
    assert_eq!(read(root, "a.txt"), "agent 2\n", "the restore is undone");
    assert!(root.join("agent.txt").exists() && !root.join("b.txt").exists());

    let forced = RestoreOptions {
        force: true,
        ..RestoreOptions::default()
    };
    restored(checkpoints.restore(&first, &forced).unwrap());
    assert_eq!(read(root, "a.txt"), "a\n");
    assert_eq!(read(root, "notes.txt"), "notes\n");
    assert!(!root.join("mine").exists(), "a folder left empty goes too");
    assert!(back.undo.number > done.undo.number);
    let kinds: Vec<CheckpointKind> = checkpoints
        .list(Some("thread-1"))
        .unwrap()
        .iter()
        .map(|c| c.kind)
        .collect();
    assert_eq!(kinds.iter().filter(|k| **k == CheckpointKind::Turn).count(), 2);
    assert_eq!(kinds.len(), 5);
}

#[test]
fn a_turn_is_undone_while_the_users_later_edits_are_reported_kept_or_forced() {
    let scratch = Scratch::new("turns");
    let repository = scratch.repository("r", &[("a.txt", "a\n"), ("b.txt", "b\n"), ("notes.txt", "notes\n")]);
    restore_around_the_users_edits(&Checkpoints::for_repository(&repository), repository.root());
}

#[test]
fn a_folder_outside_git_has_the_same_checkpoints_in_a_store_of_its_own() {
    let scratch = Scratch::new("folder");
    let folder = scratch.path("plain folder");
    for (path, content) in [("a.txt", "a\n"), ("b.txt", "b\n"), ("notes.txt", "notes\n")] {
        write(&folder, path, content);
    }
    write(&folder, "node_modules/pkg/index.js", "dependency\n");
    write(&folder, ".gitignore", "*.tmp\n");
    write(&folder, "scratch.tmp", "ignored\n");
    let git = scratch.git();
    let store = scratch.path("store");
    let checkpoints = Checkpoints::for_folder(&git, &folder, &store).unwrap();
    assert!(!checkpoints.in_repository());
    restore_around_the_users_edits(&checkpoints, &folder);
    assert!(!folder.join(".git").exists());
    assert_eq!(read(&folder, "node_modules/pkg/index.js"), "dependency\n");
    assert_eq!(read(&folder, "scratch.tmp"), "ignored\n");

    let checkpoint = checkpoints.create("thread-2", "diff", &[]).unwrap().checkpoint;
    write(&folder, "a.txt", "a\nagent line\n");
    let diff = checkpoints.diff(&checkpoint).unwrap();
    assert_eq!(diff.len(), 1);
    checkpoints.revert_hunks(&diff[0], &[0]).unwrap();
    assert_eq!(read(&folder, "a.txt"), "a\n");

    let again = Checkpoints::for_folder(&git, &folder, &store).unwrap();
    assert_eq!(again.list(None).unwrap().len(), 6, "the store is found again");
    let stores: Vec<_> = std::fs::read_dir(&store).unwrap().collect();
    assert_eq!(stores.len(), 1);
}

#[test]
#[ignore = "runs the user's own git with their global configuration; run by make test-integration"]
fn the_users_own_git_restores_around_their_edits_in_a_repository_and_a_plain_folder() {
    let scratch = Scratch::with_users_git("users-git");
    let git = scratch.git();
    eprintln!("{}: {}", git.program().unwrap().display(), git.version().unwrap());
    let repository = scratch.repository("r", &[("a.txt", "a\n"), ("b.txt", "b\n"), ("notes.txt", "notes\n")]);
    restore_around_the_users_edits(&Checkpoints::for_repository(&repository), repository.root());
    let folder = scratch.path("plain folder");
    for (path, content) in [("a.txt", "a\n"), ("b.txt", "b\n"), ("notes.txt", "notes\n")] {
        write(&folder, path, content);
    }
    let checkpoints = Checkpoints::for_folder(&git, &folder, &scratch.path("store")).unwrap();
    restore_around_the_users_edits(&checkpoints, &folder);
}

#[test]
fn taking_a_checkpoint_leaves_the_users_index_head_branches_and_stash_alone() {
    let scratch = Scratch::new("untouched");
    let repository = scratch.repository("r", &[("a.txt", "a\n"), ("b.txt", "b\n")]);
    let root = repository.root();
    write(root, "a.txt", "staged\n");
    repository.stage(&["a.txt"]).unwrap();
    write(root, "a.txt", "staged and more\n");
    write(root, "untracked.txt", "u\n");
    write(root, "c.txt", "stashed\n");
    scratch.raw(
        root,
        &["stash", "push", "--quiet", "--include-untracked", "--", "c.txt"],
    );
    let index_before = std::fs::read(root.join(".git/index")).unwrap();
    let refs_before = scratch.raw(
        root,
        &["for-each-ref", "refs/heads", "refs/tags", "refs/stash", "refs/remotes"],
    );
    let status_before = scratch.raw(root, &["status", "--porcelain=v2"]);
    let head_before = repository.head().unwrap();

    let checkpoints = Checkpoints::for_repository(&repository);
    let created = checkpoints.create("t", "before", &[]).unwrap();
    assert!(created.skipped.is_empty());

    assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index_before);
    assert_eq!(
        scratch.raw(
            root,
            &["for-each-ref", "refs/heads", "refs/tags", "refs/stash", "refs/remotes"]
        ),
        refs_before
    );
    assert_eq!(scratch.raw(root, &["status", "--porcelain=v2"]), status_before);
    assert_eq!(repository.head().unwrap(), head_before);
    assert_eq!(created.checkpoint.base, head_before);
    let refs = scratch.raw(root, &["for-each-ref", "--format=%(refname)", "refs/ide-checkpoints"]);
    assert_eq!(refs.trim(), "refs/ide-checkpoints/t/1");
    assert!(
        repository
            .branches()
            .unwrap()
            .iter()
            .all(|b| !b.full_name.contains("checkpoint"))
    );
    assert_eq!(
        repository.log(10).unwrap().len(),
        1,
        "history does not show checkpoints"
    );
}

#[test]
fn unsaved_buffers_and_untracked_files_come_back_byte_for_byte() {
    let scratch = Scratch::new("buffers");
    let repository = scratch.repository("r", &[("a.txt", "on disk\n")]);
    let root = repository.root();
    write(root, "ünïcode untracked.txt", "untracked\n");
    let buffer: Vec<u8> = b"unsaved\r\nwith \xff invalid\r\n".to_vec();
    let checkpoints = Checkpoints::for_repository(&repository);
    let buffers = vec![
        (PathBuf::from("a.txt"), buffer.clone()),
        (root.join("never saved.txt"), b"draft".to_vec()),
    ];
    let checkpoint = checkpoints.create("t", "with buffers", &buffers).unwrap().checkpoint;

    std::fs::remove_file(root.join("ünïcode untracked.txt")).unwrap();
    write(root, "a.txt", "agent\n");
    let done = restored(checkpoints.restore(&checkpoint, &RestoreOptions::default()).unwrap());
    assert_eq!(read(root, "ünïcode untracked.txt"), "untracked\n");
    assert_eq!(
        read(root, "a.txt"),
        "on disk\n",
        "the file gets its content on disk, not the buffer's"
    );
    let mut returned = done.buffers;
    returned.sort();
    assert_eq!(
        returned,
        vec![
            (PathBuf::from("a.txt"), buffer),
            (PathBuf::from("never saved.txt"), b"draft".to_vec())
        ]
    );
    assert!(!root.join("never saved.txt").exists());
    assert!(matches!(
        checkpoints.create("t", "x", &[(PathBuf::from("/elsewhere/x"), vec![])]),
        Err(Error::OutsideFolder { .. })
    ));
}

#[test]
fn large_files_and_repositories_inside_are_skipped_and_left_alone() {
    let scratch = Scratch::new("skipped");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let root = repository.root();
    write_bytes(root, "video.mov", &[7u8; 4096]);
    scratch.repository("r/vendor/inner", &[("x.txt", "x\n")]);
    let checkpoints = Checkpoints::for_repository(&repository).with_max_file_size(1024);
    let created = checkpoints.create("t", "s", &[]).unwrap();
    let mut skipped = created.skipped.clone();
    skipped.sort();
    assert_eq!(skipped, paths(&["vendor/inner/", "video.mov"]));
    write(root, "a.txt", "changed\n");
    let done = restored(
        checkpoints
            .restore(&created.checkpoint, &RestoreOptions::default())
            .unwrap(),
    );
    assert_eq!(done.written, paths(&["a.txt"]));
    assert!(root.join("video.mov").exists() && root.join("vendor/inner/x.txt").exists());
}

#[test]
fn the_index_comes_back_unless_head_moved_which_is_reported() {
    let scratch = Scratch::new("index");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let root = repository.root();
    write(root, "a.txt", "staged\n");
    repository.stage(&["a.txt"]).unwrap();
    let checkpoints = Checkpoints::for_repository(&repository);
    let checkpoint = checkpoints.create("t", "s", &[]).unwrap().checkpoint;

    repository.unstage(&["a.txt"]).unwrap();
    write(root, "a.txt", "agent\n");
    let done = restored(checkpoints.restore(&checkpoint, &RestoreOptions::default()).unwrap());
    assert!(done.index_restored && !done.head_moved);
    assert_eq!(scratch.raw(root, &["show", ":a.txt"]), "staged\n");
    assert_eq!(read(root, "a.txt"), "staged\n");

    let turn = checkpoints.create("t", "the agent commits", &[]).unwrap().checkpoint;
    write(root, "a.txt", "committed by the agent\n");
    repository.stage(&["a.txt"]).unwrap();
    repository
        .commit("agent commit", ide_git::CommitOptions::default())
        .unwrap();
    checkpoints.finish(&turn).unwrap();
    let done = restored(checkpoints.restore(&checkpoint, &RestoreOptions::default()).unwrap());
    assert!(done.head_moved && !done.index_restored);
    assert_eq!(read(root, "a.txt"), "staged\n");
    assert_eq!(
        repository.log(1).unwrap()[0].subject,
        "agent commit",
        "a restore never moves HEAD"
    );
}

#[test]
fn a_checkpoint_during_a_merge_conflict_has_no_index_but_keeps_the_files() {
    let scratch = Scratch::new("conflict");
    let repository = scratch.repository("r", &[("c.txt", "base\n")]);
    let root = repository.root();
    scratch.raw(root, &["switch", "--quiet", "-c", "other"]);
    write(root, "c.txt", "theirs\n");
    scratch.raw(root, &["commit", "--quiet", "-am", "theirs"]);
    scratch.raw(root, &["switch", "--quiet", "main"]);
    write(root, "c.txt", "ours\n");
    scratch.raw(root, &["commit", "--quiet", "-am", "ours"]);
    assert!(!scratch.try_raw(root, &["merge", "--quiet", "other"]));
    let conflicted = read(root, "c.txt");
    let checkpoints = Checkpoints::for_repository(&repository);
    let checkpoint = checkpoints.create("t", "mid-merge", &[]).unwrap().checkpoint;
    write(root, "c.txt", "resolved by the agent\n");
    let done = restored(checkpoints.restore(&checkpoint, &RestoreOptions::default()).unwrap());
    assert!(!done.index_restored);
    assert_eq!(read(root, "c.txt"), conflicted);
}

#[test]
fn diffs_show_the_changes_since_and_during_a_turn_and_single_hunks_can_be_taken_back() {
    let scratch = Scratch::new("diff");
    let repository = scratch.repository("r", &[("f.txt", &numbered(30))]);
    let root = repository.root();
    let checkpoints = Checkpoints::for_repository(&repository);
    let checkpoint = checkpoints.create("t", "s", &[]).unwrap().checkpoint;
    let edited = numbered(30)
        .replace("line 2\n", "two\n")
        .replace("line 28\n", "twenty-eight\n");
    write(root, "f.txt", &edited);
    write(root, "new.txt", "new\n");

    let diff = checkpoints.diff(&checkpoint).unwrap();
    assert_eq!(diff.len(), 2);
    let file = diff.iter().find(|d| d.path() == Path::new("f.txt")).unwrap();
    assert_eq!(file.hunks.len(), 2);
    checkpoints.revert_hunks(file, &[1]).unwrap();
    assert_eq!(read(root, "f.txt"), numbered(30).replace("line 2\n", "two\n"));
    let created = diff.iter().find(|d| d.path() == Path::new("new.txt")).unwrap();
    checkpoints.revert_hunks(created, &[0]).unwrap();
    assert!(!root.join("new.txt").exists(), "taking back a created file deletes it");

    let checkpoint = checkpoints.finish(&checkpoint).unwrap();
    write(root, "later.txt", "after the turn\n");
    let turn = checkpoints.turn_diff(&checkpoint).unwrap();
    assert_eq!(turn.len(), 1, "the turn ended before later.txt: {turn:?}");
    assert_eq!(checkpoints.diff(&checkpoint).unwrap().len(), 2);
}

#[test]
fn checkpoints_are_listed_pruned_and_deleted() {
    let scratch = Scratch::new("prune");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let checkpoints = Checkpoints::for_repository(&repository);
    for n in 0..3 {
        write(repository.root(), "a.txt", &format!("{n}\n"));
        checkpoints.create("one", &format!("one {n}"), &[]).unwrap();
        checkpoints.create("two", &format!("two {n}"), &[]).unwrap();
    }
    let all = checkpoints.list(None).unwrap();
    assert_eq!(all.len(), 6);
    assert_eq!(all[0].thread, "one");
    assert_eq!(all[2].label, "one 2");
    assert_eq!(
        checkpoints
            .prune(&Prune {
                keep_last: Some(2),
                older_than: None
            })
            .unwrap(),
        2
    );
    let one = checkpoints.list(Some("one")).unwrap();
    assert_eq!(one.iter().map(|c| c.number).collect::<Vec<_>>(), [2, 3]);
    checkpoints.delete(&one[0]).unwrap();
    assert_eq!(checkpoints.list(Some("one")).unwrap().len(), 1);
    assert_eq!(checkpoints.delete_thread("two").unwrap(), 2);
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    assert_eq!(
        checkpoints
            .prune(&Prune {
                keep_last: None,
                older_than: Some(future)
            })
            .unwrap(),
        1
    );
    assert!(checkpoints.list(None).unwrap().is_empty());
    assert!(matches!(
        checkpoints.list(Some("no/slash")),
        Err(Error::InvalidName { .. })
    ));
    assert!(matches!(
        checkpoints.create("bad name", "x", &[]),
        Err(Error::InvalidName { .. })
    ));
}

#[test]
fn checkpoints_taken_at_the_same_time_get_their_own_numbers() {
    let scratch = Scratch::new("parallel");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let checkpoints = Checkpoints::for_repository(&repository);
    let handles: Vec<_> = (0..4)
        .map(|n| {
            let checkpoints = checkpoints.clone();
            std::thread::spawn(move || checkpoints.create("t", &format!("{n}"), &[]).unwrap().checkpoint.number)
        })
        .collect();
    let mut numbers: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, [1, 2, 3, 4]);
}

#[test]
fn checkpoints_stay_local_when_the_branch_is_pushed() {
    let scratch = Scratch::new("local");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    Checkpoints::for_repository(&repository).create("t", "s", &[]).unwrap();
    scratch.raw(&scratch.dir, &["init", "--quiet", "--bare", "remote.git"]);
    scratch.raw(repository.root(), &["remote", "add", "origin", "../remote.git"]);
    let options = PushOptions {
        remote: Some("origin".into()),
        refspec: Some("main".into()),
        set_upstream: true,
    };
    repository.push(&options, &mut |_| {}, &Cancel::new()).unwrap();
    scratch.raw(repository.root(), &["push", "--quiet", "--all", "origin"]);
    scratch.raw(repository.root(), &["push", "--quiet", "--tags", "origin"]);
    let remote_refs = scratch.raw(&scratch.path("remote.git"), &["for-each-ref", "--format=%(refname)"]);
    assert_eq!(remote_refs.trim(), "refs/heads/main");
}

#[test]
fn open_picks_the_repository_when_there_is_one() {
    let scratch = Scratch::new("open");
    let repository = scratch.repository("r", &[("sub/a.txt", "a\n")]);
    let checkpoints = Checkpoints::open(&scratch.git(), &repository.root().join("sub")).unwrap();
    assert!(checkpoints.in_repository());
    assert_eq!(checkpoints.root(), repository.root());
}
