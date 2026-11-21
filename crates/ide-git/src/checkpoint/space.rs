//! The plumbing under checkpoints: snapshots built in a temporary index, trees of their parts, commits and refs that
//! leave the user's index, HEAD, branches and stash alone.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::diff::{FileDiff, diff_args, parse};
use crate::error::{Error, Result};
use crate::id::CommitId;
use crate::path::{bytes, from_bytes, nul_fields, relative};
use crate::run::{Call, Git, Output};

/// A working tree checkpoints are taken of, with the git folder that keeps them.
#[derive(Debug, Clone)]
pub(super) struct Space {
    pub(super) git: Git,
    pub(super) root: PathBuf,
    /// `GIT_DIR` and `GIT_WORK_TREE` for a folder's own store or a repository of the unattended git; empty for the
    /// user's repository.
    pub(super) env: Vec<(OsString, OsString)>,
    /// The index a snapshot starts from: the user's in a repository, the store's own in a folder.
    pub(super) index: PathBuf,
    /// Whether a snapshot's index replaces `index`, which only a folder's store does, to keep file stats for next time.
    pub(super) keeps_index: bool,
    /// Whether this is a repository with a HEAD and an index of the user's.
    pub(super) repository: bool,
}

/// The state of the working tree at one moment.
pub(super) struct Snapshot {
    pub(super) worktree: String,
    pub(super) index: Option<String>,
    pub(super) skipped: Vec<PathBuf>,
}

/// The trees a checkpoint commit holds, as entries of its own tree.
#[derive(Debug, Clone, Default)]
pub(super) struct Parts {
    /// The working tree, untracked files included.
    pub(super) worktree: String,
    /// The user's index; `None` outside a repository or during a merge conflict.
    pub(super) index: Option<String>,
    /// Unsaved editor buffers.
    pub(super) buffers: Option<String>,
    /// The working tree when the agent's turn ended.
    pub(super) after: Option<String>,
}

/// An index file of one snapshot's own, in a folder of its own that goes with its lock file when dropped.
struct TemporaryIndex {
    path: PathBuf,
    _dir: ide_process::private_dir::PrivateDir,
}

impl TemporaryIndex {
    fn new() -> Result<TemporaryIndex> {
        let dir = ide_process::private_dir::PrivateDir::new("ide-git-index")?;
        Ok(TemporaryIndex {
            path: dir.path().join("index"),
            _dir: dir,
        })
    }
}

impl Space {
    pub(super) fn call<I, S>(&self, args: I) -> Call<'_>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Call::new(&self.root, args).envs(&self.env)
    }

    fn indexed<'a, I, S>(&'a self, index: &TemporaryIndex, args: I) -> Call<'a>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.call(args).env("GIT_INDEX_FILE", &index.path)
    }

    fn ok(&self, call: Call<'_>) -> Result<Output> {
        self.git.ok(call)
    }

    /// The working tree as it is: tracked files with their changes and untracked files that are not ignored, except
    /// those larger than `max_file_size`, which are listed as skipped.
    pub(super) fn snapshot(&self, max_file_size: u64) -> Result<Snapshot> {
        let temporary = TemporaryIndex::new()?;
        if self.index.exists() {
            std::fs::copy(&self.index, &temporary.path)?;
        }
        self.ok(self.indexed(&temporary, ["add", "--update"]))?;
        let others = self.ok(self.indexed(&temporary, ["ls-files", "-z", "--others", "--exclude-standard"]))?;
        let mut added = Vec::new();
        let mut skipped = Vec::new();
        for field in nul_fields(&others.stdout) {
            let path = from_bytes(field);
            // A folder with a repository of its own is listed with a slash and not looked into.
            if field.ends_with(b"/") {
                skipped.push(path);
                continue;
            }
            match std::fs::symlink_metadata(self.root.join(&path)) {
                Ok(meta) if meta.file_type().is_symlink() || (meta.is_file() && meta.len() <= max_file_size) => {
                    added.extend_from_slice(field);
                    added.push(0);
                }
                Ok(_) => skipped.push(path),
                Err(_) => {}
            }
        }
        if !added.is_empty() {
            let call = self.indexed(&temporary, ["update-index", "--add", "--remove", "-z", "--stdin"]);
            self.ok(call.input(added))?;
        }
        let worktree = self.ok(self.indexed(&temporary, ["write-tree"]))?.text();
        if self.keeps_index {
            let replacement = self.index.with_extension("next");
            std::fs::copy(&temporary.path, &replacement)?;
            std::fs::rename(&replacement, &self.index)?;
        }
        let index = if self.repository { self.index_tree()? } else { None };
        Ok(Snapshot {
            worktree,
            index,
            skipped,
        })
    }

    /// The user's index as a tree, from a copy; `None` while it holds conflicts, which no tree can.
    fn index_tree(&self) -> Result<Option<String>> {
        let temporary = TemporaryIndex::new()?;
        if self.index.exists() {
            std::fs::copy(&self.index, &temporary.path)?;
        }
        let output = self.git.run(self.indexed(&temporary, ["write-tree"]))?;
        Ok((output.code == 0).then(|| output.text()))
    }

    /// The buffers as a tree of their exact bytes, without git's filters.
    pub(super) fn buffers_tree(&self, buffers: &[(PathBuf, Vec<u8>)]) -> Result<Option<String>> {
        if buffers.is_empty() {
            return Ok(None);
        }
        let mut info = Vec::new();
        for (path, content) in buffers {
            let path = relative(&self.root, path)?;
            let call = self
                .call(["hash-object", "-w", "--no-filters", "--stdin"])
                .input(content.clone());
            let blob = self.ok(call)?.text();
            info.extend_from_slice(format!("100644 {blob}\t").as_bytes());
            info.extend_from_slice(bytes(&path));
            info.push(0);
        }
        let temporary = TemporaryIndex::new()?;
        self.ok(self
            .indexed(&temporary, ["update-index", "-z", "--index-info"])
            .input(info))?;
        Ok(Some(self.ok(self.indexed(&temporary, ["write-tree"]))?.text()))
    }

    /// The tree of a checkpoint commit, one entry per part.
    pub(super) fn meta_tree(&self, parts: &Parts) -> Result<String> {
        let mut input = String::new();
        for (name, tree) in [
            ("worktree", Some(&parts.worktree)),
            ("index", parts.index.as_ref()),
            ("buffers", parts.buffers.as_ref()),
            ("after", parts.after.as_ref()),
        ] {
            if let Some(tree) = tree {
                input.push_str(&format!("040000 tree {tree}\t{name}\n"));
            }
        }
        Ok(self.ok(self.call(["mktree"]).input(input.into_bytes()))?.text())
    }

    /// The parts of the checkpoint commit `commit`.
    pub(super) fn parts(&self, commit: &str) -> Result<Parts> {
        let output = self.ok(self.call(["ls-tree", "-z", commit]))?;
        let mut parts = Parts::default();
        for entry in nul_fields(&output.stdout) {
            let text = String::from_utf8_lossy(entry);
            let Some((meta, name)) = text.split_once('\t') else {
                continue;
            };
            let Some(tree) = meta.split(' ').nth(2).map(String::from) else {
                continue;
            };
            match name {
                "worktree" => parts.worktree = tree,
                "index" => parts.index = Some(tree),
                "buffers" => parts.buffers = Some(tree),
                "after" => parts.after = Some(tree),
                _ => {}
            }
        }
        if parts.worktree.is_empty() {
            return Err(Error::Unexpected {
                what: format!("{commit} is not a checkpoint"),
            });
        }
        Ok(parts)
    }

    /// A commit of `tree` under the product's name, never signed, so no key or PIN prompt comes up.
    pub(super) fn commit_tree(
        &self,
        tree: &str,
        parent: Option<&str>,
        message: &str,
        created: Option<i64>,
    ) -> Result<String> {
        let email = format!("checkpoints@{}", ide_product::BUNDLE_ID);
        let mut call = self.call(["commit-tree", "--no-gpg-sign", tree]);
        if let Some(parent) = parent {
            call = call.args(["-p", parent]);
        }
        call = call
            .env("GIT_AUTHOR_NAME", ide_product::NAME)
            .env("GIT_AUTHOR_EMAIL", &email)
            .env("GIT_COMMITTER_NAME", ide_product::NAME)
            .env("GIT_COMMITTER_EMAIL", &email);
        if let Some(created) = created {
            call = call.env("GIT_AUTHOR_DATE", format!("@{created} +0000"));
        }
        Ok(self.ok(call.input(message.as_bytes().to_vec()))?.text())
    }

    /// Points `name` at `new` if it still points at `old`, or does not exist when `old` is empty.
    pub(super) fn update_ref(&self, name: &str, new: &str, old: &str) -> Result<Output> {
        self.git
            .run(self.call(["-c", "core.hooksPath=/dev/null", "update-ref", name, new, old]))
    }

    /// Deletes the refs, each only if it still points at its commit.
    pub(super) fn delete_refs(&self, refs: &[(String, String)]) -> Result<()> {
        if refs.is_empty() {
            return Ok(());
        }
        let input: String = refs
            .iter()
            .map(|(name, old)| format!("delete {name} {old}\n"))
            .collect();
        let call = self
            .call(["-c", "core.hooksPath=/dev/null", "update-ref", "--stdin"])
            .input(input.into_bytes());
        self.ok(call).map(drop)
    }

    /// The paths that differ between two trees, each with git's letter: `A` only in `to`, `D` only in `from`, `M`
    /// or `T` in both. Submodules are left out; their content is not in the trees.
    pub(super) fn changed(&self, from: &str, to: &str) -> Result<Vec<(u8, PathBuf)>> {
        let output = self.ok(self.call([
            "diff-tree",
            "-r",
            "-z",
            "--no-renames",
            "--name-status",
            "--ignore-submodules=all",
            from,
            to,
        ]))?;
        let mut fields = nul_fields(&output.stdout);
        let mut changed = Vec::new();
        while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
            changed.push((status.first().copied().unwrap_or(b'M'), from_bytes(path)));
        }
        Ok(changed)
    }

    /// The diff from tree `from` to tree `to`.
    pub(super) fn diff(&self, from: &str, to: &str) -> Result<Vec<FileDiff>> {
        let mut args = diff_args();
        args.extend(["-M", from, to].map(OsString::from));
        parse(&self.ok(self.call(args))?.stdout)
    }

    /// The commit HEAD points at, in a repository.
    pub(super) fn head(&self) -> Result<Option<CommitId>> {
        if !self.repository {
            return Ok(None);
        }
        let output = self
            .git
            .run(self.call(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]))?;
        Ok((output.code == 0).then(|| CommitId::from_git(output.text())))
    }

    /// Writes `paths` as they are in `tree` into the working tree, with the filters and modes git would use.
    pub(super) fn write_paths(&self, tree: &str, paths: &[PathBuf]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let temporary = TemporaryIndex::new()?;
        self.ok(self.indexed(&temporary, ["read-tree", tree]))?;
        let mut input = Vec::new();
        for path in paths {
            input.extend_from_slice(bytes(path));
            input.push(0);
        }
        let call = self.indexed(&temporary, ["checkout-index", "--force", "-z", "--stdin"]);
        self.ok(call.input(input)).map(drop)
    }

    /// Every file of `tree` with its bytes.
    pub(super) fn read_blobs(&self, tree: &str) -> Result<Vec<(PathBuf, Vec<u8>)>> {
        let listing = self.ok(self.call(["ls-tree", "-r", "-z", tree]))?;
        let mut entries = Vec::new();
        for entry in nul_fields(&listing.stdout) {
            let Some(tab) = entry.iter().position(|&b| b == b'\t') else {
                continue;
            };
            let meta = String::from_utf8_lossy(&entry[..tab]);
            if let Some(blob) = meta.split(' ').nth(2) {
                entries.push((from_bytes(&entry[tab + 1..]), blob.to_string()));
            }
        }
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let input: String = entries.iter().map(|(_, blob)| format!("{blob}\n")).collect();
        let output = self.ok(self.call(["cat-file", "--batch"]).input(input.into_bytes()))?;
        let mut rest = output.stdout.as_slice();
        let mut blobs = Vec::new();
        for (path, _) in entries {
            let unexpected = || Error::Unexpected {
                what: "cat-file output".into(),
            };
            let end = rest.iter().position(|&b| b == b'\n').ok_or_else(unexpected)?;
            let header = String::from_utf8_lossy(&rest[..end]).into_owned();
            let size: usize = header
                .rsplit(' ')
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(unexpected)?;
            let content = rest.get(end + 1..end + 1 + size).ok_or_else(unexpected)?;
            blobs.push((path, content.to_vec()));
            rest = rest.get(end + 2 + size..).unwrap_or_default();
        }
        Ok(blobs)
    }

    /// Makes the user's index `tree`, then lets git refresh the file stats it lost.
    pub(super) fn restore_index(&self, tree: &str) -> Result<()> {
        self.ok(self.call(["read-tree", tree]))?;
        self.git.run(self.call(["update-index", "-q", "--refresh"]))?;
        Ok(())
    }

    /// Removes `path` from the working tree, and the folders it leaves empty.
    pub(super) fn remove(&self, path: &Path) -> Result<()> {
        let full = self.root.join(path);
        match std::fs::remove_file(&full) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        let mut dir = full.parent();
        while let Some(current) = dir {
            if current == self.root || !current.starts_with(&self.root) || std::fs::remove_dir(current).is_err() {
                break;
            }
            dir = current.parent();
        }
        Ok(())
    }
}
