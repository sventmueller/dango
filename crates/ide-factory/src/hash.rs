//! SHA-256 as lowercase hex, the form fingerprints and confirmations are kept in.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The SHA-256 of `bytes` in lowercase hex.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-256 of `value`'s JSON; struct fields serialise in declaration order and maps here are ordered, so equal
/// values hash equal.
pub(crate) fn hash_json(value: &impl Serialize) -> String {
    let json = serde_json::to_vec(value).unwrap_or_default();
    sha256_hex(&json)
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_of_nothing() {
        assert_eq!(
            super::sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
