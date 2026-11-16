//! Finding the repository a folder belongs to.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::id::CommitId;
use crate::run::{Call, Git};

/// A git repository with a working tree, as seen from one of its worktrees.
#[derive(Debug, Clone)]
pub struct Repository {
    git: Git,
    root: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
}

/// Where an unattended git finds repositories (E192): a working copy and its git folders, read from that copy, never
/// from a worktree's `.git` file, which a builder or a gate could point at a configuration of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Anchor {
    root: PathBuf,
    /// Empty when the working copy has no git folder git could use, which leaves nothing to find.
    git_dir: PathBuf,
    common_dir: PathBuf,
}

impl Anchor {
    /// The anchor of the working copy `folder` lies in, from the `.git` of its top folder.
    pub(crate) fn at(folder: &Path) -> Anchor {
        let folder = real(folder);
        let root = folder
            .ancestors()
            .find(|at| std::fs::symlink_metadata(at.join(".git")).is_ok())
            .map_or_else(|| folder.clone(), Path::to_path_buf);
        let dot_git = root.join(".git");
        let git_dir = match std::fs::symlink_metadata(&dot_git) {
            Ok(meta) if meta.is_dir() => dot_git.canonicalize().ok(),
            Ok(meta) if meta.is_file() => std::fs::read_to_string(&dot_git)
                .ok()
                .and_then(|text| pointed(&root, &text)),
            _ => None,
        };
        let common_dir = git_dir.as_deref().and_then(common_of);
        match (git_dir, common_dir) {
            (Some(git_dir), Some(common_dir)) => Anchor {
                root,
                git_dir,
                common_dir,
            },
            _ => Anchor {
                root,
                git_dir: PathBuf::new(),
                common_dir: PathBuf::new(),
            },
        }
    }

    /// The anchor of a repository git found.
    fn of(root: &Path, git_dir: &Path, common_dir: &Path) -> Anchor {
        Anchor {
            root: real(root),
            git_dir: real(git_dir),
            common_dir: real(common_dir),
        }
    }

    /// The git folder of the worktree whose top folder is `root`: the anchor's own, or the one below the shared git
    /// folder whose `gitdir` record names `root`.
    fn git_dir_of(&self, root: &Path) -> Option<PathBuf> {
        if self.git_dir.as_os_str().is_empty() {
            return None;
        }
        if root == self.root {
            return Some(self.git_dir.clone());
        }
        let entries = std::fs::read_dir(self.common_dir.join("worktrees")).ok()?;
        entries.flatten().map(|entry| entry.path()).find(|admin| {
            std::fs::read_to_string(admin.join("gitdir"))
                .ok()
                .and_then(|text| {
                    // git writes the path relative to this folder when `worktree.useRelativePaths` is set.
                    let dot_git = admin.join(text.trim_end_matches(['\n', '\r']));
                    dot_git.parent()?.canonicalize().ok()
                })
                .is_some_and(|named| named == root)
        })
    }

    /// Whether a repository git found through the `.git` of its top folder can be taken without a bound working copy.
    /// Builders and gates write in the worktrees and the app's data folder, so the shared git folder must lie outside
    /// both; a linked worktree must be one the shared git folder's own records name; any other repository must lie
    /// outside the app's data folder and have its `.git` folder, or a git folder outside its working tree.
    fn trusted(&self, linked: bool) -> bool {
        let data = real(&ide_core::paths::data_dir());
        if self.common_dir.starts_with(&data) {
            return false;
        }
        if linked {
            return !self.common_dir.starts_with(&self.root)
                && self.git_dir_of_worktree(&self.root).as_ref() == Some(&self.git_dir);
        }
        !self.root.starts_with(&data)
            && (self.git_dir == self.root.join(".git") || !self.git_dir.starts_with(&self.root))
    }

    /// [`Anchor::git_dir_of`] for a worktree other than the anchor's own top folder.
    fn git_dir_of_worktree(&self, root: &Path) -> Option<PathBuf> {
        let other = Anchor {
            root: PathBuf::new(),
            ..self.clone()
        };
        other.git_dir_of(root)
    }
}

/// `path` with its links resolved, or as it is when it does not exist.
fn real(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The folder a `.git` file in `root` points at with `text`.
fn pointed(root: &Path, text: &str) -> Option<PathBuf> {
    let target = text.lines().next()?.strip_prefix("gitdir:")?.trim();
    root.join(target).canonicalize().ok()
}

/// The shared git folder of `git_dir`: the one its `commondir` file names, or itself.
fn common_of(git_dir: &Path) -> Option<PathBuf> {
    match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(text) => git_dir.join(text.trim_end_matches(['\n', '\r'])).canonicalize().ok(),
        Err(_) => Some(git_dir.to_path_buf()),
    }
}

impl Git {
    /// The repository `path` lies in; `path` may be a file or a folder anywhere inside the working tree.
    ///
    /// An unattended git bound to a working copy ([`Git::bound_to`]) finds only that copy and its repository's
    /// worktrees, through the records in its git folder; one that is not finds what git finds, but only a repository
    /// whose git folder lies where no builder or gate writes.
    pub fn discover(&self, path: &Path) -> Result<Repository> {
        if self.is_unattended()
            && let Some(anchor) = self.anchor()
        {
            return self.discover_anchored(path, anchor);
        }
        let found = self.discover_through_git(path)?;
        if !self.is_unattended() {
            return Ok(found);
        }
        let anchor = Anchor::of(&found.root, &found.git_dir, &found.common_dir);
        if !anchor.trusted(found.is_linked_worktree()) {
            return Err(Error::NotARepository {
                path: path.to_path_buf(),
            });
        }
        Ok(Repository {
            git: self.anchored(anchor.clone()),
            root: anchor.root,
            git_dir: anchor.git_dir,
            common_dir: anchor.common_dir,
        })
    }

    /// The working copy or worktree of `anchor` that `path` lies in, without asking git or reading a `.git` file.
    fn discover_anchored(&self, path: &Path, anchor: &Anchor) -> Result<Repository> {
        let refused = || Error::NotARepository {
            path: path.to_path_buf(),
        };
        let found = path.canonicalize().map_err(|_| refused())?;
        let start = if found.is_dir() {
            found.as_path()
        } else {
            found.parent().ok_or_else(refused)?
        };
        let root = start
            .ancestors()
            .find(|folder| std::fs::symlink_metadata(folder.join(".git")).is_ok())
            .ok_or_else(refused)?;
        let git_dir = anchor.git_dir_of(root).ok_or_else(refused)?;
        Ok(Repository {
            git: self.clone(),
            root: root.to_path_buf(),
            git_dir,
            common_dir: anchor.common_dir.clone(),
        })
    }

    /// The repository git finds for `path` through the `.git` it comes across.
    fn discover_through_git(&self, path: &Path) -> Result<Repository> {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        if !dir.is_dir() {
            return Err(Error::NotARepository {
                path: path.to_path_buf(),
            });
        }
        let output = self.run(Call::new(
            dir,
            [
                "rev-parse",
                "--path-format=absolute",
                "--is-bare-repository",
                "--git-dir",
                "--git-common-dir",
            ],
        ))?;
        if output.code != 0 {
            return Err(match crate::run::failure(&output) {
                Error::Failed { .. } => Error::NotARepository {
                    path: path.to_path_buf(),
                },
                other => other,
            });
        }
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        let mut lines = text.lines();
        let (Some(bare), Some(git_dir), Some(common_dir)) = (lines.next(), lines.next(), lines.next()) else {
            return Err(Error::Unexpected { what: text });
        };
        if bare == "true" {
            return Err(Error::BareRepository {
                path: PathBuf::from(git_dir),
            });
        }
        let top = self.run(Call::new(
            dir,
            ["rev-parse", "--path-format=absolute", "--show-toplevel"],
        ))?;
        if top.code != 0 || top.text().is_empty() {
            return Err(Error::NotARepository {
                path: path.to_path_buf(),
            });
        }
        Ok(Repository {
            git: self.clone(),
            root: PathBuf::from(top.text()),
            git_dir: PathBuf::from(git_dir),
            common_dir: PathBuf::from(common_dir),
        })
    }

    /// Makes the existing folder `folder` a new repository with git's default branch name, and returns it.
    pub fn init(&self, folder: &Path) -> Result<Repository> {
        if !folder.is_dir() {
            return Err(Error::NotARepository {
                path: folder.to_path_buf(),
            });
        }
        self.ok(Call::new(folder, ["init", "--quiet"]))?;
        self.discover(folder)
    }
}

impl Repository {
    /// The top folder of this working tree.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// This working tree's git folder: `.git`, or for a linked worktree its folder under `.git/worktrees`.
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// The git folder all worktrees of the repository share.
    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    /// Whether this working tree was added with `git worktree add`.
    pub fn is_linked_worktree(&self) -> bool {
        self.git_dir != self.common_dir
    }

    /// This repository for the orchestrator's commits, merges and worktree commands (E192): through
    /// [`Git::unattended`], without hooks, with `environment` and failing once `deadline` passed.
    pub fn unattended(&self, environment: Vec<(String, String)>, deadline: std::time::Instant) -> Repository {
        let anchor = Anchor::of(&self.root, &self.git_dir, &self.common_dir);
        Repository {
            git: self.git.unattended(environment, deadline).anchored(anchor),
            ..self.clone()
        }
    }

    /// Whether this repository runs git without hooks, from [`Repository::unattended`].
    pub fn is_unattended(&self) -> bool {
        self.git.is_unattended()
    }

    /// The git this repository runs.
    pub(crate) fn git(&self) -> &Git {
        &self.git
    }

    /// The commit HEAD points at; `None` on a branch without commits.
    pub fn head(&self) -> Result<Option<CommitId>> {
        self.resolve("HEAD")
    }

    /// The commit `revision` names, or `None` when it names none.
    pub fn resolve(&self, revision: &str) -> Result<Option<CommitId>> {
        let spec = format!("{revision}^{{commit}}");
        let output = self
            .git
            .run(self.call(["rev-parse", "--verify", "--quiet", "--end-of-options", &spec]))?;
        Ok((output.code == 0).then(|| CommitId::from_git(output.text())))
    }

    /// The tree without entries, whose name depends on the repository's hash.
    pub(crate) fn empty_tree(&self) -> Result<String> {
        Ok(self
            .git
            .ok(self.call(["hash-object", "-t", "tree", "/dev/null"]))?
            .text())
    }

    /// This working tree's index file.
    pub(crate) fn index_file(&self) -> PathBuf {
        self.git_dir.join("index")
    }

    /// `git <args>` at the root.
    pub(crate) fn call<I, S>(&self, args: I) -> Call<'_>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Call::new(&self.root, args).envs(&self.pinned())
    }

    /// For an unattended git, `GIT_DIR` and `GIT_WORK_TREE` naming this repository, so no call reads the working tree's
    /// `.git`, which may have changed since the repository was found (E192); nothing for the user's git.
    pub(crate) fn pinned(&self) -> Vec<(OsString, OsString)> {
        if !self.git.is_unattended() {
            return Vec::new();
        }
        vec![
            ("GIT_DIR".into(), self.git_dir.clone().into_os_string()),
            ("GIT_WORK_TREE".into(), self.root.clone().into_os_string()),
        ]
    }
}
