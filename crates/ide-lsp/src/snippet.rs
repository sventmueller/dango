//! Snippets in completion items, such as `fmt.Println(${1:a ...any})$0`, as the text to insert and its tab stops.

use std::collections::BTreeMap;

/// A stretch of the inserted text, in UTF-16 code units from its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Where it starts.
    pub start: u32,
    /// Where it ends, not included.
    pub end: u32,
}

impl Span {
    /// The stretch from `start` to `end`.
    pub(crate) fn new(start: u32, end: u32) -> Span {
        Span { start, end }
    }
}

/// A place the cursor visits after inserting, in order of `index`; the final cursor position has index 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tabstop {
    /// Its number; tab stops are visited 1, 2, … and 0 last.
    pub(crate) index: u32,
    /// Where it is; more than one span when the snippet repeats the number, and the edits mirror each other.
    pub spans: Vec<Span>,
    /// The values offered to choose from, if the snippet gives a choice.
    pub(crate) choices: Vec<String>,
}

/// A parsed snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// What to insert, without snippet syntax.
    pub text: String,
    /// In the order to visit; the last one is the final cursor position.
    pub tabstops: Vec<Tabstop>,
}

/// Snippets longer than this, in bytes, are inserted as plain text.
const MAX_LEN: usize = 64 * 1024;

/// Placeholders nested deeper than this make the whole snippet plain text.
const MAX_DEPTH: usize = 32;

/// The text and tab stops of `snippet`; what is not valid snippet syntax is taken literally, and a snippet longer than
/// [`MAX_LEN`] or nested deeper than [`MAX_DEPTH`] is taken literally as a whole.
pub(crate) fn parse(snippet: &str) -> Snippet {
    if snippet.len() > MAX_LEN {
        return literal(snippet);
    }
    let chars: Vec<char> = snippet.chars().collect();
    let mut parser = Parser {
        failed: vec![false; chars.len() + 1],
        chars,
        at: 0,
        text: String::new(),
        len: 0,
        stops: vec![],
        depth: 0,
        too_deep: false,
        choices_fail_from: usize::MAX,
        transform_fail_from: usize::MAX,
    };
    parser.any(None);
    if parser.too_deep {
        return literal(snippet);
    }
    let end = parser.len;
    let mut stops: BTreeMap<u32, Tabstop> = BTreeMap::new();
    for (index, span, choices) in parser.stops {
        let stop = stops.entry(index).or_insert_with(|| Tabstop {
            index,
            spans: vec![],
            choices: vec![],
        });
        stop.spans.push(span);
        if stop.choices.is_empty() {
            stop.choices = choices;
        }
    }
    let last = stops.remove(&0).unwrap_or(Tabstop {
        index: 0,
        spans: vec![Span::new(end, end)],
        choices: vec![],
    });
    let mut tabstops: Vec<Tabstop> = stops.into_values().collect();
    tabstops.push(last);
    Snippet {
        text: parser.text,
        tabstops,
    }
}

/// `text` as it is, with the cursor after it.
pub(crate) fn literal(text: &str) -> Snippet {
    let end = u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX);
    Snippet {
        text: text.to_string(),
        tabstops: vec![Tabstop {
            index: 0,
            spans: vec![Span::new(end, end)],
            choices: vec![],
        }],
    }
}

struct Parser {
    chars: Vec<char>,
    at: usize,
    text: String,
    /// The text's length in UTF-16 code units.
    len: u32,
    /// Every tab stop span in the order found, so that undoing a failed element is a truncation.
    stops: Vec<(u32, Span, Vec<String>)>,
    /// By position after a `$`: whether the element there failed, which it does again from any context.
    failed: Vec<bool>,
    depth: usize,
    too_deep: bool,
    /// The earliest choice list known to run unclosed to the end; every later one does too.
    choices_fail_from: usize,
    /// The earliest `${name/…` known to run unclosed to the end; every later one does too.
    transform_fail_from: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn push(&mut self, c: char) {
        self.text.push(c);
        self.len += u32::try_from(c.len_utf16()).unwrap_or(2);
    }

    /// Text and snippet elements up to `close`, which is left unread.
    fn any(&mut self, close: Option<char>) {
        while let Some(c) = self.peek() {
            if Some(c) == close || self.too_deep {
                return;
            }
            self.at += 1;
            match c {
                '\\' => match self.peek() {
                    Some(next @ ('$' | '}' | '\\')) => {
                        self.at += 1;
                        self.push(next);
                    }
                    _ => self.push('\\'),
                },
                '$' => {
                    let (at, text_len, len, stops) = (self.at, self.text.len(), self.len, self.stops.len());
                    if self.failed[at] || !self.element() {
                        self.failed[at] = true;
                        self.at = at;
                        self.text.truncate(text_len);
                        self.len = len;
                        self.stops.truncate(stops);
                        self.push('$');
                    }
                }
                _ => self.push(c),
            }
        }
    }

    /// The body of a placeholder or default and its closing `}`; false if it is not closed.
    fn body(&mut self) -> bool {
        if self.depth == MAX_DEPTH {
            self.too_deep = true;
            return false;
        }
        self.depth += 1;
        self.any(Some('}'));
        self.depth -= 1;
        if self.too_deep || self.peek() != Some('}') {
            return false;
        }
        self.at += 1;
        true
    }

    /// One element after a `$`; false if what follows is not one.
    fn element(&mut self) -> bool {
        if let Some(index) = self.number() {
            self.stop(index, Span::new(self.len, self.len), vec![]);
            return true;
        }
        if self.name().is_some() {
            return true;
        }
        if self.peek() != Some('{') {
            return false;
        }
        self.at += 1;
        if let Some(index) = self.number() {
            return self.braced_stop(index);
        }
        if self.name().is_some() {
            return self.variable();
        }
        false
    }

    /// The rest of `${n}`, `${n:placeholder}` or `${n|a,b|}`.
    fn braced_stop(&mut self, index: u32) -> bool {
        let start = self.len;
        match self.peek() {
            Some('}') => {
                self.at += 1;
                self.stop(index, Span::new(start, start), vec![]);
                true
            }
            Some(':') => {
                self.at += 1;
                if !self.body() {
                    return false;
                }
                self.stop(index, Span::new(start, self.len), vec![]);
                true
            }
            Some('|') => {
                self.at += 1;
                let Some(choices) = self.choices() else {
                    return false;
                };
                for c in choices[0].clone().chars() {
                    self.push(c);
                }
                self.stop(index, Span::new(start, self.len), choices);
                true
            }
            _ => false,
        }
    }

    /// The rest of `${name}`, `${name:default}` or `${name/regex/format/options}`. No variables are known, so the
    /// default or nothing is inserted, as the protocol leaves unknown variables to the client.
    fn variable(&mut self) -> bool {
        match self.peek() {
            Some('}') => {
                self.at += 1;
                true
            }
            Some(':') => {
                self.at += 1;
                self.body()
            }
            Some('/') => {
                let from = self.at;
                if from >= self.transform_fail_from {
                    return false;
                }
                let mut escaped = false;
                while let Some(c) = self.peek() {
                    self.at += 1;
                    match c {
                        '\\' if !escaped => escaped = true,
                        '}' if !escaped => return true,
                        _ => escaped = false,
                    }
                }
                self.transform_fail_from = from;
                false
            }
            _ => false,
        }
    }

    /// The choices of `${n|a,b|}` up to and including `|}`.
    fn choices(&mut self) -> Option<Vec<String>> {
        let from = self.at;
        if from >= self.choices_fail_from {
            return None;
        }
        let mut choices = vec![String::new()];
        while let Some(c) = self.peek() {
            self.at += 1;
            match c {
                '\\' => match self.peek() {
                    Some(next @ ('$' | '}' | '\\' | ',' | '|')) => {
                        self.at += 1;
                        choices.last_mut()?.push(next);
                    }
                    _ => choices.last_mut()?.push('\\'),
                },
                ',' => choices.push(String::new()),
                '|' if self.peek() == Some('}') => {
                    self.at += 1;
                    return Some(choices);
                }
                _ => choices.last_mut()?.push(c),
            }
        }
        self.choices_fail_from = from;
        None
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        let digits: String = self.chars[start..self.at].iter().collect();
        let number = digits.parse().ok();
        if number.is_none() {
            self.at = start;
        }
        number
    }

    fn name(&mut self) -> Option<String> {
        if !self.peek().is_some_and(|c| c == '_' || c.is_ascii_alphabetic()) {
            return None;
        }
        let start = self.at;
        while self.peek().is_some_and(|c| c == '_' || c.is_ascii_alphanumeric()) {
            self.at += 1;
        }
        Some(self.chars[start..self.at].iter().collect())
    }

    fn stop(&mut self, index: u32, span: Span, choices: Vec<String>) {
        self.stops.push((index, span, choices));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(snippet: &Snippet) -> Vec<(u32, Vec<Span>)> {
        snippet.tabstops.iter().map(|t| (t.index, t.spans.clone())).collect()
    }

    #[test]
    fn tabstops_and_placeholders_give_text_and_spans_in_visiting_order() {
        let s = parse("fmt.Println(${2:b}, ${1:a})$0");
        assert_eq!(s.text, "fmt.Println(b, a)");
        assert_eq!(
            spans(&s),
            [
                (1, vec![Span::new(15, 16)]),
                (2, vec![Span::new(12, 13)]),
                (0, vec![Span::new(17, 17)])
            ]
        );
    }

    #[test]
    fn without_a_final_stop_the_cursor_ends_after_the_text() {
        let s = parse("for ${1:i} := range $2 {}");
        assert_eq!(s.text, "for i := range  {}");
        assert_eq!(s.tabstops.last().unwrap().spans, [Span::new(18, 18)]);
    }

    #[test]
    fn nested_placeholders_choices_and_repeats_are_kept() {
        let s = parse("${1:outer ${2:inner}} ${3|one,two\\,x|} $1");
        assert_eq!(s.text, "outer inner one ");
        assert_eq!(s.tabstops[0].spans, [Span::new(0, 11), Span::new(16, 16)]);
        assert_eq!(s.tabstops[1].spans, [Span::new(6, 11)]);
        assert_eq!(s.tabstops[2].choices, ["one", "two,x"]);
    }

    #[test]
    fn spans_count_utf16_units() {
        let s = parse("😀${1:ä}");
        assert_eq!(s.tabstops[0].spans, [Span::new(2, 3)]);
    }

    #[test]
    fn escapes_variables_and_broken_syntax_are_taken_as_text() {
        assert_eq!(parse(r"\$1 \} \\ \x").text, r"$1 } \ \x");
        assert_eq!(
            parse("${TM_FILENAME:main.go} $TM_SELECTED_TEXT ${X/a/b/g}").text,
            "main.go  "
        );
        let broken = parse("price: $ and ${1:open");
        assert_eq!(broken.text, "price: $ and ${1:open");
        assert_eq!(broken.tabstops.len(), 1);
    }

    fn assert_literal(snippet: &str) {
        let parsed = parse(snippet);
        assert_eq!(parsed.text, snippet);
        assert_eq!(parsed.tabstops, literal(snippet).tabstops);
    }

    #[test]
    fn unclosed_elements_are_not_retried_from_every_dollar() {
        let started = std::time::Instant::now();
        assert_literal(&"${1:".repeat(10_000));
        assert_literal(&"${1|a,".repeat(10_000));
        assert_literal(&"${X/a/".repeat(10_000));
        let open = "${1:".repeat(MAX_DEPTH - 1);
        let s = parse(&format!("{open}{}", "a${2:b}".repeat(5_000)));
        assert_eq!(s.text, format!("{open}{}", "ab".repeat(5_000)));
        assert_eq!(s.tabstops[0].spans.len(), 5_000);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn nesting_up_to_the_limit_parses_and_deeper_is_taken_literally() {
        let nested = |depth: usize| format!("{}x{}", "${1:".repeat(depth), "}".repeat(depth));
        let s = parse(&nested(MAX_DEPTH));
        assert_eq!(s.text, "x");
        assert_eq!(s.tabstops[0].spans.len(), MAX_DEPTH);
        assert_literal(&nested(MAX_DEPTH + 1));
        assert_literal(&format!("${{X:{}}}", nested(MAX_DEPTH)));
        assert_literal(&nested(10_000));
        assert_literal(&nested(100_000));
    }

    #[test]
    fn snippets_over_the_length_limit_are_taken_literally() {
        assert_literal(&format!("${{1:a}}{}", "b".repeat(MAX_LEN)));
        assert_eq!(
            parse(&format!("${{1:a}}{}", "b".repeat(MAX_LEN - 6))).text.len(),
            MAX_LEN - 5
        );
    }
}
