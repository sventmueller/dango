//! `file:` URIs for paths and back, in one spelling so that a server's URI finds the document it names.

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use lsp_types::Uri;

/// The `file:` URI of an absolute path.
pub fn from_path(path: &Path) -> Uri {
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    Uri::from_str(&uri).unwrap_or_else(|_| unreachable!("every byte outside the path characters is escaped"))
}

/// The path a `file:` URI names; `None` for other schemes and other hosts.
pub fn to_path(uri: &Uri) -> Option<PathBuf> {
    let rest = uri.as_str().strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let path = rest.split(['?', '#']).next().unwrap_or(rest);
    Some(PathBuf::from(OsString::from_vec(decode(path)?)))
}

/// `uri` spelt as `from_path` spells it, so `%40` and `@` name the same document.
pub fn normalize(uri: &Uri) -> Uri {
    to_path(uri).map_or_else(|| uri.clone(), |path| from_path(&path))
}

fn decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_survives_the_round_trip_whatever_it_contains() {
        for path in ["/Users/me/a b/Grüße.rs", "/tmp/[x]#?%.ts", "/a/@scope/+pkg/main.go"] {
            let uri = from_path(Path::new(path));
            assert!(uri.as_str().starts_with("file:///"));
            assert_eq!(to_path(&uri).unwrap(), Path::new(path));
        }
        assert_eq!(from_path(Path::new("/a b")).as_str(), "file:///a%20b");
    }

    #[test]
    fn spellings_of_the_same_path_normalize_to_one() {
        let a = Uri::from_str("file:///x/%40scope/a%2Eb").unwrap();
        let b = Uri::from_str("file://localhost/x/@scope/a.b").unwrap();
        assert_eq!(normalize(&a), normalize(&b));
        let other = Uri::from_str("untitled:Untitled-1").unwrap();
        assert_eq!(normalize(&other), other);
        assert_eq!(to_path(&Uri::from_str("file://server/share").unwrap()), None);
    }
}
