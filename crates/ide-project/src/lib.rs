//! A project folder without a window: its tree read one folder at a time, changes on disk, quick open, project search
//! and replace, the `.editorconfig` settings of its files and whether the user trusts it.
//!
//! The watcher and quick open run threads of their own and answer through callbacks; everything else blocks and
//! belongs on a background thread. Long work takes a [`Cancel`] and has a limit.

#![forbid(unsafe_code)]
#![warn(missing_docs, unreachable_pub)]

pub mod editorconfig;
pub mod fuzzy;
pub mod index;
pub mod order;
pub mod replace;
pub mod search;
pub mod tree;
pub mod trust;
pub mod watch;

mod rules;
mod text;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use index::{FileIndex, Hit, Indexed, QuickOpen};
pub use replace::{Replacement, replacements};
pub use search::{FileResult, LineResult, Preview, SearchError, SearchOptions, Summary, search, search_with};
pub use tree::{Entry, Kind, Link, Tree, TreeOptions};
pub use trust::TrustStore;
pub use watch::{Changes, FileChange, RescanReason, WatchEvent, Watcher};

/// A flag that asks long work to stop; clones share it.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// A token nobody has cancelled yet.
    pub fn new() -> Cancel {
        Cancel::default()
    }

    /// Asks every holder of this token to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether `cancel` was called on this token or a clone of it.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::path::{Path, PathBuf};

    /// An empty folder of its own for one test, with symlinks resolved so it compares equal to what the crate reports.
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ide-project-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    /// Writes `content` to `root/relative`, creating the folders on the way.
    pub(crate) fn write(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// A list of the one range `range`.
    pub(crate) fn one(range: std::ops::Range<usize>) -> Vec<std::ops::Range<usize>> {
        vec![range]
    }

    /// Lets a bound on disk work hold in unoptimised test builds, where the walk runs about twice as slow.
    pub(crate) fn walk_factor() -> u32 {
        if cfg!(debug_assertions) { 3 } else { 1 }
    }

    /// Lets a bound on computation hold in unoptimised test builds, where the matcher runs about fifteen times slower.
    pub(crate) fn compute_factor() -> u32 {
        if cfg!(debug_assertions) { 20 } else { 1 }
    }

    #[test]
    fn the_long_lived_parts_can_move_to_and_be_shared_between_background_threads() {
        fn send_and_sync<T: Send + Sync>() {}
        send_and_sync::<crate::Tree>();
        send_and_sync::<crate::FileIndex>();
        send_and_sync::<crate::QuickOpen>();
        send_and_sync::<crate::Watcher>();
        send_and_sync::<crate::TrustStore>();
        send_and_sync::<crate::Cancel>();
    }
}
