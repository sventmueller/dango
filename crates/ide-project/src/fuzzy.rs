//! Fuzzy matching of paths: the characters of each word of the query in order, scored after fzf's algorithm, with a
//! preference for matches in the file name.

use std::ops::Range;

use crate::text;

const MATCH: i32 = 16;
const GAP_START: i32 = -3;
const GAP_EXTENSION: i32 = -1;
const AFTER_WHITESPACE: i32 = 10;
const AFTER_SLASH: i32 = 9;
const AFTER_PUNCTUATION: i32 = 8;
const PUNCTUATION: i32 = 8;
const CAMEL_CASE: i32 = 7;
/// Makes a run of matches worth at least as much as the gap it closes.
const CONSECUTIVE: i32 = -(GAP_START + GAP_EXTENSION);
const FIRST_CHARACTER_FACTOR: i32 = 2;
const IN_FILE_NAME: i32 = 3;
/// Only the end of a longer path is scored, which keeps the work per path bounded.
const MAX_CHARS: usize = 1024;
const NONE: i32 = i32::MIN / 4;

/// A query prepared for matching many paths.
#[derive(Debug, Clone)]
pub(crate) struct Pattern {
    terms: Vec<Term>,
}

#[derive(Debug, Clone)]
struct Term {
    chars: Vec<char>,
    /// Smart case: a word with a capital letter matches case exactly.
    exact_case: bool,
    ascii: bool,
}

/// How well a path matches and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Match {
    /// Higher is better; only comparable between matches of the same pattern.
    pub(crate) score: i32,
    /// The matched characters as ranges of UTF-16 code units, for highlighting.
    pub(crate) ranges: Vec<Range<usize>>,
}

/// Reusable buffers, so matching many paths does not allocate per path.
#[derive(Debug, Default)]
pub(crate) struct Scratch {
    chars: Vec<char>,
    folded: Vec<char>,
    bonus: Vec<i32>,
    score: Vec<i32>,
    run: Vec<i32>,
    from: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Whitespace,
    Slash,
    Punctuation,
    Lower,
    Upper,
    Digit,
    Letter,
}

impl Pattern {
    /// The pattern of `query`, whose words separated by spaces must all match; `None` if it has no words.
    pub(crate) fn new(query: &str) -> Option<Pattern> {
        let terms: Vec<Term> = query
            .split_whitespace()
            .map(|word| {
                let exact_case = word.chars().any(char::is_uppercase);
                let chars: Vec<char> = if exact_case {
                    word.chars().collect()
                } else {
                    word.chars().map(fold).collect()
                };
                Term {
                    ascii: chars.iter().all(char::is_ascii),
                    chars,
                    exact_case,
                }
            })
            .collect();
        (!terms.is_empty()).then_some(Pattern { terms })
    }

    /// How well `text` matches, with the positions of the matched characters.
    pub(crate) fn find(&self, text: &str) -> Option<Match> {
        let mut scratch = Scratch::default();
        let (score, mut positions) = self.run(text, &mut scratch, true)?;
        positions.sort_unstable();
        positions.dedup();
        Some(Match {
            score,
            ranges: text::char_ranges(text, &positions),
        })
    }

    /// How well `text` matches, without the positions.
    pub(crate) fn score(&self, text: &str, scratch: &mut Scratch) -> Option<i32> {
        self.run(text, scratch, false).map(|(score, _)| score)
    }

    fn run(&self, text: &str, scratch: &mut Scratch, positions: bool) -> Option<(i32, Vec<usize>)> {
        if !self.terms.iter().all(|term| term.could_match(text)) {
            return None;
        }
        let skipped = scratch.prepare(text);
        let mut total = 0;
        let mut found = Vec::new();
        for term in &self.terms {
            let (score, at) = scratch.align(term, positions)?;
            total += score;
            found.extend(at.into_iter().map(|p| p + skipped));
        }
        Some((total, found))
    }
}

impl Term {
    /// Whether the characters of the term occur in order, checked before any scoring.
    fn could_match(&self, text: &str) -> bool {
        if self.ascii && !self.exact_case {
            let mut wanted = self.chars.iter().map(|c| *c as u8).peekable();
            for byte in text.bytes() {
                match wanted.peek() {
                    Some(w) if *w == byte.to_ascii_lowercase() => {
                        wanted.next();
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            return wanted.peek().is_none();
        }
        let mut wanted = self.chars.iter().peekable();
        for c in text.chars() {
            let c = if self.exact_case { c } else { fold(c) };
            if wanted.peek().is_some_and(|w| **w == c) {
                wanted.next();
            }
        }
        wanted.peek().is_none()
    }
}

impl Scratch {
    /// Reads the characters of `text` and their bonuses; how many leading characters were left out.
    fn prepare(&mut self, text: &str) -> usize {
        self.chars.clear();
        self.chars.extend(text.chars());
        let skipped = self.chars.len().saturating_sub(MAX_CHARS);
        self.chars.drain(..skipped);
        self.folded.clear();
        self.folded.extend(self.chars.iter().map(|c| fold(*c)));
        let name_start = self.chars.iter().rposition(|c| *c == '/').map_or(0, |slash| slash + 1);
        self.bonus.clear();
        let mut previous = Class::Slash;
        for (index, c) in self.chars.iter().enumerate() {
            let class = class(*c);
            let in_name = if index >= name_start { IN_FILE_NAME } else { 0 };
            self.bonus.push(bonus(previous, class) + in_name);
            previous = class;
        }
        skipped
    }

    /// The best placement of `term` in the prepared text, as in fzf's second algorithm.
    fn align(&mut self, term: &Term, want_positions: bool) -> Option<(i32, Vec<usize>)> {
        let text = if term.exact_case { &self.chars } else { &self.folded };
        let (n, m) = (text.len(), term.chars.len());
        let first = text.iter().position(|c| *c == term.chars[0])?;
        let last = text.iter().rposition(|c| *c == term.chars[m - 1])?;
        if last < first {
            return None;
        }
        self.score.clear();
        self.score.resize(n * m, NONE);
        self.run.clear();
        self.run.resize(n * m, 0);
        self.from.clear();
        self.from.resize(n * m, u32::MAX);
        for (i, wanted) in term.chars.iter().enumerate() {
            let row = i * n;
            let mut gap = NONE;
            let mut gap_from = 0;
            for (j, c) in text.iter().enumerate().take(last + 1).skip(first) {
                if i > 0 {
                    if gap > NONE {
                        gap += GAP_EXTENSION;
                    }
                    if j >= 2 && self.score[row - n + j - 2] > NONE {
                        let opened = self.score[row - n + j - 2] + GAP_START;
                        if opened > gap {
                            gap = opened;
                            gap_from = j - 2;
                        }
                    }
                }
                if c != wanted {
                    continue;
                }
                let bonus = self.bonus[j];
                if i == 0 {
                    self.score[row + j] = MATCH + bonus * FIRST_CHARACTER_FACTOR;
                    self.run[row + j] = bonus;
                    continue;
                }
                let mut best = NONE;
                if j >= 1 && self.score[row - n + j - 1] > NONE {
                    let run = self.run[row - n + j - 1];
                    best = self.score[row - n + j - 1] + MATCH + run.max(bonus).max(CONSECUTIVE);
                    self.run[row + j] = run.max(bonus);
                    self.from[row + j] = u32::try_from(j - 1).ok()?;
                }
                if gap > NONE && gap + MATCH + bonus > best {
                    best = gap + MATCH + bonus;
                    self.run[row + j] = bonus;
                    self.from[row + j] = u32::try_from(gap_from).ok()?;
                }
                self.score[row + j] = best;
            }
        }
        let last_row = (m - 1) * n;
        let (end, score) = (first..=last)
            .map(|j| (j, self.score[last_row + j]))
            .filter(|(_, score)| *score > NONE)
            .max_by_key(|(j, score)| (*score, std::cmp::Reverse(*j)))?;
        let mut positions = Vec::new();
        if want_positions {
            let mut j = end;
            for i in (0..m).rev() {
                positions.push(j);
                j = self.from[i * n + j] as usize;
            }
        }
        Some((score, positions))
    }
}

fn fold(c: char) -> char {
    if c.is_ascii() {
        c.to_ascii_lowercase()
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

fn class(c: char) -> Class {
    if c == '/' {
        Class::Slash
    } else if c.is_whitespace() {
        Class::Whitespace
    } else if c.is_lowercase() {
        Class::Lower
    } else if c.is_uppercase() {
        Class::Upper
    } else if c.is_numeric() {
        Class::Digit
    } else if c.is_alphabetic() {
        Class::Letter
    } else {
        Class::Punctuation
    }
}

fn bonus(previous: Class, current: Class) -> i32 {
    match current {
        Class::Whitespace => AFTER_WHITESPACE,
        Class::Slash | Class::Punctuation => PUNCTUATION,
        _ => match previous {
            Class::Whitespace => AFTER_WHITESPACE,
            Class::Slash => AFTER_SLASH,
            Class::Punctuation => AFTER_PUNCTUATION,
            Class::Lower if current == Class::Upper => CAMEL_CASE,
            Class::Lower | Class::Upper | Class::Letter if current == Class::Digit => CAMEL_CASE,
            _ => 0,
        },
    }
}

#[cfg(test)]
mod property_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::one;

    fn rank<'a>(query: &str, paths: &[&'a str]) -> Vec<&'a str> {
        let pattern = Pattern::new(query).unwrap();
        let mut scored: Vec<(i32, &str)> = paths
            .iter()
            .filter_map(|path| pattern.find(path).map(|m| (m.score, *path)))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.len().cmp(&b.1.len())));
        scored.into_iter().map(|(_, path)| path).collect()
    }

    #[test]
    fn a_match_in_the_file_name_beats_one_spread_over_folders() {
        let paths = ["library/mod.rs", "src/lib.rs", "lab/internal/b.rs"];
        assert_eq!(
            rank("lib", &paths),
            ["src/lib.rs", "library/mod.rs", "lab/internal/b.rs"]
        );
    }

    #[test]
    fn word_starts_and_camel_case_humps_count_more_than_letters_inside_words() {
        let paths = ["src/fileopener.rs", "src/FileOpener.rs", "src/file_opener.rs"];
        let ranked = rank("fo", &paths);
        assert_eq!(ranked.last(), Some(&"src/fileopener.rs"));
        assert_eq!(
            rank("ide core", &["crates/ide-core/src/lib.rs", "docs/ideas/core.md"]).len(),
            2
        );
    }

    #[test]
    fn every_word_of_the_query_must_match() {
        assert!(
            Pattern::new("core lib")
                .unwrap()
                .find("crates/ide-core/src/lib.rs")
                .is_some()
        );
        assert!(
            Pattern::new("core zzz")
                .unwrap()
                .find("crates/ide-core/src/lib.rs")
                .is_none()
        );
        assert!(Pattern::new("   ").is_none());
    }

    #[test]
    fn a_capital_letter_makes_the_word_match_case_exactly() {
        assert!(Pattern::new("readme").unwrap().find("README.md").is_some());
        assert!(Pattern::new("Readme").unwrap().find("README.md").is_none());
        assert!(Pattern::new("README").unwrap().find("README.md").is_some());
    }

    #[test]
    fn matched_characters_come_back_as_utf16_ranges() {
        let found = Pattern::new("main").unwrap().find("src/main.rs").unwrap();
        assert_eq!(found.ranges, one(4..8));
        let found = Pattern::new("😀b").unwrap().find("a/😀b.txt").unwrap();
        assert_eq!(found.ranges, one(2..5));
        let found = Pattern::new("über").unwrap().find("docs/Über.md").unwrap();
        assert_eq!(found.ranges, one(5..9));
    }

    #[test]
    fn the_best_placement_wins_over_the_first_one() {
        let found = Pattern::new("abc").unwrap().find("a/b/c/abc.txt").unwrap();
        assert_eq!(found.ranges, one(6..9));
    }

    #[test]
    fn very_long_paths_are_scored_on_their_end() {
        let long = format!("{}/target.rs", "x".repeat(5_000));
        let found = Pattern::new("target").unwrap().find(&long).unwrap();
        assert_eq!(found.ranges, one(5_001..5_007));
        assert!(Pattern::new("q").unwrap().find(&format!("q{long}")).is_none());
    }
}
