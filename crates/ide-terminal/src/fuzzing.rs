//! Entry points for the fuzz targets in `fuzz/`.

use crate::scan::Scanner;

/// Feeds `bytes` to the scanner for shell marks, working directories and servers, in pieces of the size the first
/// byte picks, and checks that the pieces find exactly the events the whole output finds at once.
pub fn scan(bytes: &[u8]) {
    let Some((&first, rest)) = bytes.split_first() else {
        return;
    };
    let mut pieces = Scanner::new();
    let mut found = Vec::new();
    for piece in rest.chunks(usize::from(first % 32) + 1) {
        pieces.feed(piece);
        found.extend(pieces.take_events());
    }
    let mut whole = Scanner::new();
    whole.feed(rest);
    assert_eq!(found, whole.take_events(), "pieces of {}", usize::from(first % 32) + 1);
}

/// Reads `bytes` as a `KERN_PROCARGS2` buffer, and checks that arguments written into one read back as they were:
/// those the buffer held, and those the bytes make when split at NUL.
pub fn procargs(bytes: &[u8]) {
    if let Some(args) = crate::foreground::parse_procargs(bytes) {
        assert_eq!(crate::foreground::parse_procargs(&buffer(&args)), Some(args.clone()));
        let _ = crate::foreground::classify(&args);
    }
    let text = String::from_utf8_lossy(bytes);
    let args: Vec<String> = text.split('\0').map(str::to_string).collect();
    // The kernel's padding after the executable's path is NULs, so a first argument cannot be empty.
    if args.first().is_some_and(|first| !first.is_empty()) {
        assert_eq!(crate::foreground::parse_procargs(&buffer(&args)), Some(args));
    }
}

/// A `KERN_PROCARGS2` buffer as the kernel writes it for `args`, with the environment after them.
fn buffer(args: &[String]) -> Vec<u8> {
    let count = i32::try_from(args.len()).unwrap_or(i32::MAX);
    let mut out = count.to_ne_bytes().to_vec();
    out.extend_from_slice(b"/bin/program\0\0\0\0");
    for arg in args {
        out.extend_from_slice(arg.as_bytes());
        out.push(0);
    }
    out.extend_from_slice(b"HOME=/Users/me\0");
    out
}
