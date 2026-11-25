//! Every ref of a repository at one moment, and what changed between two such moments: the ref watch of work orders
//! (E173, C47), which after each turn looks whether anything moved branches, checkpoints, seals, notes or the stash of
//! the user's repository. It detects, it does not protect: a builder that can write the shared git folder can change
//! refs, and only the comparison afterwards tells.

use std::collections::BTreeMap;

use crate::error::Result;
use crate::repo::Repository;

/// What a ref is, by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefKind {
    /// `HEAD` of the working tree the snapshot was taken in.
    Head,
    /// A local branch, `refs/heads/…`.
    Branch,
    /// A remote-tracking branch, `refs/remotes/…`.
    Remote,
    /// A tag, `refs/tags/…`.
    Tag,
    /// A checkpoint before an agent's turn, `refs/ide-checkpoints/…` (I6).
    Checkpoint,
    /// A seal of a tested tree, `refs/ci-seals/…`.
    Seal,
    /// Notes, `refs/notes/…`.
    Note,
    /// The stash, `refs/stash`.
    Stash,
    /// Anything else, such as `refs/worktree/…` or a tool's own refs.
    Other,
}

impl RefKind {
    /// The kind of the ref called `name`.
    pub fn of(name: &str) -> RefKind {
        let prefixes = [
            ("refs/heads/", RefKind::Branch),
            ("refs/remotes/", RefKind::Remote),
            ("refs/tags/", RefKind::Tag),
            ("refs/ide-checkpoints/", RefKind::Checkpoint),
            ("refs/ci-seals/", RefKind::Seal),
            ("refs/notes/", RefKind::Note),
        ];
        if name == "HEAD" {
            return RefKind::Head;
        }
        if name == "refs/stash" {
            return RefKind::Stash;
        }
        prefixes
            .iter()
            .find(|(prefix, _)| name.starts_with(prefix))
            .map_or(RefKind::Other, |(_, kind)| *kind)
    }
}

/// Every ref of a repository with what it pointed at: `refs/…` by object name, `HEAD` as `ref: <branch>` while it is
/// on one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefSnapshot {
    refs: BTreeMap<String, String>,
}

/// A ref that differs between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefChange {
    /// Its full name, such as `refs/heads/main`.
    pub name: String,
    /// What it is.
    pub kind: RefKind,
    /// What it pointed at in the earlier snapshot; `None` when it was added.
    pub before: Option<String>,
    /// What it points at in the later snapshot; `None` when it was removed.
    pub after: Option<String>,
}

/// What differs between two snapshots, each list sorted by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefChanges {
    /// Refs only the later snapshot has.
    pub added: Vec<RefChange>,
    /// Refs only the earlier snapshot has.
    pub removed: Vec<RefChange>,
    /// Refs both have, pointing at different objects.
    pub changed: Vec<RefChange>,
}

impl RefChanges {
    /// Whether nothing differs.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Every difference, added, removed and changed refs one after the other.
    pub fn all(&self) -> impl Iterator<Item = &RefChange> {
        self.added.iter().chain(&self.removed).chain(&self.changed)
    }
}

impl RefSnapshot {
    /// What `name` points at; `None` when the snapshot has no such ref.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.refs.get(name).map(String::as_str)
    }

    /// The refs with what each points at, sorted by name.
    pub fn refs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.refs.iter().map(|(name, target)| (name.as_str(), target.as_str()))
    }

    /// What differs from this snapshot to the `later` one.
    pub fn changes_to(&self, later: &RefSnapshot) -> RefChanges {
        let change = |name: &str, before: Option<&String>, after: Option<&String>| RefChange {
            name: name.to_string(),
            kind: RefKind::of(name),
            before: before.cloned(),
            after: after.cloned(),
        };
        let mut changes = RefChanges::default();
        for (name, before) in &self.refs {
            match later.refs.get(name) {
                None => changes.removed.push(change(name, Some(before), None)),
                Some(after) if after != before => changes.changed.push(change(name, Some(before), Some(after))),
                Some(_) => {}
            }
        }
        for (name, after) in &later.refs {
            if !self.refs.contains_key(name) {
                changes.added.push(change(name, None, Some(after)));
            }
        }
        changes
    }
}

impl Repository {
    /// Every ref of the repository now — branches, remote-tracking branches, tags, checkpoints, seals, notes, the
    /// stash and any other under `refs/` — and this working tree's `HEAD`. Taken in the user's main working tree it is
    /// what the ref watch compares (E173).
    pub fn ref_snapshot(&self) -> Result<RefSnapshot> {
        let output = self
            .git()
            .ok(self.call(["for-each-ref", "--format=%(refname)%00%(objectname)"]))?;
        let mut refs: BTreeMap<String, String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.split_once('\0'))
            .map(|(name, object)| (name.to_string(), object.to_string()))
            .collect();
        let symbolic = self.git().run(self.call(["symbolic-ref", "--quiet", "HEAD"]))?;
        if symbolic.code == 0 {
            refs.insert("HEAD".into(), format!("ref: {}", symbolic.text()));
        } else if let Some(head) = self.head()? {
            refs.insert("HEAD".into(), head.to_string());
        }
        Ok(RefSnapshot { refs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(refs: &[(&str, &str)]) -> RefSnapshot {
        RefSnapshot {
            refs: refs.iter().map(|(n, o)| (n.to_string(), o.to_string())).collect(),
        }
    }

    #[test]
    fn refs_are_told_apart_by_name() {
        assert_eq!(RefKind::of("refs/heads/order/csv"), RefKind::Branch);
        assert_eq!(RefKind::of("refs/ide-checkpoints/o-1.builder/3"), RefKind::Checkpoint);
        assert_eq!(RefKind::of("refs/ci-seals/abc"), RefKind::Seal);
        assert_eq!(RefKind::of("refs/notes/commits"), RefKind::Note);
        assert_eq!(RefKind::of("refs/stash"), RefKind::Stash);
        assert_eq!(RefKind::of("HEAD"), RefKind::Head);
        assert_eq!(RefKind::of("refs/worktree/x"), RefKind::Other);
        assert_eq!(RefKind::of("refs/headsx"), RefKind::Other);
    }

    #[test]
    fn a_comparison_lists_added_removed_and_changed_refs() {
        let before = snapshot(&[("refs/heads/main", "a"), ("refs/stash", "s"), ("refs/tags/v1", "t")]);
        let after = snapshot(&[
            ("refs/heads/main", "b"),
            ("refs/tags/v1", "t"),
            ("refs/ci-seals/x", "c"),
        ]);
        let changes = before.changes_to(&after);
        let names = |list: &[RefChange]| list.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&changes.added), ["refs/ci-seals/x"]);
        assert_eq!(names(&changes.removed), ["refs/stash"]);
        assert_eq!(names(&changes.changed), ["refs/heads/main"]);
        assert_eq!(changes.changed[0].before.as_deref(), Some("a"));
        assert_eq!(changes.changed[0].after.as_deref(), Some("b"));
        assert_eq!(changes.added[0].kind, RefKind::Seal);
        assert_eq!(changes.all().count(), 3);
        assert!(after.changes_to(&after).is_empty());
    }
}
