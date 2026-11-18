//! The ref watch's snapshots (E173, C47) on a temporary repository: every kind of ref is in them, and a comparison
//! names what was added, removed and moved, also by a linked worktree, whose branches live in the same repository.

mod common;

use common::{Scratch, write};
use ide_git::{CheckpointThread, Checkpoints, CommitOptions, RefKind, StashOptions};

#[test]
fn a_snapshot_holds_every_ref_and_a_comparison_names_what_moved() {
    let scratch = Scratch::new("ref-watch");
    let repository = scratch.repository("r", &[("a.txt", "a\n")]);
    let root = repository.root().to_path_buf();
    scratch.raw(&root, &["branch", "doomed"]);
    let before = repository.ref_snapshot().unwrap();
    assert_eq!(before.get("HEAD"), Some("ref: refs/heads/main"));
    assert!(before.get("refs/heads/main").is_some() && before.get("refs/heads/doomed").is_some());

    let head = repository.head().unwrap().unwrap();
    scratch.raw(&root, &["update-ref", "refs/ci-seals/tree", head.as_str()]);
    scratch.raw(&root, &["notes", "add", "-m", "a note", "HEAD"]);
    scratch.raw(&root, &["tag", "v1"]);
    Checkpoints::for_repository(&repository)
        .create(&CheckpointThread::new("o-1.builder").unwrap(), "turn", &[])
        .unwrap();
    write(&root, "a.txt", "stashed\n");
    let options = StashOptions {
        message: "kept".into(),
        untracked: false,
        staged_only: false,
    };
    assert!(repository.stash(&options).unwrap());
    scratch.raw(&root, &["branch", "-D", "doomed"]);
    let copy = repository.add_worktree(&scratch.path("wt"), "order/x", None).unwrap();
    write(copy.root(), "w.txt", "w\n");
    copy.stage(&["w.txt"]).unwrap();
    copy.commit("from the worktree", CommitOptions::default()).unwrap();
    scratch.raw(copy.root(), &["update-ref", "refs/heads/main", "HEAD"]);

    let after = repository.ref_snapshot().unwrap();
    let changes = before.changes_to(&after);
    let kinds: Vec<RefKind> = changes.added.iter().map(|c| c.kind).collect();
    for kind in [
        RefKind::Seal,
        RefKind::Note,
        RefKind::Tag,
        RefKind::Checkpoint,
        RefKind::Stash,
        RefKind::Branch,
    ] {
        assert!(kinds.contains(&kind), "{kind:?} in {:?}", changes.added);
    }
    assert!(changes.added.iter().any(|c| c.name == "refs/heads/order/x"));
    let removed: Vec<&str> = changes.removed.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(removed, ["refs/heads/doomed"]);
    let changed: Vec<&str> = changes.changed.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        changed,
        ["refs/heads/main"],
        "a worktree moving the user's branch shows"
    );
    assert!(after.changes_to(&repository.ref_snapshot().unwrap()).is_empty());
}
