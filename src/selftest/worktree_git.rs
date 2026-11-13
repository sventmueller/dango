//! The `worktree-git` scenario: what the Git panel's worktree buttons do to the repository, without the panel. An
//! untouched worktree has nothing to stage; a second thread of the same title gets names of its own; a thread's work
//! taken over one commit at a time lands on the user's branch with its uncommitted rest committed under the thread's
//! title; removing a worktree with its branch leaves neither; a worktree whose folder is gone is forgotten and pruned
//! at the next reading.

use std::path::Path;

use ide_git::{CommitOptions, Git, Repository};

use super::git::{append, fixture_git, write};
use super::{check, finish};
use crate::util::main_soon;
use crate::worktrees::{self, TakeOver};

const NAME: &str = "worktree-git";
const TITLE: &str = "Pick me";

/// A check's name, whether it held and what was seen.
type Seen = (&'static str, bool, String);

pub fn run() {
    let root = ide_core::paths::home().join("worktree-git");
    std::thread::spawn(move || {
        let seen = steps(&root).unwrap_or_else(|error| vec![("the worktree steps run", false, error)]);
        main_soon(move || {
            for (name, ok, detail) in seen {
                check(name, ok, detail);
            }
            finish(NAME);
        });
    });
}

fn repository(git: &Git, root: &Path) -> Result<Repository, String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let repo = git.init(root).map_err(|e| e.to_string())?;
    append(
        &repo.git_dir().join("config"),
        "[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n",
    )?;
    write(root, "README.md", "# Worktrees\n")?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", CommitOptions::default())
        .map_err(|e| e.to_string())?;
    Ok(repo)
}

fn steps(root: &Path) -> Result<Vec<Seen>, String> {
    let git = fixture_git()?;
    let repo = repository(&git, root)?;
    let mut seen = Vec::new();
    let tree = worktrees::create(&repo, root, TITLE, "claude").map_err(|e| format!("{e:?}"))?;
    let clean = worktrees::stage_pending(&git, &tree);
    seen.push((
        "a worktree the thread left as it was has nothing to stage and counts as clean",
        matches!(clean, Ok(ide_git::SecretScan::Clean)),
        format!("{clean:?}"),
    ));
    let twin = worktrees::create(&repo, root, TITLE, "claude").map_err(|e| format!("{e:?}"))?;
    seen.push((
        "a second thread of the same title gets a worktree and a branch of its own",
        twin.path != tree.path && twin.branch == format!("{}-2", tree.branch) && twin.path.is_dir(),
        format!("{} / {}", tree.branch, twin.branch),
    ));
    worktrees::remove(&repo, &twin, true).map_err(|e| e.to_string())?;
    write(&tree.path, "picked.txt", "from the thread\n")?;
    let taken = worktrees::take_over(&git, &repo, &tree, TakeOver::CherryPick);
    let subject = repo.log(1).ok().and_then(|log| log.first().map(|c| c.subject.clone()));
    seen.push((
        "a thread's work taken over commit by commit lands on the user's branch, its rest committed under its title",
        taken.is_ok() && root.join("picked.txt").is_file() && subject.as_deref() == Some(TITLE),
        format!("{taken:?}, last commit {subject:?}"),
    ));
    let removed = worktrees::remove(&repo, &tree, true);
    let branch = repo.resolve(&format!("refs/heads/{}", tree.branch)).ok().flatten();
    seen.push((
        "removing a worktree with its branch leaves neither, and the app forgets it",
        removed.is_ok() && !tree.path.exists() && branch.is_none() && worktrees::list(root).is_empty(),
        format!("{removed:?}, branch {branch:?}"),
    ));
    let gone = worktrees::create(&repo, root, "Gone", "claude").map_err(|e| format!("{e:?}"))?;
    std::fs::remove_dir_all(&gone.path).map_err(|e| e.to_string())?;
    let branches = repo.branches().map_err(|e| e.to_string())?;
    let read = worktrees::read(&git, &repo, root, &branches);
    let listed = repo.worktrees().map_err(|e| e.to_string())?;
    seen.push((
        "a worktree whose folder is gone is forgotten and pruned at the next reading",
        read.is_empty() && worktrees::list(root).is_empty() && listed.iter().all(|w| w.path != gone.path),
        format!("{} read, {} listed by git", read.len(), listed.len()),
    ));
    Ok(seen)
}
