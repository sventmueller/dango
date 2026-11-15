//! Bringing a checkpoint back without overwriting what the user changed since, and with a way back.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::{Checkpoint, CheckpointKind, Checkpoints, Parts};
use crate::error::Result;
use crate::path::all_relative;

/// How to restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreOptions {
    /// Overwrite files the user changed since, too.
    pub force: bool,
    /// Files to leave as they are, such as those the user chose to keep after a report.
    pub keep: Vec<PathBuf>,
    /// Bring back the index as well, when HEAD has not moved since.
    pub index: bool,
}

impl Default for RestoreOptions {
    fn default() -> RestoreOptions {
        RestoreOptions {
            force: false,
            keep: Vec::new(),
            index: true,
        }
    }
}

/// The outcome of a restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restore {
    /// Nothing was changed, because the user changed these files after the checkpoint; restore with `force`, or with
    /// them in `keep`.
    UserChanges(Vec<PathBuf>),
    /// The checkpoint is back.
    Restored(Box<Restored>),
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// Files written with their content at the checkpoint.
    pub written: Vec<PathBuf>,
    /// Files deleted because they did not exist at the checkpoint.
    pub removed: Vec<PathBuf>,
    /// Files left as they are on the caller's request.
    pub kept: Vec<PathBuf>,
    /// The unsaved buffers at the checkpoint, for the editor to take back.
    pub buffers: Vec<(PathBuf, Vec<u8>)>,
    /// Whether the index was brought back.
    pub index_restored: bool,
    /// Whether HEAD points elsewhere than at the checkpoint, such as after an agent's commit; branches and HEAD are
    /// never moved by a restore.
    pub head_moved: bool,
    /// The checkpoint of the state before the restore, which undoes it.
    pub undo: Checkpoint,
}

impl Checkpoints {
    /// Brings the working tree back to `checkpoint`. Files the user changed after it — between the end of a turn and
    /// the start of the next, or since the last end — are reported and nothing changes, unless `options` force it or
    /// keep them. Files created since are deleted, except ignored ones, which no checkpoint holds. The state before is
    /// taken as a checkpoint first, so a restore can itself be restored away.
    pub fn restore(&self, checkpoint: &Checkpoint, options: &RestoreOptions) -> Result<Restore> {
        let target = self.space.parts(checkpoint.commit.as_str())?;
        let now = self.space.snapshot(self.max_file_size)?;
        let changed = self.space.changed(&target.worktree, &now.worktree)?;
        let keep: BTreeSet<PathBuf> = all_relative(&self.space.root, &options.keep)?.into_iter().collect();
        if !options.force {
            let user = self.user_changes(checkpoint, &now.worktree)?;
            let conflicts: Vec<PathBuf> = changed
                .iter()
                .map(|(_, path)| path)
                .filter(|path| user.contains(*path) && !keep.contains(*path))
                .cloned()
                .collect();
            if !conflicts.is_empty() {
                return Ok(Restore::UserChanges(conflicts));
            }
        }
        let before = Parts {
            worktree: now.worktree,
            index: now.index,
            buffers: None,
            after: None,
        };
        let undo = self.record(
            &checkpoint.thread,
            &checkpoint.label,
            CheckpointKind::BeforeRestore,
            &before,
        )?;
        let (mut written, mut removed, mut kept) = (Vec::new(), Vec::new(), Vec::new());
        for (status, path) in changed {
            if keep.contains(&path) {
                kept.push(path);
            } else if status == b'A' {
                removed.push(path);
            } else {
                written.push(path);
            }
        }
        for path in &removed {
            self.space.remove(path)?;
        }
        self.space.write_paths(&target.worktree, &written)?;
        let head_moved = self.space.repository && self.space.head()? != checkpoint.base;
        let index_restored = match (&target.index, options.index && self.space.repository && !head_moved) {
            (Some(index), true) => {
                self.space.restore_index(index)?;
                true
            }
            _ => false,
        };
        let buffers = match &target.buffers {
            Some(tree) => self.space.read_blobs(tree)?,
            None => Vec::new(),
        };
        let undo = self.finish(&undo)?;
        Ok(Restore::Restored(Box::new(Restored {
            written,
            removed,
            kept,
            buffers,
            index_restored,
            head_moved,
            undo,
        })))
    }

    /// The files changed outside the turns since `checkpoint`.
    fn user_changes(&self, checkpoint: &Checkpoint, now: &str) -> Result<BTreeSet<PathBuf>> {
        let later: Vec<Checkpoint> = self
            .list(Some(&checkpoint.thread))?
            .into_iter()
            .filter(|c| c.number >= checkpoint.number)
            .collect();
        let mut user = BTreeSet::new();
        for (i, current) in later.iter().enumerate() {
            if !current.finished {
                continue;
            }
            let Some(after) = self.space.parts(current.commit.as_str())?.after else {
                continue;
            };
            let next = match later.get(i + 1) {
                Some(next) => self.space.parts(next.commit.as_str())?.worktree,
                None => now.to_string(),
            };
            user.extend(self.space.changed(&after, &next)?.into_iter().map(|(_, path)| path));
        }
        Ok(user)
    }
}
