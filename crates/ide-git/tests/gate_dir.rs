//! The private git folder of a gate: a linked worktree's HEAD, the shared refs packed and loose, the index, and no
//! link out of the repository followed.

mod common;

use std::process::Command;

use common::{Scratch, write};
use ide_git::GateGitDir;

#[test]
fn the_private_folder_sees_the_worktrees_head_and_every_ref_but_follows_no_link() {
    let scratch = Scratch::new("gate-dir");
    let main = scratch.repository("repo", &[("a.txt", "a\n")]);
    let root = main.root().to_path_buf();
    scratch.raw(&root, &["tag", "packed"]);
    scratch.raw(&root, &["pack-refs", "--all"]);
    scratch.raw(&root, &["branch", "loose"]);
    let tree = scratch.path("tree");
    scratch.raw(
        &root,
        &["worktree", "add", "--quiet", "-b", "order", &tree.to_string_lossy()],
    );
    write(&tree, "b.txt", "b\n");
    scratch.raw(&tree, &["add", "b.txt"]);
    scratch.raw(&tree, &["commit", "--quiet", "-m", "second"]);
    std::fs::write(scratch.path("secret"), "token\n").unwrap();
    std::os::unix::fs::symlink(scratch.path("secret"), main.common_dir().join("refs/heads/linked")).unwrap();

    let copy = scratch.git().discover(&tree).unwrap();
    let private = GateGitDir::build(&copy, &scratch.path("run/git")).unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("/usr/bin/git")
            .args(args)
            .current_dir(&tree)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .envs(private.environment())
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    assert_eq!(git(&["rev-parse", "HEAD"]), copy.head().unwrap().unwrap().to_string());
    assert_eq!(git(&["symbolic-ref", "HEAD"]), "refs/heads/order");
    assert_eq!(
        git(&["for-each-ref", "--format=%(refname)"]),
        "refs/heads/loose\nrefs/heads/main\nrefs/heads/order\nrefs/tags/packed"
    );
    assert_eq!(git(&["ls-files"]), "a.txt\nb.txt");
    assert!(!private.folder().join("git/refs/heads/linked").exists());
    assert!(
        std::fs::read_to_string(private.folder().join("git/config"))
            .unwrap()
            .contains("hooksPath = /dev/null")
    );
    let folder = private.folder().to_path_buf();
    private.remove().unwrap();
    assert!(!folder.exists());
}
