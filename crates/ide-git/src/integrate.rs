//! Bringing a branch's work over: how far two revisions apart are, what a working tree changed since a commit, and
//! merging or cherry-picking a branch into the current one, which stops at conflicts for the user (I1).

use std::ffi::OsString;
use std::path::PathBuf;

use crate::diff::{Change, FileDiff, diff_args, literal_args, parse};
use crate::error::{Error, Result};
use crate::id::CommitId;
use crate::path::{all_relative, from_bytes, nul_fields};
use crate::remote::word;
use crate::repo::Repository;
use crate::said::{Said, overwritten_paths};

/// A file a working tree changed since a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// Its path now, relative to the root; for a deleted file the one it had.
    pub path: PathBuf,
    /// What happened to it; an untracked file counts as added.
    pub change: Change,
    /// The path it had before a rename or copy.
    pub from: Option<PathBuf>,
}

impl Repository {
    /// How many commits `right` has that `left` lacks, and how many `left` has that `right` lacks.
    pub fn ahead_behind(&self, left: &str, right: &str) -> Result<(usize, usize)> {
        let range = format!("{}...{}", word(left)?, word(right)?);
        let output = self
            .git()
            .ok(self.call(["rev-list", "--left-right", "--count", range.as_str()]))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut counts = text.split_whitespace().map(str::parse::<usize>);
        match (counts.next(), counts.next()) {
            (Some(Ok(behind)), Some(Ok(ahead))) => Ok((ahead, behind)),
            _ => Err(Error::Unexpected {
                what: format!("rev-list --count: {text}"),
            }),
        }
    }

    /// The newest commit both `left` and `right` contain; `None` when their histories never meet.
    pub fn merge_base(&self, left: &str, right: &str) -> Result<Option<CommitId>> {
        let output = self.git().run(self.call(["merge-base", word(left)?, word(right)?]))?;
        match output.code {
            0 => Ok(Some(CommitId::from_git(output.text()))),
            1 => Ok(None),
            _ => Err(crate::run::failure(&output)),
        }
    }

    /// The files the working tree, staged or not, changed since `revision`, untracked ones among them.
    pub fn changes_since(&self, revision: &str) -> Result<Vec<ChangedFile>> {
        let revision = word(revision)?;
        let output = self.git().ok(self.call([
            "diff",
            "--name-status",
            "-z",
            "-M",
            "--ignore-submodules=none",
            revision,
            "--",
        ]))?;
        let mut files = name_status(&output.stdout);
        files.extend(self.untracked(&[])?.into_iter().map(|path| ChangedFile {
            path,
            change: Change::Added,
            from: None,
        }));
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// The files that differ between the commits `from` and `to`, each with what happened to it; a renamed or copied
    /// file keeps the path it had (C54).
    pub fn changes_between(&self, from: &str, to: &str) -> Result<Vec<ChangedFile>> {
        let output = self.git().ok(self.call([
            "diff",
            "--name-status",
            "-z",
            "-M",
            "--no-ext-diff",
            "--ignore-submodules=none",
            word(from)?,
            word(to)?,
            "--",
        ]))?;
        Ok(name_status(&output.stdout))
    }

    /// The whole diff from `from` to `to` as bytes, binary files included with full object names, the same for the
    /// same two trees whatever the user's diff settings: two such diffs are equal byte for byte exactly when the
    /// changes are, which a patch id does not tell, as it passes over whitespace (C56).
    pub fn full_diff(&self, from: &str, to: &str) -> Result<Vec<u8>> {
        let output = self.git().ok(self.call([
            "-c",
            "core.quotepath=true",
            "-c",
            "diff.suppressBlankEmpty=false",
            "diff",
            "--binary",
            "--full-index",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--no-relative",
            "--diff-algorithm=myers",
            "--indent-heuristic",
            "--unified=3",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--ignore-submodules=none",
            "--submodule=short",
            "-O/dev/null",
            word(from)?,
            word(to)?,
            "--",
        ]))?;
        Ok(output.stdout)
    }

    /// The diffs of `paths` between `revision` and the working tree, staged or not, untracked files as new ones; of
    /// every such file when `paths` is empty.
    pub fn diff_since<P: AsRef<std::path::Path>>(&self, revision: &str, paths: &[P]) -> Result<Vec<FileDiff>> {
        let paths = all_relative(self.root(), paths)?;
        let mut args = diff_args();
        args.extend(["-M".into(), word(revision)?.into(), OsString::from("--")]);
        args.extend(literal_args(&paths));
        let output = self.git().ok(self.call(args))?;
        let mut diffs = parse(&output.stdout)?;
        for path in self.untracked(&paths)? {
            diffs.push(self.diff_untracked(&path)?);
        }
        Ok(diffs)
    }

    /// The untracked files among `paths`, or all of them when `paths` is empty; ignored ones are left out.
    pub(crate) fn untracked(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut args: Vec<OsString> = ["ls-files", "-z", "--others", "--exclude-standard", "--"]
            .map(OsString::from)
            .to_vec();
        args.extend(literal_args(paths));
        let output = self.git().ok(self.call(args))?;
        Ok(nul_fields(&output.stdout).map(from_bytes).collect())
    }

    /// Merges `branch` into the current branch with a merge commit, even where a fast-forward would do, so the work
    /// stays one step to undo. Local changes the merge would overwrite give `Error::LocalChanges` and leave
    /// everything as it was; conflicts give `Error::Conflicts` with the merge waiting for the user.
    pub fn merge_branch(&self, branch: &str, message: &str) -> Result<()> {
        let branch = word(branch)?;
        self.integrate(["merge", "--no-ff", "-m", message, branch].map(OsString::from).to_vec())
    }

    /// Applies the commits of `branch` since `base` to the current branch one by one, like
    /// [`Repository::merge_branch`] with its errors; a stop at conflicts waits for the user.
    pub fn cherry_pick_branch(&self, base: &str, branch: &str) -> Result<()> {
        let range = format!("{}..{}", word(base)?, word(branch)?);
        self.integrate(vec!["cherry-pick".into(), range.into()])
    }

    fn integrate(&self, args: Vec<OsString>) -> Result<()> {
        let (output, failed_hook) = crate::hooks::run(self.git(), self.call(args).unlimited())?;
        if output.code == 0 {
            return Ok(());
        }
        let said = output.said();
        if Said::LocalChanges.in_text(&said) {
            return Err(Error::LocalChanges {
                paths: overwritten_paths(&said),
                output: said,
            });
        }
        let conflicts = self.unmerged_paths()?;
        if !conflicts.is_empty() {
            return Err(Error::Conflicts {
                paths: conflicts,
                output: said,
            });
        }
        if let Some(hook) = failed_hook {
            return Err(Error::HookFailed { hook, output: said });
        }
        Err(crate::run::failure(&output))
    }
}

/// The files of `git diff --name-status -z`: a status letter with a score, then one path, or two for a rename or
/// copy.
pub(crate) fn name_status(output: &[u8]) -> Vec<ChangedFile> {
    let mut fields = output.split(|&b| b == 0).filter(|f| !f.is_empty());
    let mut files = Vec::new();
    while let Some(status) = fields.next() {
        let score = std::str::from_utf8(&status[1..])
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let change = match status.first() {
            Some(b'A') => Change::Added,
            Some(b'D') => Change::Deleted,
            Some(b'R') => Change::Renamed { similarity: score },
            Some(b'C') => Change::Copied { similarity: score },
            Some(b'U') => Change::Unmerged,
            _ => Change::Modified,
        };
        let from = matches!(change, Change::Renamed { .. } | Change::Copied { .. })
            .then(|| fields.next().map(from_bytes))
            .flatten();
        if let Some(path) = fields.next() {
            files.push(ChangedFile {
                path: from_bytes(path),
                change,
                from,
            });
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_status_reads_letters_scores_and_the_new_path_of_a_rename() {
        let files = name_status(b"M\0src/a.rs\0R087\0old.rs\0new.rs\0A\0n\xc3\xa4u.txt\0D\0gone\0");
        let read: Vec<_> = files.iter().map(|f| (f.path.display().to_string(), f.change)).collect();
        assert_eq!(files[1].from, Some(PathBuf::from("old.rs")));
        assert_eq!(files[0].from, None);
        assert_eq!(
            read,
            [
                ("src/a.rs".to_string(), Change::Modified),
                ("new.rs".to_string(), Change::Renamed { similarity: 87 }),
                ("näu.txt".to_string(), Change::Added),
                ("gone".to_string(), Change::Deleted),
            ]
        );
    }
}
