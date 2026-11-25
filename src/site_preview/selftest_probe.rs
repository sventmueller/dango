//! The self-test's hooks of `site_preview.rs`.

use super::*;

/// The running servers' folders, for the self-test.
pub(crate) fn running(mtm: MainThreadMarker) -> Vec<(PathBuf, String, String)> {
    AppContext::get(mtm)
        .site_previews()
        .sites
        .borrow()
        .iter()
        .map(|site| (site.root().to_path_buf(), site.origin(), site.secret().to_string()))
        .collect()
}
