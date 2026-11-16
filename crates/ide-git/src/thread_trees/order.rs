//! The worktree of a work order (B45a): made on a branch `order/<title>` from the order's base branch, kept up to
//! date by merging the base into it rather than rebasing, so no accepted commit changes (C56), taken over into that
//! base only, never into whatever the user has checked out (F49), and removed together with its checkpoints (C70).
//!
//! The orchestrator passes repositories from [`Repository::unattended`], so none of this runs the repository's hooks
//! (E192). Taking over is for after the user accepted the proof (G2, E164); nothing here pushes.

use std::path::{Path, PathBuf};

use super::{NewTree, TreeError, forget, make};
use crate::{
    CheckpointThread, Checkpoints, CommitId, EntryKind, Error, Operation, Repository, StatusOptions, ThreadTree,
    Untracked,
};

/// Makes a worktree for the work order `order` titled `title`, whose builder is `agent`, on a new branch
/// `order/<title>` that starts at the tip of `base_branch`, or at HEAD and the branch the user is on when `None`.
pub fn create(
    repo: &Repository,
    root: &Path,
    title: &str,
    agent: &str,
    order: &str,
    base_branch: Option<&str>,
) -> Result<ThreadTree, TreeError> {
    make(
        repo,
        root,
        &NewTree {
            prefix: "order",
            title,
            agent,
            order: Some(order),
            base_branch,
        },
    )
}

/// Why an order's work cannot come over now; the take-over waits and names the reason (B45a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The worktree belongs to no order or names no base branch.
    NoBase,
    /// The user's working copy is on another branch than the order's base, or on none.
    OtherBranch {
        /// The branch it is on; `None` when HEAD is detached.
        current: Option<String>,
        /// The order's base branch.
        base: String,
    },
    /// A merge, rebase, cherry-pick or revert waits in the user's working copy.
    InOperation(Operation),
    /// The user's working copy has changes to tracked files; untracked files do not count, git refuses to overwrite
    /// them on its own.
    NotClean {
        /// The changed files, relative to the root.
        paths: Vec<PathBuf>,
    },
    /// The order's branch no longer points at the commit the user accepted.
    Moved {
        /// The accepted commit.
        expected: CommitId,
        /// Where the branch points now; `None` when it is gone.
        actual: Option<CommitId>,
    },
    /// The base has commits the order's branch lacks: merge the base into the order and check it again (C56).
    BaseAhead {
        /// The base's tip.
        base: CommitId,
    },
}

/// Why taking an order's work over did not happen.
#[derive(Debug)]
pub enum TakeOverError {
    /// The take-over waits for the user, for this reason.
    Refused(Refusal),
    /// git failed.
    Git(Error),
}

impl From<Error> for TakeOverError {
    fn from(error: Error) -> TakeOverError {
        TakeOverError::Git(error)
    }
}

/// Brings the order's accepted work into its base branch in the user's working copy `main` with one merge commit,
/// and returns that commit. It refuses unless `main` is on the base, clean and in no operation, the order's branch
/// still points at `accepted` and already contains the base, so the merge can have no conflict.
pub fn take_over(main: &Repository, tree: &ThreadTree, accepted: &CommitId) -> Result<CommitId, TakeOverError> {
    let refuse = |refusal| Err(TakeOverError::Refused(refusal));
    let (Some(_), Some(base)) = (&tree.order, &tree.base_branch) else {
        return refuse(Refusal::NoBase);
    };
    let current = main.current_branch()?;
    if current.as_deref() != Some(base.as_str()) {
        return refuse(Refusal::OtherBranch {
            current,
            base: base.clone(),
        });
    }
    if let Some(operation) = main.operation()? {
        return refuse(Refusal::InOperation(operation));
    }
    let paths = changed_paths(main)?;
    if !paths.is_empty() {
        return refuse(Refusal::NotClean { paths });
    }
    let tip = main.resolve(&format!("refs/heads/{}", tree.branch))?;
    if tip.as_ref() != Some(accepted) {
        return refuse(Refusal::Moved {
            expected: accepted.clone(),
            actual: tip,
        });
    }
    let base_tip = main.head()?.ok_or(Error::Unexpected {
        what: format!("{base} has no commit"),
    })?;
    if main.merge_base(base_tip.as_str(), accepted.as_str())?.as_ref() != Some(&base_tip) {
        return refuse(Refusal::BaseAhead { base: base_tip });
    }
    main.merge_branch(&tree.branch, &format!("Merge branch '{}'", tree.branch))?;
    Ok(main.head()?.ok_or(Error::Unexpected {
        what: "no HEAD after the merge".into(),
    })?)
}

/// The tracked files of `repository` with changes, staged or not, conflicts among them.
fn changed_paths(repository: &Repository) -> crate::Result<Vec<PathBuf>> {
    let status = repository.status(StatusOptions {
        untracked: Untracked::No,
        ignored: false,
    })?;
    Ok(status
        .entries
        .into_iter()
        .filter(|e| !matches!(e.kind, EntryKind::Untracked | EntryKind::Ignored))
        .map(|e| e.path)
        .collect())
}

/// What merging the base into an order's branch did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseMerge {
    /// The order's branch already contains the base.
    UpToDate,
    /// The base came in with this merge commit.
    Merged(CommitId),
    /// The merge stopped at conflicts in these files and waits in the order's worktree; they go back to the builder.
    Conflicts(Vec<PathBuf>),
}

/// Merges the order's base branch into its branch in the worktree `copy`, so the order sees what the user did on the
/// base meanwhile; its own commits stay as they are. The worktree must have nothing uncommitted.
pub fn merge_base(copy: &Repository, tree: &ThreadTree) -> crate::Result<BaseMerge> {
    let Some(base) = &tree.base_branch else {
        return Err(Error::Unexpected {
            what: format!("{} has no base branch", tree.branch),
        });
    };
    let base_tip = copy
        .resolve(&format!("refs/heads/{base}"))?
        .ok_or_else(|| Error::Unexpected {
            what: format!("the base branch {base} is gone"),
        })?;
    if copy.merge_base(base_tip.as_str(), "HEAD")?.as_ref() == Some(&base_tip) {
        return Ok(BaseMerge::UpToDate);
    }
    match copy.merge_branch(base, &format!("Merge branch '{base}' into {}", tree.branch)) {
        Ok(()) => Ok(BaseMerge::Merged(copy.head()?.ok_or(Error::Unexpected {
            what: "no HEAD after the merge".into(),
        })?)),
        Err(Error::Conflicts { paths, .. }) => Ok(BaseMerge::Conflicts(paths)),
        Err(error) => Err(error),
    }
}

/// Removes the order's worktree with whatever it holds, after the checkpoints of its sessions `threads` (E167, C70),
/// and, if `delete_branch`, its branch; returns how many checkpoints went. The checkpoints are deleted through the
/// worktree while it is there and through the user's repository `main` once it is gone, after a crash in between;
/// both see the same refs. Removing an order that was not taken over needs the user's approval (E27).
pub fn remove(
    main: &Repository,
    tree: &ThreadTree,
    threads: &[CheckpointThread],
    delete_branch: bool,
) -> crate::Result<usize> {
    let copy = tree
        .path
        .is_dir()
        .then(|| main.git().discover(&tree.path).ok())
        .flatten()
        .filter(|copy| copy.common_dir() == main.common_dir());
    let checkpoints = Checkpoints::for_repository(copy.as_ref().unwrap_or(main));
    let mut deleted = 0;
    for thread in threads {
        deleted += checkpoints.delete_thread(thread)?;
    }
    if copy.is_some() {
        main.destructive().remove_worktree(&tree.path)?;
    } else {
        main.prune_worktrees()?;
    }
    forget(tree);
    if delete_branch && main.resolve(&format!("refs/heads/{}", tree.branch))?.is_some() {
        main.destructive().delete_branch(&tree.branch)?;
    }
    Ok(deleted)
}
