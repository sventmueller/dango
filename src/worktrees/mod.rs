//! Worktree threads (I1): an AI thread that works in a git worktree and on a branch of its own, so its edits stay out
//! of the user's working tree until the user takes them over. The worktrees live in the app's data folder, one folder
//! per project and never inside the repository; beside each lies what the app knows about it. Everything here calls
//! git and so runs on worker threads, only for trusted folders (E22).

mod read;
mod start;

use std::path::{Path, PathBuf};

use ide_git::{ChangedFile, Git, Repository, SecretScan, StatusOptions};
use serde::{Deserialize, Serialize};

#[cfg(feature = "selftest")]
pub use read::counts as read_counts;
pub use read::read;
pub use start::{agent_usable, agents, ask_and_open, can_start, open_thread};

/// What the app keeps about a thread's worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadTree {
    /// Its folder.
    pub path: PathBuf,
    /// Its branch, such as `thread/fix-login`.
    pub branch: String,
    /// The thread's title, which the branch is named after.
    pub title: String,
    /// The commit the branch started at.
    pub base: String,
    /// The branch the user was on then, which ahead and behind count against; `None` for a detached HEAD.
    pub base_branch: Option<String>,
    /// The agent its thread runs, by the id threads name it with; empty means Claude.
    #[serde(default)]
    pub agent: String,
}

/// A thread's worktree as it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeState {
    pub tree: ThreadTree,
    /// Commits on the thread's branch the base lacks.
    pub ahead: usize,
    /// Commits on the base the thread's branch lacks.
    pub behind: usize,
    /// Where the branch and the base meet, which its changes are shown against.
    pub since: String,
    /// What the worktree changed since then, committed or not.
    pub changed: Vec<ChangedFile>,
}

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
    Git(ide_git::Error),
    Io(std::io::Error),
}

impl From<ide_git::Error> for TreeError {
    fn from(error: ide_git::Error) -> TreeError {
        TreeError::Git(error)
    }
}

impl From<std::io::Error> for TreeError {
    fn from(error: std::io::Error) -> TreeError {
        TreeError::Io(error)
    }
}

/// The folder of every project's thread worktrees.
pub fn all_folder() -> PathBuf {
    ide_core::paths::data_dir().join("worktrees")
}

/// The folder of the thread worktrees of the project at `root`.
pub fn folder(root: &Path) -> PathBuf {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    all_folder().join(ide_core::paths::project_key(&root))
}

/// `title` as a branch and folder name: lower case letters, digits and dashes, at most 40, `thread` when nothing is
/// left.
pub fn slug(title: &str) -> String {
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
    if slug.is_empty() { "thread".into() } else { slug.into() }
}

/// Makes a worktree for a thread of `agent` titled `title` on a new branch that starts at HEAD, and keeps what it is.
pub fn create(repo: &Repository, root: &Path, title: &str, agent: &str) -> Result<ThreadTree, TreeError> {
    let base = repo.head()?.ok_or(TreeError::NoCommit)?;
    let base_branch = repo.current_branch()?;
    let folder = folder(root);
    std::fs::create_dir_all(&folder)?;
    let stem = slug(title);
    let mut n = 1;
    let (name, branch) = loop {
        let name = if n == 1 { stem.clone() } else { format!("{stem}-{n}") };
        let branch = format!("thread/{name}");
        let taken = folder.join(&name).exists()
            || folder.join(format!("{name}.json")).exists()
            || repo.resolve(&format!("refs/heads/{branch}"))?.is_some();
        if !taken {
            break (name, branch);
        }
        n += 1;
    };
    let path = folder.join(&name);
    repo.add_worktree(&path, &branch, Some(&base))?;
    let tree = ThreadTree {
        path: path.canonicalize().unwrap_or(path),
        branch,
        title: title.trim().to_string(),
        base,
        base_branch,
        agent: agent.to_string(),
    };
    write(&folder.join(format!("{name}.json")), &tree)?;
    Ok(tree)
}

fn write(file: &Path, tree: &ThreadTree) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(tree).map_err(std::io::Error::other)?;
    std::fs::write(file, text)
}

/// The thread worktrees kept for the project at `root`.
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

/// The thread worktree that holds `path`, for a file opened from it.
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

/// A thread finished a turn in `cwd`: when that is a thread's worktree, which the folder's watcher does not see, the
/// window's Git panel reads its changes.
pub fn turn_ended(controller: &crate::window::WindowController, cwd: &Path) {
    if cwd.starts_with(all_folder()) || all_folder().canonicalize().is_ok_and(|real| cwd.starts_with(real)) {
        read::mark(cwd);
        crate::git_panel::refresh(controller);
    }
}

/// The editor saved `path`: when it lies in a thread's worktree, the window's Git panel reads that worktree's changes.
pub fn file_saved(controller: &crate::window::WindowController, path: &Path) {
    if let Some(tree) = tree_of(path) {
        read::mark(&tree.path);
        crate::git_panel::refresh(controller);
    }
}

/// Forgets what the app kept about `tree`.
fn forget(tree: &ThreadTree) {
    read::evict(&tree.path);
    let file = tree.path.with_extension("json");
    if let Err(error) = std::fs::remove_file(&file)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        crate::diag::report("forgetting a thread worktree", error);
    }
}

/// Stages what the thread left uncommitted in its worktree and checks it for secrets (I4), before it is committed.
pub fn stage_pending(git: &Git, tree: &ThreadTree) -> ide_git::Result<SecretScan> {
    let copy = git.discover(&tree.path)?;
    let status = copy.status(StatusOptions::default())?;
    if !status.entries.iter().any(|e| e.kind != ide_git::EntryKind::Ignored) {
        return Ok(SecretScan::Clean);
    }
    copy.stage_all()?;
    copy.scan_staged()
}

/// Commits what the thread left uncommitted in its worktree, titled as the thread.
fn commit_pending(git: &Git, tree: &ThreadTree) -> ide_git::Result<()> {
    let copy = git.discover(&tree.path)?;
    copy.stage_all()?;
    match copy.commit(&tree.title, ide_git::CommitOptions::default()) {
        Ok(_) | Err(ide_git::Error::NothingToCommit) => Ok(()),
        Err(error) => Err(error),
    }
}

/// Brings the thread's work into the user's current branch, committing what it left uncommitted first. Conflicts
/// stop the merge or cherry-pick for the user, as `ide_git::Error::Conflicts`.
pub fn take_over(git: &Git, repo: &Repository, tree: &ThreadTree, how: TakeOver) -> ide_git::Result<()> {
    commit_pending(git, tree)?;
    match how {
        TakeOver::Merge => repo.merge_branch(&tree.branch, &format!("Merge branch '{}'", tree.branch)),
        TakeOver::CherryPick => {
            let base = repo
                .merge_base("HEAD", &tree.branch)?
                .unwrap_or_else(|| tree.base.clone());
            repo.cherry_pick_branch(&base, &tree.branch)
        }
    }
}

/// Removes the worktree but keeps its branch, with what the thread left uncommitted committed on it.
pub fn keep_branch(git: &Git, repo: &Repository, tree: &ThreadTree) -> ide_git::Result<()> {
    commit_pending(git, tree)?;
    repo.remove_worktree(&tree.path)?;
    forget(tree);
    Ok(())
}

/// Removes the worktree with whatever it holds and, if `delete_branch`, its branch with commits not merged anywhere;
/// only after the user confirmed it (E27).
pub fn remove(repo: &Repository, tree: &ThreadTree, delete_branch: bool) -> ide_git::Result<()> {
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
        assert_eq!(slug("Fix the Login!"), "fix-the-login");
        assert_eq!(slug("  Größe ändern  "), "groesse-aendern");
        assert_eq!(slug("Brücke über"), "bruecke-ueber");
        assert_eq!(slug("日本"), "thread");
        assert_eq!(slug("a/b..c"), "a-b-c");
        assert_eq!(slug(&"x".repeat(60)).len(), 40);
    }

    #[test]
    fn a_file_in_a_thread_worktree_finds_its_tree() {
        let tree = ThreadTree {
            path: folder(Path::new("/work/shop")).join("fix"),
            branch: "thread/fix".into(),
            title: "Fix".into(),
            base: "abc".into(),
            base_branch: Some("main".into()),
            agent: String::new(),
        };
        assert_eq!(tree.path.with_extension("json").file_name().unwrap(), "fix.json");
        assert_eq!(tree_of(Path::new("/work/shop/src/lib.rs")), None);
    }
}
