//! A project folder without a window: its tree read one folder at a time, changes on disk, quick open, project search
//! and replace, the `.editorconfig` settings of its files and whether the user trusts it.
//!
//! The watcher and quick open run threads of their own and answer through callbacks; everything else blocks and
//! belongs on a background thread. Long work takes a [`Cancel`] and has a limit.
//!
//! [`search_panel`] and [`files_panel`] are what the app's search and files panels decide without AppKit.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod editorconfig;
pub mod files_panel;
pub mod fuzzy;
pub mod index;
pub mod order;
pub mod prettier;
pub mod replace;
pub mod search;
pub mod search_panel;
pub mod site;
pub mod tree;
pub mod trust;
pub mod watch;

mod rules;
mod text;

pub use index::{FileIndex, Hit, Indexed, QuickOpen};
pub use replace::{Replacement, replacements};
pub use search::{FileResult, LineResult, Preview, SearchError, SearchOptions, Summary, search, search_with};
pub use tree::{Entry, Kind, Tree, TreeOptions};
pub use trust::TrustStore;
pub use watch::{Changes, FileChange, RescanReason, WatchEvent, Watcher};

pub use ide_core::Cancel;

#[cfg(test)]
pub(crate) mod testing {
    use std::path::Path;

    pub(crate) use ide_core::testing::scratch;

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
