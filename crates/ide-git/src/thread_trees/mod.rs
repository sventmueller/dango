//! The worktrees of AI threads (I1) and of work orders (B45a): a git worktree on a branch of its own each, so the
//! edits stay out of the user's working tree until the user takes them over. The worktrees live in the app's data
//! folder, one folder per project and never inside the repository; beside each lies what the app knows about it, a
//! [`ThreadTree`] as JSON. Every call runs git and so belongs on a worker thread, only for trusted folders (E22).
//!
//! A thread's branch is `thread/<title>`, an order's `order/<title>`. The order variants — taking over into the
//! order's base, merging the base into the order, removing it with its checkpoints — are in [`order`].

pub mod order;
mod read;

use std::path::{Path, PathBuf};

use crate::{CommitOptions, EntryKind, Error, Git, Repository, SecretScan, StatusOptions, ThreadTree};

#[cfg(feature = "selftest")]
pub use read::counts as read_counts;
pub use read::{mark, read};

/// How the thread's work comes over into the user's branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeOver {
    /// One merge commit.
    Merge,
    /// Its commits one by one.
    CherryPick,
}

/// Why a worktree could not be made.
#[derive(Debug)]
pub enum TreeError {
    /// The repository has no commit to start a branch from.
    NoCommit,
    /// The branch an order should start from does not exist.
    NoBranch(String),
    /// git failed.
    Git(Error),
    /// The worktree's folder or what is kept beside it could not be written.
    Io(std::io::Error),
}

impl From<Error> for TreeError {
    fn from(error: Error) -> TreeError {
        TreeError::Git(error)
    }
}

impl From<std::io::Error> for TreeError {
    fn from(error: std::io::Error) -> TreeError {
        TreeError::Io(error)
    }
}

/// The folder of every project's thread and order worktrees.
pub fn all_folder() -> PathBuf {
    ide_core::paths::data_dir().join("worktrees")
}

/// The folder of the thread and order worktrees of the project at `root`.
pub fn folder(root: &Path) -> PathBuf {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    all_folder().join(ide_core::paths::project_key(&root))
}

/// `title` as a branch and folder name: lower case letters, digits and dashes, at most 40, `fallback` when nothing
/// is left.
fn slug(title: &str, fallback: &str) -> String {
    let mut slug = String::new();
    for c in title.to_lowercase().chars() {
        let part = match c {
            'ä' => "ae",
            'ö' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            c if c.is_ascii_alphanumeric() => {
                slug.push(c);
                continue;
            }
            _ => "-",
        };
        if part == "-" && (slug.is_empty() || slug.ends_with('-')) {
            continue;
        }
        slug.push_str(part);
    }
    let slug: String = slug.chars().take(40).collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() { fallback.into() } else { slug.into() }
}

/// What a new worktree is for.
struct NewTree<'a> {
    /// The branch's first part, `thread` or `order`, which is also the name when the title gives none.
    prefix: &'a str,
    title: &'a str,
    agent: &'a str,
    order: Option<&'a str>,
    /// The branch it starts from; `None` for HEAD and the branch the user is on.
    base_branch: Option<&'a str>,
}

/// Makes a worktree for a thread of `agent` titled `title` on a new branch that starts at HEAD, and keeps what it is.
pub fn create(repo: &Repository, root: &Path, title: &str, agent: &str) -> Result<ThreadTree, TreeError> {
    make(
        repo,
        root,
        &NewTree {
            prefix: "thread",
            title,
            agent,
            order: None,
            base_branch: None,
        },
    )
}

fn make(repo: &Repository, root: &Path, new: &NewTree<'_>) -> Result<ThreadTree, TreeError> {
    let (base, base_branch) = match new.base_branch {
        None => (repo.head()?.ok_or(TreeError::NoCommit)?, repo.current_branch()?),
        Some(branch) => {
            let tip = repo.resolve(&format!("refs/heads/{branch}"))?;
            (
                tip.ok_or_else(|| TreeError::NoBranch(branch.to_string()))?,
                Some(branch.to_string()),
            )
        }
    };
    let folder = folder(root);
    std::fs::create_dir_all(&folder)?;
    let stem = slug(new.title, new.prefix);
    let mut n = 1;
    let (name, branch) = loop {
        let name = if n == 1 { stem.clone() } else { format!("{stem}-{n}") };
        let branch = format!("{}/{name}", new.prefix);
        let taken = folder.join(&name).exists()
            || folder.join(format!("{name}.json")).exists()
            || repo.resolve(&format!("refs/heads/{branch}"))?.is_some();
        if !taken {
            break (name, branch);
        }
        n += 1;
    };
    let path = folder.join(&name);
    repo.add_worktree(&path, &branch, Some(base.as_str()))?;
    let tree = ThreadTree {
        path: path.canonicalize().unwrap_or(path),
        branch,
        title: new.title.trim().to_string(),
        base,
        base_branch,
        agent: new.agent.to_string(),
        order: new.order.map(str::to_string),
    };
    write(&folder.join(format!("{name}.json")), &tree)?;
    Ok(tree)
}

fn write(file: &Path, tree: &ThreadTree) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(tree).map_err(std::io::Error::other)?;
    std::fs::write(file, text)
}

/// The thread and order worktrees kept for the project at `root`.
pub fn list(root: &Path) -> Vec<ThreadTree> {
    let Ok(entries) = std::fs::read_dir(folder(root)) else {
        return Vec::new();
    };
    let mut trees: Vec<ThreadTree> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    trees.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.branch.cmp(&b.branch)));
    trees
}

/// The thread or order worktree that holds `path`, for a file opened from it.
pub fn tree_of(path: &Path) -> Option<ThreadTree> {
    let all = all_folder();
    let real = all.canonicalize().unwrap_or_else(|_| all.clone());
    let inside = path.strip_prefix(&all).or_else(|_| path.strip_prefix(&real)).ok()?;
    let mut parts = inside.components();
    let key = parts.next()?.as_os_str().to_owned();
    let name = parts.next()?.as_os_str().to_string_lossy().into_owned();
    parts.next()?;
    let file = all.join(key).join(format!("{name}.json"));
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

/// Whether `path` lies in the folder of the thread and order worktrees.
pub fn holds(path: &Path) -> bool {
    let all = all_folder();
    path.starts_with(&all) || all.canonicalize().is_ok_and(|real| path.starts_with(real))
}

/// Forgets what the app kept about `tree`.
fn forget(tree: &ThreadTree) {
    read::evict(&tree.path);
    let file = tree.path.with_extension("json");
    if let Err(error) = std::fs::remove_file(&file)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        ide_core::diag::report("forgetting a thread worktree", error);
    }
}

/// Stages what the thread left uncommitted in its worktree and checks it for secrets (I4), before it is committed.
pub fn stage_pending(git: &Git, tree: &ThreadTree) -> crate::Result<SecretScan> {
    let copy = git.discover(&tree.path)?;
    let status = copy.status(StatusOptions::default())?;
    if !status.entries.iter().any(|e| e.kind != EntryKind::Ignored) {
        return Ok(SecretScan::Clean);
    }
    copy.stage_all()?;
    copy.scan_staged()
}

/// Commits what the thread left uncommitted in its worktree, titled as the thread.
fn commit_pending(git: &Git, tree: &ThreadTree) -> crate::Result<()> {
    let copy = git.discover(&tree.path)?;
    copy.stage_all()?;
    match copy.commit(&tree.title, CommitOptions::default()) {
        Ok(_) | Err(Error::NothingToCommit) => Ok(()),
        Err(error) => Err(error),
    }
}

/// Brings the thread's work into the user's current branch, committing what it left uncommitted first. Conflicts
/// stop the merge or cherry-pick for the user, as `Error::Conflicts`.
pub fn take_over(git: &Git, repo: &Repository, tree: &ThreadTree, how: TakeOver) -> crate::Result<()> {
    commit_pending(git, tree)?;
    match how {
        TakeOver::Merge => repo.merge_branch(&tree.branch, &format!("Merge branch '{}'", tree.branch)),
        TakeOver::CherryPick => {
            let base = repo
                .merge_base("HEAD", &tree.branch)?
                .unwrap_or_else(|| tree.base.clone());
            repo.cherry_pick_branch(base.as_str(), &tree.branch)
        }
    }
}

/// Removes the worktree but keeps its branch, with what the thread left uncommitted committed on it.
pub fn keep_branch(git: &Git, repo: &Repository, tree: &ThreadTree) -> crate::Result<()> {
    commit_pending(git, tree)?;
    repo.remove_worktree(&tree.path)?;
    forget(tree);
    Ok(())
}

/// Removes the worktree with whatever it holds and, if `delete_branch`, its branch with commits not merged anywhere;
/// only after the user confirmed it (E27).
pub fn remove(repo: &Repository, tree: &ThreadTree, delete_branch: bool) -> crate::Result<()> {
    repo.destructive().remove_worktree(&tree.path)?;
    forget(tree);
    if delete_branch {
        repo.destructive().delete_branch(&tree.branch)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_becomes_a_branch_and_folder_name() {
        assert_eq!(slug("Fix the Login!", "thread"), "fix-the-login");
        assert_eq!(slug("  Größe ändern  ", "thread"), "groesse-aendern");
        assert_eq!(slug("Brücke über", "thread"), "bruecke-ueber");
        assert_eq!(slug("日本", "thread"), "thread");
        assert_eq!(slug("日本", "order"), "order");
        assert_eq!(slug("a/b..c", "thread"), "a-b-c");
        assert_eq!(slug(&"x".repeat(60), "thread").len(), 40);
    }

    #[test]
    fn a_file_in_a_thread_worktree_finds_its_tree() {
        let tree = ThreadTree {
            path: folder(Path::new("/work/shop")).join("fix"),
            branch: "thread/fix".into(),
            title: "Fix".into(),
            base: crate::CommitId::parse(&"a".repeat(40)).unwrap(),
            base_branch: Some("main".into()),
            agent: String::new(),
            order: None,
        };
        assert_eq!(tree.path.with_extension("json").file_name().unwrap(), "fix.json");
        assert_eq!(tree_of(Path::new("/work/shop/src/lib.rs")), None);
        assert!(holds(&tree.path) && !holds(Path::new("/work/shop")));
    }
}
