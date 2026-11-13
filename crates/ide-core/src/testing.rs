//! What the crates' tests share, behind the feature `testing`: a folder of its own for each test that goes away with
//! it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// An empty folder of its own for one test, removed when it is dropped. Its path has no symbolic links, so it
/// compares equal to what the system reports for files in it (`/private/var` for `/var`).
#[derive(Debug)]
pub struct Scratch(PathBuf);

impl Scratch {
    /// A new empty folder whose name starts with `name`; no other test of this process gets the same one.
    pub fn new(name: &str) -> Scratch {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ide-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Scratch(dir.canonicalize().unwrap_or(dir))
    }

    /// The folder's path.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The folder's path, leaving the folder in place, for what must outlive the test's own scope.
    pub fn keep(self) -> PathBuf {
        let path = self.0.clone();
        std::mem::forget(self);
        path
    }
}

/// A new empty folder for one test; see [`Scratch`].
pub fn scratch(name: &str) -> Scratch {
    Scratch::new(name)
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<std::ffi::OsStr> for Scratch {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_os_str()
    }
}

impl PartialEq<Scratch> for Path {
    fn eq(&self, scratch: &Scratch) -> bool {
        self == scratch.0
    }
}

impl PartialEq<Scratch> for &Path {
    fn eq(&self, scratch: &Scratch) -> bool {
        *self == scratch.0
    }
}

impl PartialEq<Scratch> for PathBuf {
    fn eq(&self, scratch: &Scratch) -> bool {
        *self == scratch.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_folder_is_its_own_and_goes_away_with_it() {
        let one = scratch("testing");
        let two = scratch("testing");
        assert_ne!(one.path(), two.path());
        std::fs::write(one.join("file"), "x").unwrap();
        let kept = one.to_path_buf();
        drop(one);
        assert!(!kept.exists());
        assert!(two.is_dir());
    }
}
