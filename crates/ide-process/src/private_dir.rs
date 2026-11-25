//! Fresh folders for the files a program started outside the agent's sandbox reads or writes, such as git's trace or
//! a scratch index: a random name no one can lay a link at beforehand, and only the user may enter it.

use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

/// How many random names are tried before giving up, each taken by someone else.
const ATTEMPTS: usize = 16;

/// A folder of one call's own, removed with what is in it when dropped.
#[derive(Debug)]
pub struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    /// A new folder named after `purpose`, such as `ide-git-trace`, in the user's cache folder below `/var/folders`,
    /// or in the temporary folder when that is out of reach.
    pub fn new(purpose: &str) -> io::Result<PrivateDir> {
        // `$TMPDIR` is among the folders the own agent's sandbox writes in, the cache folder beside it is not (B101).
        if let Some(cache) = user_cache()
            && let Ok(dir) = PrivateDir::new_in(&cache, purpose)
        {
            return Ok(dir);
        }
        PrivateDir::new_in(&std::env::temp_dir(), purpose)
    }

    /// A new folder named after `purpose` in `parent`, which is never made; a name that is taken is never reused.
    pub fn new_in(parent: &Path, purpose: &str) -> io::Result<PrivateDir> {
        PrivateDir::named(parent, purpose, random_name)
    }

    fn named(parent: &Path, purpose: &str, mut name: impl FnMut() -> String) -> io::Result<PrivateDir> {
        for _ in 0..ATTEMPTS {
            let path = parent.join(format!("{purpose}-{}", name()));
            match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(PrivateDir { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("every name tried for {purpose} in {} was taken", parent.display()),
        ))
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// 16 random lowercase hexadecimal digits.
fn random_name() -> String {
    let mut bytes = [0u8; 8];
    // SAFETY: arc4random_buf fills exactly `bytes.len()` bytes of the buffer it is handed, which lives until it returns.
    unsafe { libc::arc4random_buf(bytes.as_mut_ptr().cast(), bytes.len()) };
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The user's cache folder below `/var/folders`.
fn user_cache() -> Option<PathBuf> {
    let mut buffer = [0u8; 1024];
    // SAFETY: confstr writes at most `buffer.len()` bytes, the terminating NUL included, into the buffer it is handed,
    // which lives until the call returned.
    let length = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_CACHE_DIR,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    if length == 0 || length > buffer.len() {
        return None;
    }
    let cache = PathBuf::from(OsStr::from_bytes(&buffer[..length - 1]));
    cache.is_dir().then_some(cache)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn each_call_gets_a_new_folder_only_the_user_may_enter_which_goes_when_dropped() {
        let first = PrivateDir::new("ide-process-test").unwrap();
        let second = PrivateDir::new("ide-process-test").unwrap();
        assert_ne!(first.path(), second.path());
        for dir in [&first, &second] {
            assert!(dir.path().is_dir());
            assert_eq!(mode(dir.path()), 0o700, "{}", dir.path().display());
        }
        std::fs::write(first.path().join("trace.json"), "{}").unwrap();
        let path = first.path().to_path_buf();
        drop(first);
        assert!(!path.exists());
    }

    #[test]
    fn a_name_that_is_taken_is_never_reused() {
        let scratch = ide_core::testing::scratch("private-dir-taken");
        let parent = scratch.join("parent");
        let elsewhere = scratch.join("elsewhere");
        std::fs::create_dir(&parent).unwrap();
        std::fs::create_dir(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, parent.join("taken-link")).unwrap();
        std::fs::create_dir(parent.join("taken-folder")).unwrap();
        let mut names = ["link", "folder", "fresh"].into_iter().map(str::to_string);
        let dir = PrivateDir::named(&parent, "taken", || names.next().unwrap()).unwrap();
        assert_eq!(dir.path(), parent.join("taken-fresh"));
        assert_eq!(mode(dir.path()), 0o700);
        assert!(parent.join("taken-link").is_symlink());
        assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
        let always_taken = PrivateDir::named(&parent, "taken", || "link".to_string());
        assert_eq!(always_taken.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert!(PrivateDir::new_in(&scratch.join("missing"), "taken").is_err());
    }
}
