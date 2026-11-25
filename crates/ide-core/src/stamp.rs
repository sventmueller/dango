//! What tells whether a file changed since it was last read, without reading it: one `stat` instead of a read and a
//! parse, for files the main thread asks about many times a second, such as the settings and the trusted folders.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::sync::LockIgnoringPoison;

/// A file's identity, size and times of change; a file replaced by a rename is a new file, so it has a new identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Stamp {
    inode: u64,
    len: u64,
    modified: Option<SystemTime>,
    /// When the file's own data last changed, such as its permissions, which do not touch its time of change.
    changed: (i64, i64),
}

impl Stamp {
    /// The stamp of the file at `path`; `None` if there is none or it cannot be asked.
    pub(crate) fn of(path: &Path) -> Option<Stamp> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Stamp {
            inode: std::os::unix::fs::MetadataExt::ino(&metadata),
            len: metadata.len(),
            modified: metadata.modified().ok(),
            changed: (
                std::os::unix::fs::MetadataExt::ctime(&metadata),
                std::os::unix::fs::MetadataExt::ctime_nsec(&metadata),
            ),
        })
    }
}

/// What was worked out from files, by file and by key, kept while the file keeps its stamp: the answer to a question
/// about a file the main thread asks many times a second costs one `stat`. Without a file nothing is kept.
pub struct ByStamp<K, V> {
    files: Mutex<Option<Files<K, V>>>,
}

/// Each file's stamp when its values were worked out, and the values by key.
type Files<K, V> = HashMap<PathBuf, (Stamp, HashMap<K, V>)>;

impl<K: Eq + Hash, V: Clone> ByStamp<K, V> {
    /// Nothing kept yet; usable in a `static`.
    pub const fn new() -> ByStamp<K, V> {
        ByStamp {
            files: Mutex::new(None),
        }
    }

    /// What `work` makes of `file` for `key`, from what was kept if the file did not change since.
    pub fn get(&self, file: &Path, key: K, work: impl FnOnce() -> V) -> V {
        let Some(stamp) = Stamp::of(file) else {
            return work();
        };
        if let Some((seen, values)) = self.files.lock_ignoring_poison().get_or_insert_default().get(file)
            && *seen == stamp
            && let Some(value) = values.get(&key)
        {
            return value.clone();
        }
        let value = work();
        // A file that changed while it was read keeps no entry, so the next question reads it again.
        if Stamp::of(file) == Some(stamp) {
            let mut files = self.files.lock_ignoring_poison();
            let (seen, values) = files
                .get_or_insert_default()
                .entry(file.to_path_buf())
                .or_insert_with(|| (stamp, HashMap::new()));
            if *seen != stamp {
                *seen = stamp;
                values.clear();
            }
            values.insert(key, value.clone());
        }
        value
    }
}

impl<K: Eq + Hash, V: Clone> Default for ByStamp<K, V> {
    fn default() -> ByStamp<K, V> {
        ByStamp::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_is_kept_until_the_file_changes_and_never_kept_without_a_file() {
        let dir = crate::testing::scratch("by-stamp");
        let path = dir.join("trust.json");
        let kept: ByStamp<&str, usize> = ByStamp::new();
        let mut runs = 0;
        let mut ask = |key| {
            kept.get(&path, key, || {
                runs += 1;
                runs
            })
        };
        assert_eq!(ask("a"), 1);
        assert_eq!(ask("a"), 2, "no file, nothing kept");
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(ask("a"), 3);
        assert_eq!(ask("a"), 3);
        assert_eq!(ask("b"), 4);
        let other = dir.join("new.json");
        std::fs::write(&other, "{}").unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert_eq!(ask("a"), 5);
        assert_eq!(ask("b"), 6);
    }

    #[test]
    fn a_file_replaced_by_a_rename_has_a_new_stamp_even_with_the_same_size() {
        let dir = crate::testing::scratch("stamp");
        let path = dir.join("a.json");
        std::fs::write(&path, "{}").unwrap();
        let first = Stamp::of(&path).unwrap();
        assert_eq!(Stamp::of(&path), Some(first));
        let other = dir.join("b.json");
        std::fs::write(&other, "[]").unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert_ne!(Stamp::of(&path), Some(first));
        assert_eq!(Stamp::of(&dir.join("missing")), None);
    }
}
