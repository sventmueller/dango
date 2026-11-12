//! Entry points for the fuzz targets in `fuzz/`.

/// Reads `bytes` as the output of `git diff`.
pub fn diff(bytes: &[u8]) {
    let _ = crate::diff::parse(bytes);
}
