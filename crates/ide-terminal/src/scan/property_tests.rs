//! Output read in pieces finds what it finds read whole, checked over generated output (E96).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::Scanner;

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x5ca7_7e57),
        ..Config::default()
    }
}

/// Output from what the scanner looks for, what it skips and what cuts a sequence short.
fn output() -> impl Strategy<Value = Vec<u8>> {
    let piece = prop_oneof![
        Just(&b"\x1b]133;A\x07"[..]),
        Just(&b"\x1b]133;C\x1b\\"[..]),
        Just(&b"\x1b]133;D;"[..]),
        Just(&b"\x1b]7;file://host/a%20b"[..]),
        Just(&b"\x1b]7;kitty-shell-cwd://h/x"[..]),
        Just(&b"\x07"[..]),
        Just(&b"\x1b\\"[..]),
        Just(&b"\x1b[32m"[..]),
        Just(&b"\x1bP1$r"[..]),
        Just(&b"\x18"[..]),
        Just(&b"Local: http://localhost:"[..]),
        Just(&b"5173/"[..]),
        Just(&b"listening on port 8080"[..]),
        Just(&b"\r\n"[..]),
        Just(&b"\n"[..]),
        Just(&b"\r"[..]),
        Just(&b"text \xc3\xa4 "[..]),
        Just(&b"\x1b"[..]),
        Just(&b";"[..]),
        Just(&b"0"[..]),
    ];
    proptest::collection::vec(piece, 0..60).prop_map(|pieces| pieces.concat())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn output_read_in_pieces_finds_what_it_finds_read_whole(
        bytes in output(),
        cuts in proptest::collection::vec(any::<usize>(), 0..12),
    ) {
        let mut whole = Scanner::new();
        whole.feed(&bytes);
        let expected = whole.take_events();

        let mut at: Vec<usize> = cuts.iter().map(|cut| cut % (bytes.len() + 1)).collect();
        at.push(0);
        at.push(bytes.len());
        at.sort_unstable();
        let mut pieces = Scanner::new();
        let mut found = Vec::new();
        for pair in at.windows(2) {
            pieces.feed(&bytes[pair[0]..pair[1]]);
            found.extend(pieces.take_events());
        }
        prop_assert_eq!(found, expected);
    }
}
