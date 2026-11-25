//! Fuzzy matching checked against a plain subsequence search over generated paths and queries (E96): a path matches
//! exactly when every word of the query occurs in it in order, the highlighted characters spell a one-word query,
//! and scoring many paths with one set of buffers scores each as on its own.

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::{Pattern, Scratch};

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x00f2_2e15),
        ..Config::default()
    }
}

const PIECES: &[&str] = &["a", "b", "B", "c", "/", "_", "-", ".", " ", "ä", "Ä", "1", "😀"];

const LETTERS: &[&str] = &["a", "b", "B", "c", "_", ".", "ä", "Ä", "1", "😀"];

fn path() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(PIECES), 0..40).prop_map(|pieces| pieces.concat())
}

fn word() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::sample::select(LETTERS), 1..5).prop_map(|pieces| pieces.concat())
}

/// The characters of `text` as a word of the query compares them: as they are when the word has a capital letter,
/// in lower case otherwise.
fn compared(text: &str, exact: bool) -> Vec<char> {
    text.chars()
        .map(|c| if exact { c } else { c.to_lowercase().next().unwrap_or(c) })
        .collect()
}

fn occurs_in_order(word: &str, path: &str) -> bool {
    let exact = word.chars().any(char::is_uppercase);
    let mut wanted = compared(word, exact).into_iter().peekable();
    for c in compared(path, exact) {
        if wanted.peek() == Some(&c) {
            wanted.next();
        }
    }
    wanted.peek().is_none()
}

/// The characters `ranges`, counted in UTF-16, cover in `text`.
fn highlighted(text: &str, ranges: &[std::ops::Range<usize>]) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    ranges
        .iter()
        .map(|range| String::from_utf16_lossy(&units[range.clone()]))
        .collect()
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn a_path_matches_when_every_word_occurs_in_it_in_order(
        words in proptest::collection::vec(word(), 1..4),
        paths in proptest::collection::vec(path(), 1..8),
    ) {
        let query = words.join(" ");
        let pattern = Pattern::new(&query).expect("a query with words");
        let mut scratch = Scratch::default();
        for path in &paths {
            let found = pattern.find(path);
            let expected = words.iter().all(|word| occurs_in_order(word, path));
            prop_assert_eq!(found.is_some(), expected, "{:?} in {:?}", query, path);
            prop_assert_eq!(pattern.score(path, &mut scratch), found.as_ref().map(|m| m.score));
            if let (Some(found), [word]) = (&found, words.as_slice()) {
                let exact = word.chars().any(char::is_uppercase);
                let shown = highlighted(path, &found.ranges);
                prop_assert_eq!(compared(&shown, exact), compared(word, exact), "{:?} in {:?}", word, path);
            }
        }
    }
}
