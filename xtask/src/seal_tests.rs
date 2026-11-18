//! The seal against real git: repositories in a scratch folder, a bare one as origin, and no configuration of this Mac.

use std::path::{Path, PathBuf};

use super::*;
use crate::shell::fake::scratch;

/// A repository with one commit of `a.txt`, `VERSION` and a `.gitignore` for `ignored.txt`, and a bare origin.
pub(crate) fn repository(name: &str) -> (PathBuf, Git) {
    let dir = scratch(name);
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let home = home.display().to_string();
    let mut git = Git::at(&dir.join("repo"));
    git.env = [
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("HOME", home.as_str()),
        ("XDG_CONFIG_HOME", home.as_str()),
        ("GIT_AUTHOR_NAME", "Test"),
        ("GIT_AUTHOR_EMAIL", "test@example.invalid"),
        ("GIT_COMMITTER_NAME", "Test"),
        ("GIT_COMMITTER_EMAIL", "test@example.invalid"),
    ]
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .to_vec();
    let origin = dir.join("origin.git").display().to_string();
    let bare = Git {
        dir: dir.clone(),
        env: git.env.clone(),
    };
    bare.output(&mut System, &["init", "--quiet", "--bare", "-b", BRANCH, &origin])
        .unwrap();
    bare.output(&mut System, &["init", "--quiet", "-b", BRANCH, "repo"])
        .unwrap();
    write(&git, "a.txt", "a\n");
    write(&git, "VERSION", "0.1.0\n");
    write(&git, ".gitignore", "ignored.txt\n");
    commit(&git, "first");
    git.output(&mut System, &["remote", "add", "origin", &origin]).unwrap();
    (dir, git)
}

fn write(git: &Git, file: &str, content: &str) {
    std::fs::write(git.dir().join(file), content).unwrap();
}

fn commit(git: &Git, message: &str) {
    git.output(&mut System, &["add", "--all"]).unwrap();
    git.output(&mut System, &["commit", "--quiet", "--no-gpg-sign", "-m", message])
        .unwrap();
}

fn fp(git: &Git, content: Content) -> String {
    fingerprint(&mut System, git, content).unwrap()
}

fn origin_refs(dir: &Path) -> String {
    let origin = Git::at(&dir.join("origin.git"));
    origin
        .output(&mut System, &["for-each-ref", "--format=%(refname) %(objectname)"])
        .unwrap()
}

#[test]
fn the_fingerprint_is_the_content_without_version() {
    let (dir, git) = repository("seal-fingerprint");
    let first = fp(&git, Content::WorkingTree);
    assert!(is_object_name(&first), "{first}");
    assert_eq!(fp(&git, Content::Head), first, "a clean tree is HEAD's content");
    assert_eq!(
        fp(&git, Content::WorkingTree),
        first,
        "the same content, the same fingerprint"
    );

    write(&git, "VERSION", "0.1.1\n");
    assert_eq!(
        fp(&git, Content::WorkingTree),
        first,
        "a raised build number is no change"
    );
    write(&git, "ignored.txt", "scratch\n");
    assert_eq!(fp(&git, Content::WorkingTree), first, "ignored files are no content");

    write(&git, "new.txt", "new\n");
    let untracked = fp(&git, Content::WorkingTree);
    assert_ne!(untracked, first, "an untracked file is content");
    assert_eq!(fp(&git, Content::Head), first);
    let status = git.output(&mut System, &["status", "--porcelain"]).unwrap();
    assert_eq!(
        status, " M VERSION\n?? new.txt\n",
        "the fingerprint leaves the index alone"
    );
    std::fs::remove_file(git.dir().join("new.txt")).unwrap();
    write(&git, "a.txt", "changed\n");
    let changed = fp(&git, Content::WorkingTree);
    assert!(changed != first && changed != untracked);

    write(&git, "a.txt", "a\n");
    commit(&git, "only the version");
    assert_eq!(
        fp(&git, Content::Head),
        first,
        "a commit of the raised build number keeps the fingerprint"
    );

    std::fs::remove_dir_all(git.dir().join(".git")).unwrap();
    assert!(
        fingerprint(&mut System, &git, Content::Head).is_err(),
        "no repository, no fingerprint"
    );
    assert!(!is_object_name("abc") && !is_object_name(&"g".repeat(40)) && is_object_name(&"a".repeat(64)));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_red_seal_outweighs_a_green_one_for_the_same_content() {
    let green = Seal::Green.reference("f1");
    let red = Seal::Red.reference("f1");
    assert_eq!(green, "refs/ci-seals/green/f1");
    assert_eq!(verdict([], "f1"), Verdict::Unsealed);
    assert_eq!(verdict([green.as_str()], "f1"), Verdict::Green);
    assert_eq!(verdict([red.as_str()], "f1"), Verdict::Red);
    assert_eq!(verdict([green.as_str(), red.as_str()], "f1"), Verdict::Red);
    assert_eq!(verdict([red.as_str(), green.as_str()], "f1"), Verdict::Red);
    assert_eq!(
        verdict([green.as_str(), red.as_str()], "f2"),
        Verdict::Unsealed,
        "other content's seals"
    );
    assert_eq!(verdict(["refs/ci-seals/green/f10"], "f1"), Verdict::Unsealed);
    assert_eq!(
        [Verdict::Green, Verdict::Red, Verdict::Unsealed].map(Verdict::name),
        ["green", "red", "none"]
    );
}

#[test]
fn a_seal_is_a_local_ref_to_a_commit_of_the_sealed_tree() {
    let (dir, git) = repository("seal-mint");
    let head = fp(&git, Content::Head);
    let sealed = mint(&mut System, &git, Seal::Green, &head).unwrap();
    assert_eq!(sealed, Seal::Green.reference(&head));
    let tree = git
        .output(&mut System, &["rev-parse", &format!("{sealed}^{{tree}}")])
        .unwrap();
    assert_eq!(tree.trim(), head, "the seal commits the sealed tree");
    assert!(!origin_refs(&dir).contains("refs/ci-seals/"), "minting pushes nothing");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn explain_says_what_separates_heads_content_from_the_nearest_green_seal() {
    let (dir, git) = repository("seal-explain");
    let first = fp(&git, Content::Head);
    let text = explain_in(&mut System, &git).unwrap();
    assert!(
        text.starts_with(&format!("HEAD's content {first}\n  seal here: none\n")),
        "{text}"
    );
    assert!(text.contains("seal on origin: none\n"), "{text}");
    assert!(text.contains("the working tree has the same content\nno green seal here to compare with\n"));

    mint(&mut System, &git, Seal::Green, &first).unwrap();
    let text = explain_in(&mut System, &git).unwrap();
    assert!(
        text.contains("seal here: green\n") && !text.contains("nearest"),
        "{text}"
    );

    for n in 0..=LISTED {
        write(&git, &format!("file-{n:02}.txt"), "x\n");
    }
    commit(&git, "more");
    write(&git, "a.txt", "changed\n");
    let text = explain_in(&mut System, &git).unwrap();
    let working = fp(&git, Content::WorkingTree);
    assert!(
        text.contains(&format!(
            "the working tree differs: content {working}, seal here: none;"
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "nearest green seal {first}, 21 paths differ:\n  file-00.txt\n"
        )),
        "{text}"
    );
    assert!(text.ends_with("  file-19.txt\n  … and 1 more\n"), "{text}");

    git.output(&mut System, &["remote", "set-url", "origin", "/nonexistent/origin.git"])
        .unwrap();
    assert!(
        explain_in(&mut System, &git)
            .unwrap()
            .contains("seal on origin: unreachable (")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn deploy_needs_no_seal_and_pushes_a_green_one_along() {
    let (dir, git) = repository("seal-deploy");
    let head = fp(&git, Content::Head);
    let refused = |git: &Git| ready(&mut System, git).unwrap_err();

    git.output(&mut System, &["switch", "--quiet", "-c", "topic"]).unwrap();
    assert!(
        refused(&git).starts_with("deploy pushes main, but \"topic\""),
        "on another branch"
    );
    git.output(&mut System, &["switch", "--quiet", BRANCH]).unwrap();

    write(&git, "new.txt", "new\n");
    assert!(refused(&git).contains("not committed"), "with an untracked file");
    std::fs::remove_file(git.dir().join("new.txt")).unwrap();

    assert_eq!(ready(&mut System, &git), Ok(head.clone()), "unsealed content may ship");
    mint(&mut System, &git, Seal::Red, &head).unwrap();
    assert_eq!(ready(&mut System, &git), Ok(head.clone()), "so may content sealed red");
    git.output(&mut System, &["tag", "v0.1.0"]).unwrap();
    assert_eq!(push(&mut System, &git, "v0.1.0"), Ok(()));
    let pushed = origin_refs(&dir);
    let main = git.output(&mut System, &["rev-parse", BRANCH]).unwrap();
    assert!(pushed.contains(&format!("refs/heads/main {}", main.trim())), "{pushed}");
    assert!(pushed.contains("refs/tags/v0.1.0"), "{pushed}");
    assert!(!pushed.contains("refs/ci-seals/"), "a red seal stays here: {pushed}");

    git.output(&mut System, &["update-ref", "-d", &Seal::Red.reference(&head)])
        .unwrap();
    mint(&mut System, &git, Seal::Green, &head).unwrap();
    git.output(&mut System, &["tag", "v0.1.1"]).unwrap();
    assert_eq!(push(&mut System, &git, "v0.1.1"), Ok(()));
    assert!(origin_refs(&dir).contains(&Seal::Green.reference(&head)));
    std::fs::remove_dir_all(dir).unwrap();
}
