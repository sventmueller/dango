//! The self-test's hooks of `project.rs`.

use super::*;

/// Takes the trust in `folder` back, so a self-test can open an AI tab without its agent starting.
pub(crate) fn distrust(folder: &Path) {
    if let Err(error) = TrustStore::open().revoke(folder) {
        crate::diag::report("taking back the trust in a folder", error);
    }
}
