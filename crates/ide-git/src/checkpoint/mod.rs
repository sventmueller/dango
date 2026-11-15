//! Checkpoints before an agent's turn (I6): the working tree with untracked files, unsaved buffers and the index,
//! kept as commits under hidden refs, restorable without overwriting what the user changed since.
//!
//! A checkpoint is a commit whose tree holds the parts as folders — `worktree`, `index`, `buffers` and, once the
//! turn has ended, `after` — reachable from `refs/ide-checkpoints/<thread>/<n>`. The refs lie outside `refs/heads`
//! and `refs/tags`, so a normal push or fetch never carries them; `git push --mirror` would, and `git log --all`
//! shows them. A folder outside any repository gets a git object store of its own under
//! `ide_core::paths::data_dir()/checkpoints/`, so the same code and API serve both.
//!
//! Who changed what is told by turns: [`Checkpoints::create`] before the agent starts, [`Checkpoints::finish`] when
//! its turn ends. What changed between one turn's end and the next turn's start, or since the last end, is the
//! user's, and [`Checkpoints::restore`] reports those files instead of overwriting them.

mod restore;
mod space;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::diff::FileDiff;
use crate::error::{Error, Result};
use crate::id::CommitId;
use crate::repo::Repository;
use crate::run::{Call, Git};
use crate::staging::apply_hunks;

use space::{Parts, Space};

pub use restore::{Restore, RestoreOptions, Restored};

/// Where the refs live.
const REFS: &str = "refs/ide-checkpoints/";

/// Untracked files larger than this are left out of a checkpoint unless the caller sets another limit.
const DEFAULT_MAX_FILE_SIZE: u64 = 16 * 1024 * 1024;

/// What a folder's own store leaves out, besides the folder's `.gitignore` files: dependencies and caches that are
/// large and can be rebuilt.
const FOLDER_EXCLUDES: &str = ".DS_Store\nnode_modules/\n.venv/\nvenv/\n__pycache__/\n.mypy_cache/\n.pytest_cache/\n\
.tox/\n.gradle/\n.next/\n.nuxt/\n.cache/\nDerivedData/\nPods/\ntarget/\n.build/\n";

/// The checkpoints of one repository or folder.
#[derive(Debug, Clone)]
pub struct Checkpoints {
    space: Space,
    max_file_size: u64,
}

/// The name of the thread checkpoints belong to, which is one part of their ref names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CheckpointThread(String);

impl CheckpointThread {
    /// `name` as a thread's name: letters, digits, `.`, `_` and `-`, at most 128, not starting with `.` or `-`, and
    /// nothing else git refuses in a ref name; otherwise `Error::InvalidName`.
    pub fn new(name: impl Into<String>) -> Result<CheckpointThread> {
        let name = name.into();
        let fine = !name.is_empty()
            && name.len() <= 128
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            && !name.starts_with(['.', '-'])
            && !name.ends_with(".lock")
            && !name.contains("..");
        if fine {
            Ok(CheckpointThread(name))
        } else {
            Err(Error::InvalidName { name })
        }
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CheckpointThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A checkpoint as listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// The thread it belongs to.
    pub thread: CheckpointThread,
    /// Its number in the thread, counting from 1.
    pub number: u64,
    /// The commit holding it.
    pub commit: CommitId,
    /// The caller's label, on one line.
    pub label: String,
    /// When it was taken.
    pub(crate) created: SystemTime,
    /// Why it was taken.
    pub kind: CheckpointKind,
    /// Whether the turn it started has ended.
    pub finished: bool,
    /// The commit HEAD pointed at; `None` outside a repository or before its first commit.
    pub base: Option<CommitId>,
}

/// Why a checkpoint was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointKind {
    /// Before an agent's turn.
    Turn,
    /// Before a restore, so that the restore can be undone.
    BeforeRestore,
}

impl CheckpointKind {
    fn name(self) -> &'static str {
        match self {
            CheckpointKind::Turn => "turn",
            CheckpointKind::BeforeRestore => "before-restore",
        }
    }
}

/// A new checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Created {
    pub checkpoint: Checkpoint,
    /// Untracked files left out: larger than the limit, or folders with a repository of their own.
    pub skipped: Vec<PathBuf>,
}

/// Which checkpoints [`Checkpoints::prune`] deletes; a checkpoint goes when any rule says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Prune {
    /// Keep only this many of each thread, the newest.
    pub keep_last: Option<usize>,
    /// Delete those taken before this moment.
    pub older_than: Option<SystemTime>,
}

impl Checkpoints {
    /// The checkpoints of `repository`, kept in its own object store.
    pub fn for_repository(repository: &Repository) -> Checkpoints {
        let space = Space {
            git: repository.git().clone(),
            root: repository.root().to_path_buf(),
            env: Vec::new(),
            index: repository.index_file(),
            keeps_index: false,
            repository: true,
        };
        Checkpoints {
            space,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        }
    }

    /// The checkpoints of `folder`, which is in no repository, kept in a store of its own under `store`.
    pub fn for_folder(git: &Git, folder: &Path, store: &Path) -> Result<Checkpoints> {
        let folder = folder.canonicalize()?;
        let git_dir = store.join(store_name(&folder));
        if !git_dir.join("HEAD").is_file() {
            create_store(git, store, &git_dir, &folder)?;
        }
        let output = git.run(store_config(
            store,
            &git_dir,
            &["--get".as_ref(), "ide.folder".as_ref()],
        ))?;
        if output.stdout.strip_suffix(b"\n") != Some(folder.as_os_str().as_bytes()) {
            return Err(Error::Unexpected {
                what: format!("{} belongs to another folder", git_dir.display()),
            });
        }
        let space = Space {
            git: git.clone(),
            env: vec![
                ("GIT_DIR".into(), git_dir.clone().into()),
                ("GIT_WORK_TREE".into(), folder.clone().into()),
            ],
            root: folder,
            index: git_dir.join("index"),
            keeps_index: true,
            repository: false,
        };
        Ok(Checkpoints {
            space,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        })
    }

    /// The checkpoints of the repository `folder` is in, or of the folder itself in the app's store.
    pub fn open(git: &Git, folder: &Path) -> Result<Checkpoints> {
        match git.discover(folder) {
            Ok(repository) => Ok(Checkpoints::for_repository(&repository)),
            Err(Error::NotARepository { .. }) => Checkpoints::for_folder(git, folder, &Checkpoints::default_store()),
            Err(error) => Err(error),
        }
    }

    /// The app's store for folders outside repositories.
    pub(crate) fn default_store() -> PathBuf {
        ide_core::paths::data_dir().join("checkpoints")
    }

    /// Leaves untracked files larger than `bytes` out of new checkpoints.
    #[must_use]
    pub fn with_max_file_size(mut self, bytes: u64) -> Checkpoints {
        self.max_file_size = bytes;
        self
    }

    /// The folder checkpoints are taken of: the repository's root or the folder.
    pub fn root(&self) -> &Path {
        &self.space.root
    }

    /// Whether the checkpoints live in the user's repository.
    pub fn in_repository(&self) -> bool {
        self.space.repository
    }

    /// Takes a checkpoint for `thread` before an agent's turn. `buffers` are unsaved editor contents by path, kept
    /// byte for byte and handed back by a restore; the files on disk are kept as they are.
    pub fn create(&self, thread: &CheckpointThread, label: &str, buffers: &[(PathBuf, Vec<u8>)]) -> Result<Created> {
        let snapshot = self.space.snapshot(self.max_file_size)?;
        let parts = Parts {
            worktree: snapshot.worktree,
            index: snapshot.index,
            buffers: self.space.buffers_tree(buffers)?,
            after: None,
        };
        let checkpoint = self.record(thread, label, CheckpointKind::Turn, &parts)?;
        Ok(Created {
            checkpoint,
            skipped: snapshot.skipped,
        })
    }

    /// Records the state the turn `checkpoint` started has ended in; what changes after it is the user's.
    pub fn finish(&self, checkpoint: &Checkpoint) -> Result<Checkpoint> {
        let mut parts = self.space.parts(checkpoint.commit.as_str())?;
        parts.after = Some(self.space.snapshot(self.max_file_size)?.worktree);
        let tree = self.space.meta_tree(&parts)?;
        let message = message(&checkpoint.label, checkpoint.kind, Some(unix(SystemTime::now())));
        let created = unix(checkpoint.created);
        let base = checkpoint.base.as_ref().map(CommitId::as_str);
        let commit = self.space.commit_tree(&tree, base, &message, Some(created))?;
        let name = ref_name(&checkpoint.thread, checkpoint.number);
        let output = self.space.update_ref(&name, &commit, checkpoint.commit.as_str())?;
        if output.code != 0 {
            return Err(crate::run::failure(&output));
        }
        self.get(&checkpoint.thread, checkpoint.number)
    }

    /// The checkpoints of `thread`, or of every thread, ordered by thread and number.
    pub fn list(&self, thread: Option<&CheckpointThread>) -> Result<Vec<Checkpoint>> {
        let pattern = match thread {
            Some(thread) => format!("{REFS}{thread}/"),
            None => REFS.to_string(),
        };
        self.read(&pattern)
    }

    /// The changes from `checkpoint` to the working tree now.
    pub fn diff(&self, checkpoint: &Checkpoint) -> Result<Vec<FileDiff>> {
        let parts = self.space.parts(checkpoint.commit.as_str())?;
        let now = self.space.snapshot(self.max_file_size)?;
        self.space.diff(&parts.worktree, &now.worktree)
    }

    /// The changes of the turn `checkpoint` started: to its end, or to now while it runs.
    pub fn turn_diff(&self, checkpoint: &Checkpoint) -> Result<Vec<FileDiff>> {
        let parts = self.space.parts(checkpoint.commit.as_str())?;
        let to = match parts.after {
            Some(after) => after,
            None => self.space.snapshot(self.max_file_size)?.worktree,
        };
        self.space.diff(&parts.worktree, &to)
    }

    /// Takes back the hunks of `diff` at `hunks` in the working tree; `diff` comes from [`Checkpoints::diff`] or
    /// [`Checkpoints::turn_diff`] and must still match the files.
    pub fn revert_hunks(&self, diff: &FileDiff, hunks: &[usize]) -> Result<()> {
        apply_hunks(&self.space.git, self.space.call(["apply"]), diff, hunks, true)
    }

    /// Deletes `checkpoint`; git drops what only it kept at its next garbage collection.
    pub fn delete(&self, checkpoint: &Checkpoint) -> Result<()> {
        self.space.delete_refs(&[(
            ref_name(&checkpoint.thread, checkpoint.number),
            checkpoint.commit.to_string(),
        )])?;
        self.collect_garbage()
    }

    /// Deletes every checkpoint of `thread` and returns how many there were.
    pub fn delete_thread(&self, thread: &CheckpointThread) -> Result<usize> {
        let doomed = self.list(Some(thread))?;
        self.delete_all(&doomed)
    }

    /// Deletes the checkpoints `rules` select and returns how many.
    pub fn prune(&self, rules: &Prune) -> Result<usize> {
        let all = self.list(None)?;
        let mut doomed = Vec::new();
        for (i, checkpoint) in all.iter().enumerate() {
            let newer = all[i + 1..].iter().filter(|c| c.thread == checkpoint.thread).count();
            let too_many = rules.keep_last.is_some_and(|keep| newer >= keep);
            let too_old = rules.older_than.is_some_and(|limit| checkpoint.created < limit);
            if too_many || too_old {
                doomed.push(checkpoint.clone());
            }
        }
        self.delete_all(&doomed)
    }

    fn delete_all(&self, doomed: &[Checkpoint]) -> Result<usize> {
        let refs: Vec<(String, String)> = doomed
            .iter()
            .map(|c| (ref_name(&c.thread, c.number), c.commit.to_string()))
            .collect();
        self.space.delete_refs(&refs)?;
        self.collect_garbage()?;
        Ok(refs.len())
    }

    /// Lets a folder's own store shrink; a repository's garbage is the user's git's to collect.
    fn collect_garbage(&self) -> Result<()> {
        if !self.space.repository {
            self.space.git.run(self.space.call(["gc", "--auto", "--quiet"]))?;
        }
        Ok(())
    }

    /// Commits `parts` and points the thread's next free ref at it.
    fn record(
        &self,
        thread: &CheckpointThread,
        label: &str,
        kind: CheckpointKind,
        parts: &Parts,
    ) -> Result<Checkpoint> {
        let tree = self.space.meta_tree(parts)?;
        let base = self.space.head()?;
        let commit = self.space.commit_tree(
            &tree,
            base.as_ref().map(CommitId::as_str),
            &message(label, kind, None),
            None,
        )?;
        let first = self.list(Some(thread))?.last().map_or(1, |c| c.number + 1);
        let mut last = None;
        // Another window may take the same number at the same moment; the ref is only created if it is free.
        for number in (first..).take(8) {
            let output = self.space.update_ref(&ref_name(thread, number), &commit, "")?;
            if output.code == 0 {
                return self.get(thread, number);
            }
            last = Some(output);
        }
        Err(last.map_or(
            Error::Unexpected {
                what: "no free checkpoint number".into(),
            },
            |o| crate::run::failure(&o),
        ))
    }

    fn get(&self, thread: &CheckpointThread, number: u64) -> Result<Checkpoint> {
        self.read(&ref_name(thread, number))?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Unexpected {
                what: format!("checkpoint {thread}/{number} is gone"),
            })
    }

    fn read(&self, pattern: &str) -> Result<Vec<Checkpoint>> {
        let output = self.space.git.ok(self.space.call([
            "for-each-ref",
            "--format=%(refname)%1f%(objectname)%1f%(authordate:unix)%1f%(parent)%1f%(contents)%00",
            pattern,
        ]))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut checkpoints: Vec<Checkpoint> = text.split("\0\n").filter_map(parse_checkpoint).collect();
        checkpoints.sort_by(|a, b| (&a.thread, a.number).cmp(&(&b.thread, b.number)));
        Ok(checkpoints)
    }
}

/// Sets up a folder's store under a temporary name and moves it into place, so that a store is either complete or
/// absent, also when two windows open the folder at once.
fn create_store(git: &Git, store: &Path, git_dir: &Path, folder: &Path) -> Result<()> {
    std::fs::create_dir_all(store)?;
    let partial = git_dir.with_extension(format!("{}.partial", std::process::id()));
    let _ = std::fs::remove_dir_all(&partial);
    git.ok(Call::new(store, ["init", "--bare", "--quiet", "--initial-branch=main"]).args([&partial]))?;
    for (key, value) in [
        ("core.bare", "false"),
        ("core.autocrlf", "false"),
        ("core.fsmonitor", "false"),
        ("core.hooksPath", "/dev/null"),
    ] {
        git.ok(store_config(store, &partial, &[key.as_ref(), value.as_ref()]))?;
    }
    git.ok(store_config(
        store,
        &partial,
        &["ide.folder".as_ref(), folder.as_os_str()],
    ))?;
    std::fs::create_dir_all(partial.join("info"))?;
    std::fs::write(partial.join("info/exclude"), FOLDER_EXCLUDES)?;
    if std::fs::rename(&partial, git_dir).is_err() {
        std::fs::remove_dir_all(&partial)?;
        if !git_dir.join("HEAD").is_file() {
            return Err(Error::Unexpected {
                what: format!("{} could not be created", git_dir.display()),
            });
        }
    }
    Ok(())
}

/// `git config <args>` on a folder's store.
fn store_config<'a>(store: &'a Path, git_dir: &Path, args: &[&OsStr]) -> Call<'a> {
    Call::new(
        store,
        [OsStr::new("--git-dir"), git_dir.as_os_str(), OsStr::new("config")],
    )
    .args(args.iter().copied())
}

fn ref_name(thread: &CheckpointThread, number: u64) -> String {
    format!("{REFS}{thread}/{number}")
}

fn message(label: &str, kind: CheckpointKind, turn_end: Option<i64>) -> String {
    let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
    let label = if label.is_empty() {
        "checkpoint".to_string()
    } else {
        label
    };
    let mut message = format!("{label}\n\nIde-Checkpoint: {}\n", kind.name());
    if let Some(end) = turn_end {
        message.push_str(&format!("Ide-Turn-End: {end}\n"));
    }
    message
}

fn parse_checkpoint(record: &str) -> Option<Checkpoint> {
    let record = record.trim_start_matches('\n').trim_end_matches('\0');
    let fields: Vec<&str> = record.splitn(5, '\u{1f}').collect();
    let [refname, commit, date, parents, contents] = fields[..] else {
        return None;
    };
    let (thread, number) = refname.strip_prefix(REFS)?.rsplit_once('/')?;
    let mut kind = CheckpointKind::Turn;
    let mut finished = false;
    for line in contents.lines() {
        match line.split_once(": ") {
            Some(("Ide-Checkpoint", "before-restore")) => kind = CheckpointKind::BeforeRestore,
            Some(("Ide-Turn-End", _)) => finished = true,
            _ => {}
        }
    }
    Some(Checkpoint {
        // Refs another program made keep their name, so that pruning still finds them.
        thread: CheckpointThread(thread.to_string()),
        number: number.parse().ok()?,
        commit: CommitId::from_git(commit),
        label: contents.lines().next().unwrap_or_default().to_string(),
        created: UNIX_EPOCH + Duration::from_secs(date.parse().unwrap_or(0)),
        kind,
        finished,
        base: parents.split_whitespace().next().map(CommitId::from_git),
    })
}

fn unix(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The store's folder name: readable from the folder's name, unique by a hash of its whole path.
fn store_name(folder: &Path) -> String {
    let readable: String = folder
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    // FNV-1a, which unlike std's hasher is the same in every Rust release.
    let hash = folder
        .as_os_str()
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("{readable}-{hash:016x}.git")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_names_must_fit_in_a_ref() {
        assert_eq!(CheckpointThread::new("thread-1_a.b").unwrap().as_str(), "thread-1_a.b");
        for bad in ["", "a/b", "a b", ".hidden", "-x", "x.lock", "a..b", "ü"] {
            assert!(
                matches!(CheckpointThread::new(bad), Err(Error::InvalidName { name }) if name == bad),
                "{bad}"
            );
        }
        assert!(CheckpointThread::new("x".repeat(128)).is_ok() && CheckpointThread::new("x".repeat(129)).is_err());
    }

    #[test]
    fn a_listed_checkpoint_is_read_from_its_ref_and_message() {
        let record = "refs/ide-checkpoints/t1/12\u{1f}abc\u{1f}1794247537\u{1f}head\u{1f}Fix the parser\n\n\
                      Ide-Checkpoint: before-restore\nIde-Turn-End: 1794247600\n\0";
        let checkpoint = parse_checkpoint(record).unwrap();
        assert_eq!((checkpoint.thread.as_str(), checkpoint.number), ("t1", 12));
        assert_eq!(checkpoint.label, "Fix the parser");
        assert_eq!(checkpoint.kind, CheckpointKind::BeforeRestore);
        assert!(checkpoint.finished);
        assert_eq!(checkpoint.base.as_ref().map(CommitId::as_str), Some("head"));
        assert_eq!(unix(checkpoint.created), 1_794_247_537);
    }

    #[test]
    fn labels_become_one_line() {
        assert!(
            message("  two\nlines ", CheckpointKind::Turn, None).starts_with("two lines\n\nIde-Checkpoint: turn\n")
        );
        assert!(message("", CheckpointKind::Turn, Some(5)).starts_with("checkpoint\n"));
    }

    #[test]
    fn store_names_are_readable_and_differ_by_path() {
        let a = store_name(Path::new("/Users/x/My Project"));
        let b = store_name(Path::new("/Users/y/My Project"));
        assert!(a.starts_with("My_Project-") && a.ends_with(".git"));
        assert_ne!(a, b);
    }
}
