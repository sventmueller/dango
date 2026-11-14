//! What tells whether a file changed since it was last read, without reading it: one `stat` instead of a read and a
//! parse, for files the main thread asks about many times a second, such as the settings and the trusted folders.

use std::path::Path;
use std::time::SystemTime;

/// A file's identity, size and times of change; a file replaced by a rename is a new file, so it has a new identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stamp {
    inode: u64,
    len: u64,
    modified: Option<SystemTime>,
    /// When the file's own data last changed, such as its permissions, which do not touch its time of change.
    changed: (i64, i64),
}

impl Stamp {
    /// The stamp of the file at `path`; `None` if there is none or it cannot be asked.
    pub fn of(path: &Path) -> Option<Stamp> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
