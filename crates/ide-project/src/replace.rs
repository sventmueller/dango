//! What replacing the matches of a project search changes in one text: the same matches the search finds, line by
//! line, each with its replacement.

use std::ops::Range;

use grep_matcher::{Captures, Matcher};
use grep_regex::RegexMatcher;

use crate::search::{SearchError, SearchOptions, build_matcher};
use crate::text;

/// One match in a text and what replaces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// The match, in UTF-16 code units from the start of the text.
    pub range_utf16: Range<usize>,
    /// Its line, counted from 1.
    pub line: u64,
    /// Where it starts and ends in its line, in UTF-16 code units, as a search result gives its columns.
    pub columns: Range<usize>,
    /// What replaces it.
    pub text: String,
}

/// The matches of `options` in `text` with what `replacement` turns each into. With `options.regex`, `$1` and
/// `${name}` in `replacement` stand for capture groups and `$$` for a dollar sign; otherwise it is taken as it is.
pub fn replacements(text: &str, options: &SearchOptions, replacement: &str) -> Result<Vec<Replacement>, SearchError> {
    Replacer::new(options)?.replacements(text, replacement)
}

/// The matcher of one search's options, built once for replacing in many texts.
pub struct Replacer {
    matcher: RegexMatcher,
    regex: bool,
}

impl Replacer {
    /// The replacer of `options`, or why its pattern is no valid one.
    pub fn new(options: &SearchOptions) -> Result<Replacer, SearchError> {
        Ok(Replacer {
            matcher: build_matcher(options)?,
            regex: options.regex,
        })
    }

    /// [`replacements`] in `text` with the options this replacer was built from.
    pub fn replacements(&self, text: &str, replacement: &str) -> Result<Vec<Replacement>, SearchError> {
        let matcher = &self.matcher;
        let mut captures = matcher
            .new_captures()
            .map_err(|e| SearchError::Pattern(e.to_string()))?;
        let mut found = Vec::new();
        let mut line_start_utf16 = 0;
        for (number, raw) in (1u64..).zip(text.split_inclusive('\n')) {
            let line = raw.strip_suffix('\n').unwrap_or(raw);
            let line = line.strip_suffix('\r').unwrap_or(line);
            let bytes = line.as_bytes();
            let mut spans: Vec<(Range<usize>, String)> = Vec::new();
            // The matcher of grep-regex cannot fail.
            let _ = matcher.captures_iter(bytes, &mut captures, |caps| {
                let Some(whole) = caps.get(0).filter(|m| !m.is_empty()) else {
                    return true;
                };
                let with = if self.regex {
                    let mut out = Vec::new();
                    caps.interpolate(
                        |name| matcher.capture_index(name),
                        bytes,
                        replacement.as_bytes(),
                        &mut out,
                    );
                    String::from_utf8_lossy(&out).into_owned()
                } else {
                    replacement.to_string()
                };
                spans.push((whole.start()..whole.end(), with));
                true
            });
            let offsets: Vec<usize> = spans.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
            let columns = text::columns(bytes, &offsets);
            for ((_, with), pair) in spans.into_iter().zip(columns.chunks(2)) {
                found.push(Replacement {
                    range_utf16: line_start_utf16 + pair[0]..line_start_utf16 + pair[1],
                    line: number,
                    columns: pair[0]..pair[1],
                    text: with,
                });
            }
            line_start_utf16 += raw.encode_utf16().count();
        }
        Ok(found)
    }
}

/// `text` with `replacements` made, which must not overlap.
#[cfg(test)]
pub(crate) fn apply(text: &str, replacements: &[Replacement]) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut sorted: Vec<&Replacement> = replacements.iter().collect();
    sorted.sort_by_key(|r| r.range_utf16.start);
    let mut out: Vec<u16> = Vec::with_capacity(units.len());
    let mut at = 0;
    for replacement in sorted {
        let range = replacement.range_utf16.start.min(units.len())..replacement.range_utf16.end.min(units.len());
        if range.start < at {
            continue;
        }
        out.extend_from_slice(&units[at..range.start]);
        out.extend(replacement.text.encode_utf16());
        at = range.end;
    }
    out.extend_from_slice(&units[at..]);
    String::from_utf16_lossy(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(pattern: &str) -> SearchOptions {
        SearchOptions::new(pattern)
    }

    #[test]
    fn literal_matches_take_the_replacement_as_it_is() {
        let text = "Needle and needle\nno match\r\nneedle $1\n";
        let found = replacements(text, &options("needle"), "pin $1").unwrap();
        assert_eq!(found.len(), 3, "case is ignored by default, as in the search");
        assert_eq!(found[0].range_utf16, 0..6);
        assert_eq!((found[1].line, found[1].columns.clone()), (1, 11..17));
        assert_eq!(found[2].line, 3);
        assert!(found.iter().all(|r| r.text == "pin $1"));
        assert_eq!(apply(text, &found), "pin $1 and pin $1\nno match\r\npin $1 $1\n");
    }

    #[test]
    fn regular_expressions_fill_in_their_groups() {
        let mut regex = options(r"fn (\w+)\((?P<arg>\w*)\)");
        regex.regex = true;
        regex.case_sensitive = true;
        let text = "fn main() {}\nfn run(x) {}\n";
        let found = replacements(text, &regex, "fn ${1}_new(${arg}) $$").unwrap();
        assert_eq!(apply(text, &found), "fn main_new() $ {}\nfn run_new(x) $ {}\n");
    }

    #[test]
    fn columns_count_utf16_as_the_search_does() {
        let text = "äöü 😀 needle\nab needle\n";
        let found = replacements(text, &options("needle"), "x").unwrap();
        assert_eq!(found[0].columns, 7..13);
        assert_eq!(found[0].range_utf16, 7..13);
        assert_eq!(found[1].range_utf16, 14 + 3..14 + 9);
        assert_eq!(apply(text, &found), "äöü 😀 x\nab x\n");
    }

    #[test]
    fn whole_words_and_case_follow_the_options() {
        let mut word = options("needle");
        word.whole_word = true;
        word.case_sensitive = true;
        let found = replacements("needles needle Needle", &word, "pin").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].range_utf16, 8..14);
    }

    #[test]
    fn a_pattern_that_is_no_regular_expression_is_an_error() {
        let mut broken = options("fn (");
        broken.regex = true;
        assert!(matches!(
            replacements("fn (", &broken, ""),
            Err(SearchError::Pattern(_))
        ));
    }
}
