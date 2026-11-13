//! Reading the thread worktrees for the Git panel with as few git calls as a refresh allows: one `git worktree list`
//! for all of them, and per worktree only what changed since the last reading. A reading stays good while the
//! thread's branch, the base and the worktree's index stay as they were and nobody said its files changed; the
//! files a thread or the editor writes leave the index alone, so a finished turn and a save say so through [`mark`].

use ide_core::LockIgnoringPoison;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use ide_git::{Branch, Git, Repository, Worktree};

use super::{ThreadTree, TreeState, forget, list};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// The last reading of a worktree and what it was read against.
struct Reading {
    /// The worktree as a repository, found once.
    copy: Repository,
    /// The tips of the thread's branch and of the base.
    tips: (String, String),
    /// When the worktree's index last changed.
    index: Option<SystemTime>,
    /// The worktree's mark count then.
    mark: u64,
    state: TreeState,
}

#[derive(Default)]
struct Cache {
    readings: HashMap<PathBuf, Reading>,
    /// How often each worktree was said to have changed files.
    marks: HashMap<PathBuf, u64>,
}

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Mutex::default);

fn cache() -> std::sync::MutexGuard<'static, Cache> {
    CACHE.lock_ignoring_poison()
}

/// Files in the worktree at `path` changed, so its next reading looks at them again.
pub fn mark(path: &Path) {
    *cache().marks.entry(path.to_path_buf()).or_default() += 1;
}

/// Drops what is kept about the worktree at `path`.
pub fn evict(path: &Path) {
    let mut cache = cache();
    cache.readings.remove(path);
    cache.marks.remove(path);
}

/// The thread worktrees of the project at `root` as they are now; those git no longer knows or whose folder is gone
/// are forgotten. `branches` are the repository's branches as the same refresh read them.
pub fn read(git: &Git, repo: &Repository, root: &Path, branches: &[Branch]) -> Vec<TreeState> {
    let trees = list(root);
    if trees.is_empty() {
        return Vec::new();
    }
    let Ok(listed) = repo.worktrees() else {
        return Vec::new();
    };
    #[cfg(feature = "selftest")]
    count(1, 0);
    let mut pruned = false;
    trees
        .into_iter()
        .filter_map(|tree| {
            let found = listed
                .iter()
                .find(|w| same_folder(&w.path, &tree.path) && w.prunable.is_none());
            let Some(found) = found.filter(|_| tree.path.exists()) else {
                forget(&tree);
                if !pruned {
                    pruned = true;
                    let _ = repo.prune_worktrees();
                }
                return None;
            };
            let tips = tips(&tree, found, branches);
            Some(read_one(git, repo, tree, tips))
        })
        .collect()
}

fn same_folder(a: &Path, b: &Path) -> bool {
    a == b
        || a.canonicalize()
            .ok()
            .is_some_and(|a| b.canonicalize().is_ok_and(|b| a == b))
}

/// The tips of the thread's branch and of what it is compared against: the branch the user was on, or the commit it
/// started at when that branch is gone.
fn tips(tree: &ThreadTree, found: &Worktree, branches: &[Branch]) -> (String, String) {
    let tip = |name: &str| {
        branches
            .iter()
            .find(|b| !b.remote && b.name == name)
            .map(|b| b.commit.clone())
    };
    let branch = tip(&tree.branch).or_else(|| found.head.clone()).unwrap_or_default();
    let base = tree
        .base_branch
        .as_deref()
        .and_then(tip)
        .unwrap_or_else(|| tree.base.clone());
    (branch, base)
}

fn read_one(git: &Git, repo: &Repository, tree: ThreadTree, tips: (String, String)) -> TreeState {
    let (prior, mark) = {
        let mut cache = cache();
        let mark = cache.marks.get(&tree.path).copied().unwrap_or(0);
        (cache.readings.remove(&tree.path), mark)
    };
    let copy = match prior.as_ref() {
        Some(prior) => Ok(prior.copy.clone()),
        None => git.discover(&tree.path),
    };
    let Ok(copy) = copy else {
        return fallback(tree, prior);
    };
    let index = std::fs::metadata(copy.git_dir().join("index"))
        .and_then(|m| m.modified())
        .ok();
    let same_tips = prior.as_ref().filter(|p| p.tips == tips);
    let (ahead, behind, since) = match same_tips {
        Some(p) => (p.state.ahead, p.state.behind, p.state.since.clone()),
        None => {
            let (ahead, behind) = repo.ahead_behind(&tips.1, &tips.0).unwrap_or((0, 0));
            let since = repo
                .merge_base(&tips.1, &tips.0)
                .ok()
                .flatten()
                .unwrap_or_else(|| tree.base.clone());
            (ahead, behind, since)
        }
    };
    let changed = match same_tips.filter(|p| p.index == index && p.mark == mark) {
        Some(p) => p.state.changed.clone(),
        None => {
            #[cfg(feature = "selftest")]
            count(0, 1);
            copy.changes_since(&since).unwrap_or_default()
        }
    };
    let state = TreeState {
        tree,
        ahead,
        behind,
        since,
        changed,
    };
    cache().readings.insert(
        state.tree.path.clone(),
        Reading {
            copy,
            tips,
            index,
            mark,
            state: state.clone(),
        },
    );
    state
}

/// What to show for a worktree git cannot read now: the last reading, or nothing changed.
fn fallback(tree: ThreadTree, prior: Option<Reading>) -> TreeState {
    prior.map_or_else(
        || TreeState {
            since: tree.base.clone(),
            tree,
            ahead: 0,
            behind: 0,
            changed: Vec::new(),
        },
        |p| p.state,
    )
}
