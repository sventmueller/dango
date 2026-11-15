//! The folders the user trusts to run what they control, such as git hooks, language servers from the project and
//! agent hooks: a folder is trusted when it or a folder above it is.

use ide_core::LockIgnoringPoison;
use ide_core::stamp::ByStamp;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// The file in the data folder.
const FILE: &str = "trust.json";

/// Where a damaged file is moved before a new one replaces it.
const DAMAGED: &str = "trust.damaged.json";

const VERSION: u64 = 1;

/// Keeps two windows that trust folders at the same moment from losing one of the two.
static WRITING: Mutex<()> = Mutex::new(());

/// Whether each folder asked about was trusted, by trust file: the main thread asks on every caret rest and every
/// redraw of some views, and each answer otherwise reads the file and resolves every path in it on disk.
static ANSWERS: ByStamp<PathBuf, bool> = ByStamp::new();

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    version: u64,
    roots: Vec<PathBuf>,
}

/// The trusted folders, read from disk on every question so all windows agree.
#[derive(Debug, Clone)]
pub struct TrustStore {
    file: PathBuf,
}

enum Read {
    Missing,
    Found(Stored),
    Damaged,
}

impl TrustStore {
    /// The store in the app's data folder.
    pub fn open() -> TrustStore {
        TrustStore::in_dir(&ide_core::paths::data_dir())
    }

    /// The store in `dir`, for tests.
    pub(crate) fn in_dir(dir: &Path) -> TrustStore {
        TrustStore { file: dir.join(FILE) }
    }

    /// Whether `folder` or a folder above it is trusted; nothing is while the file cannot be read.
    pub fn is_trusted(&self, folder: &Path) -> bool {
        ANSWERS.get(&self.file, folder.to_path_buf(), || self.ask(folder))
    }

    fn ask(&self, folder: &Path) -> bool {
        let roots = self.roots();
        if roots.is_empty() {
            return false;
        }
        let folder = canonical(folder);
        roots.iter().any(|root| folder.starts_with(canonical(root)))
    }

    /// Trusts `folder` and everything below it.
    pub fn trust(&self, folder: &Path) -> io::Result<()> {
        let folder = canonical(folder);
        folder
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path is not UTF-8"))?;
        self.change(|roots| {
            if roots.iter().any(|root| canonical(root) == folder) {
                return false;
            }
            roots.push(folder.clone());
            roots.sort();
            true
        })
    }

    /// Stops trusting `folder` itself; whether it was trusted. A folder above it may still be.
    pub fn revoke(&self, folder: &Path) -> io::Result<bool> {
        let folder = canonical(folder);
        let mut removed = false;
        self.change(|roots| {
            let before = roots.len();
            roots.retain(|root| canonical(root) != folder);
            removed = roots.len() != before;
            removed
        })?;
        Ok(removed)
    }

    /// The folders trusted with everything below them, in the order of their paths.
    #[cfg(test)]
    pub(crate) fn list(&self) -> Vec<PathBuf> {
        self.roots()
    }

    fn roots(&self) -> Vec<PathBuf> {
        match self.read() {
            Read::Found(stored) => stored.roots,
            Read::Missing | Read::Damaged => Vec::new(),
        }
    }

    fn read(&self) -> Read {
        match std::fs::read(&self.file) {
            Err(_) => Read::Missing,
            Ok(bytes) => serde_json::from_slice(&bytes).map_or(Read::Damaged, Read::Found),
        }
    }

    /// Applies `edit` to the stored folders and writes them if it reports a change.
    fn change(&self, edit: impl FnOnce(&mut Vec<PathBuf>) -> bool) -> io::Result<()> {
        let _writing = WRITING.lock_ignoring_poison();
        let mut stored = match self.read() {
            Read::Found(stored) if stored.version > VERSION => {
                return Err(io::Error::other(format!(
                    "{} was written by a newer version and stays as it is",
                    self.file.display()
                )));
            }
            Read::Found(stored) => stored,
            Read::Missing => Stored::default(),
            Read::Damaged => {
                // Trust is only ever granted by the file, so a damaged one is kept for inspection and replaced.
                std::fs::rename(&self.file, self.file.with_file_name(DAMAGED))?;
                Stored::default()
            }
        };
        if !edit(&mut stored.roots) {
            return Ok(());
        }
        stored.version = VERSION;
        ide_core::atomic_write::write_json(&self.file, &stored)
    }
}

/// `path` absolute, with links resolved and each name in the case it has on disk, as far as the path exists.
fn canonical(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            return missing.iter().rev().fold(real, |path, name| path.join(name));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_os_string());
                existing = parent;
            }
            _ => return absolute,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::testing::scratch;

    fn setup(name: &str) -> (TrustStore, PathBuf, ide_core::testing::Scratch) {
        let base = scratch(name);
        let projects = base.join("Projects");
        std::fs::create_dir_all(projects.join("app/src")).unwrap();
        std::fs::create_dir_all(projects.join("other")).unwrap();
        (TrustStore::in_dir(&base.join("data")), projects, base)
    }

    #[test]
    fn a_trusted_folder_trusts_everything_below_it_and_nothing_beside_it() {
        let (store, projects, _base) = setup("trust-below");
        assert!(!store.is_trusted(&projects.join("app")));
        store.trust(&projects.join("app")).unwrap();
        assert!(store.is_trusted(&projects.join("app")));
        assert!(store.is_trusted(&projects.join("app/src")));
        assert!(store.is_trusted(&projects.join("app/src/not-yet-created")));
        assert!(!store.is_trusted(&projects.join("other")));
        assert!(!store.is_trusted(&projects.join("app-copy")));
        assert!(!store.is_trusted(&projects));
        assert_eq!(store.list(), [projects.join("app")]);
    }

    #[test]
    fn links_and_other_spellings_of_a_trusted_folder_are_trusted() {
        let (store, projects, _base) = setup("trust-spelling");
        store.trust(&projects.join("app")).unwrap();
        symlink(projects.join("app"), projects.join("alias")).unwrap();
        assert!(store.is_trusted(&projects.join("alias/src")));
        assert!(store.is_trusted(&projects.join("other/../app/./src")));
        let shouted = projects.parent().unwrap().join("PROJECTS/APP");
        if shouted.exists() {
            assert!(
                store.is_trusted(&shouted),
                "the volume ignores case, so the folder is the same"
            );
        }
        store.trust(&projects.join("alias")).unwrap();
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn revoking_a_folder_ends_its_trust_but_not_that_of_a_folder_above() {
        let (store, projects, _base) = setup("trust-revoke");
        store.trust(&projects).unwrap();
        store.trust(&projects.join("app")).unwrap();
        assert!(store.revoke(&projects.join("app")).unwrap());
        assert!(!store.revoke(&projects.join("app")).unwrap());
        assert!(store.is_trusted(&projects.join("app")));
        assert!(store.revoke(&projects).unwrap());
        assert!(!store.is_trusted(&projects.join("app")));
        assert!(store.list().is_empty());
    }

    #[test]
    fn a_damaged_file_trusts_nothing_and_is_set_aside_on_the_next_change() {
        let (store, projects, _base) = setup("trust-damaged");
        std::fs::create_dir_all(store.file.parent().unwrap()).unwrap();
        std::fs::write(&store.file, "{ not json").unwrap();
        assert!(!store.is_trusted(&projects));
        store.trust(&projects.join("app")).unwrap();
        assert!(store.is_trusted(&projects.join("app")));
        let aside = store.file.with_file_name(DAMAGED);
        assert_eq!(std::fs::read_to_string(aside).unwrap(), "{ not json");
    }

    #[test]
    fn a_file_from_a_newer_version_is_read_but_never_overwritten() {
        let (store, projects, _base) = setup("trust-newer");
        std::fs::create_dir_all(store.file.parent().unwrap()).unwrap();
        let original = format!(
            r#"{{"version": 9, "roots": [{:?}], "future": true}}"#,
            projects.join("app")
        );
        std::fs::write(&store.file, &original).unwrap();
        assert!(store.is_trusted(&projects.join("app/src")));
        assert!(store.trust(&projects.join("other")).is_err());
        assert_eq!(std::fs::read_to_string(&store.file).unwrap(), original);
    }
}
