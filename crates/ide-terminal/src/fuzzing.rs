//! Entry points for the fuzz targets in `fuzz/`.

use crate::scan::Scanner;

/// Feeds `bytes` to the scanner for shell marks, working directories and servers, in pieces of the size the first
/// byte picks, so that sequences split across reads are tried too.
pub fn scan(bytes: &[u8]) {
    let Some((&first, rest)) = bytes.split_first() else {
        return;
    };
    let mut scanner = Scanner::new();
    for piece in rest.chunks(usize::from(first % 32) + 1) {
        scanner.feed(piece);
        let _ = scanner.take_events();
    }
}

/// Reads `bytes` as a `KERN_PROCARGS2` buffer.
pub fn procargs(bytes: &[u8]) {
    let _ = crate::foreground::parse_procargs(bytes);
}
