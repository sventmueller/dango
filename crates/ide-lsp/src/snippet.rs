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

/// The text and tab stops of `snippet`; what is not valid snippet syntax is taken literally.
pub(crate) fn parse(snippet: &str) -> Snippet {
    let mut parser = Parser {
        chars: snippet.chars().collect(),
        at: 0,
        text: String::new(),
        len: 0,
        stops: BTreeMap::new(),
    };
    parser.any(None);
    let end = parser.len;
    let mut stops = parser.stops;
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

struct Parser {
    chars: Vec<char>,
    at: usize,
    text: String,
    /// The text's length in UTF-16 code units.
    len: u32,
    stops: BTreeMap<u32, Tabstop>,
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
            if Some(c) == close {
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
                    let (at, text_len, len) = (self.at, self.text.len(), self.len);
                    let stops = self.stops.clone();
                    if !self.element() {
                        self.at = at;
                        self.text.truncate(text_len);
                        self.len = len;
                        self.stops = stops;
                        self.push('$');
                    }
                }
                _ => self.push(c),
            }
        }
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
                self.any(Some('}'));
                if self.peek() != Some('}') {
                    return false;
                }
                self.at += 1;
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
                self.any(Some('}'));
                if self.peek() != Some('}') {
                    return false;
                }
                self.at += 1;
                true
            }
            Some('/') => {
                let mut escaped = false;
                while let Some(c) = self.peek() {
                    self.at += 1;
                    match c {
                        '\\' if !escaped => escaped = true,
                        '}' if !escaped => return true,
                        _ => escaped = false,
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// The choices of `${n|a,b|}` up to and including `|}`.
    fn choices(&mut self) -> Option<Vec<String>> {
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
        let stop = self.stops.entry(index).or_insert_with(|| Tabstop {
            index,
            spans: vec![],
            choices: vec![],
        });
        stop.spans.push(span);
        if stop.choices.is_empty() {
            stop.choices = choices;
        }
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
}
