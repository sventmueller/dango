//! The self-test's hooks of `settings_sync.rs`.

use super::*;

/// The folder and this Mac's id, for the self-test.
pub fn where_and_who() -> (PathBuf, String) {
    (folder(), device_id())
}
