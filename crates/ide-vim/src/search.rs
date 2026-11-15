//! Vim's own search and Ex prompt (E106, F11): the editor's find bar gives no matches or patterns, so `/`, `?`, `n`,
//! `N`, `*`, `:s` and `:%s` search here, with Rust's regular expressions. Where they differ from Vim's patterns, such
//! as `\v` or `\<`, is in the help (R24).

use modalkit::key::TerminalKey;
use regex::Regex;

use crate::Note;

/// The prompt being typed and the last search, for `n` and `N`.
#[derive(Debug, Default)]
pub(crate) struct Search {
    /// The prompt's first character and what follows it, while it is open.
    open: Option<(char, String)>,
    /// The last pattern and whether it searched forward.
    last: Option<(String, bool)>,
}

/// What a key in the prompt did.
pub(crate) enum Step {
    /// The prompt goes on.
    Typing,
    /// The search found a match at this line and column; the pattern is to be marked.
    Jump(usize, usize, String),
    /// An Ex command was entered.
    Command(String),
    /// Nothing was found, or the pattern is not valid.
    Failed(Note),
}

/// Whether `key` opens the prompt in Normal mode.
pub(crate) fn opens(key: &TerminalKey) -> bool {
    matches!(key.to_string().as_str(), "/" | "?" | ":")
}

/// Whether `key` repeats or starts a search without a prompt.
pub(crate) fn repeats(key: &TerminalKey) -> bool {
    matches!(key.to_string().as_str(), "n" | "N" | "*")
}

impl Search {
    pub(crate) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// What the prompt shows, such as `/foo`, while it is open.
    pub(crate) fn shown(&self) -> Option<String> {
        self.open.as_ref().map(|(kind, typed)| format!("{kind}{typed}"))
    }

    /// Takes `key` while the prompt is open, or opens it, in `text` with the cursor at `cursor`.
    pub(crate) fn key(&mut self, key: &TerminalKey, text: &str, cursor: (usize, usize)) -> Step {
        let name = key.to_string();
        let Some((kind, typed)) = self.open.as_mut() else {
            let kind = name.chars().next().unwrap_or('/');
            self.open = Some((kind, String::new()));
            return Step::Typing;
        };
        match name.as_str() {
            "<Esc>" | "<C-C>" | "<C-[>" => {
                self.open = None;
                Step::Typing
            }
            "<BS>" => {
                if typed.pop().is_none() {
                    self.open = None;
                }
                Step::Typing
            }
            "<Enter>" | "<CR>" => {
                let (kind, typed) = (*kind, std::mem::take(typed));
                self.open = None;
                if kind == ':' {
                    return Step::Command(typed);
                }
                let forward = kind == '/';
                let pattern = if typed.is_empty() {
                    match &self.last {
                        Some((pattern, _)) => pattern.clone(),
                        None => return Step::Failed(Note::NoPreviousPattern),
                    }
                } else {
                    typed
                };
                self.last = Some((pattern.clone(), forward));
                find(&pattern, text, cursor, forward)
            }
            one if one.chars().count() == 1 => {
                typed.push_str(one);
                Step::Typing
            }
            "<Space>" => {
                typed.push(' ');
                Step::Typing
            }
            _ => Step::Typing,
        }
    }

    /// `n`, `N` or `*` in `text` with the cursor at `cursor`: where the next match is.
    pub(crate) fn again(&mut self, key: &TerminalKey, text: &str, cursor: (usize, usize)) -> Option<(usize, usize)> {
        let name = key.to_string();
        if name == "*" {
            let word = word_at(text, cursor)?;
            let pattern = format!(r"\b{}\b", regex::escape(&word));
            self.last = Some((pattern, true));
        }
        let (pattern, forward) = self.last.clone()?;
        let forward = if name == "N" { !forward } else { forward };
        match find(&pattern, text, cursor, forward) {
            Step::Jump(line, column, _) => Some((line, column)),
            _ => None,
        }
    }
}

/// The next match of `pattern` in `text` after `cursor`, or before it backwards, wrapping around the end.
fn find(pattern: &str, text: &str, cursor: (usize, usize), forward: bool) -> Step {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => return Step::Failed(invalid(pattern, &error)),
    };
    let at = byte_of(text, cursor);
    let starts: Vec<usize> = regex.find_iter(text).map(|m| m.start()).collect();
    let found = if forward {
        starts.iter().find(|s| **s > at).or_else(|| starts.first())
    } else {
        starts.iter().rev().find(|s| **s < at).or_else(|| starts.last())
    };
    match found {
        Some(start) => {
            let (line, column) = place_of(text, *start);
            Step::Jump(line, column, pattern.to_string())
        }
        None => Step::Failed(Note::PatternNotFound(pattern.to_string())),
    }
}

/// `:s/old/new/[g]` on the cursor's line `line` or `:%s/…` on every line of `text`: the text after it, or why not;
/// `None` for any other command.
pub(crate) fn substitute(command: &str, text: &str, line: usize) -> Option<Result<String, Note>> {
    let (whole, rest) = match command.strip_prefix('%') {
        Some(rest) => (true, rest),
        None => (false, command),
    };
    let rest = rest.strip_prefix('s')?;
    let separator = rest.chars().next().filter(|c| !c.is_alphanumeric())?;
    let parts: Vec<&str> = rest[separator.len_utf8()..].split(separator).collect();
    let (old, new) = (parts.first()?, parts.get(1).copied().unwrap_or_default());
    let global = parts.get(2).is_some_and(|flags| flags.contains('g'));
    let regex = match Regex::new(old) {
        Ok(regex) => regex,
        Err(error) => return Some(Err(invalid(old, &error))),
    };
    let replacement = new.replace('&', "$0");
    let lines: Vec<String> = text
        .split_inclusive('\n')
        .enumerate()
        .map(|(n, content)| {
            if !(whole || n == line) {
                return content.to_string();
            }
            if global {
                regex.replace_all(content, replacement.as_str()).into_owned()
            } else {
                regex.replace(content, replacement.as_str()).into_owned()
            }
        })
        .collect();
    Some(Ok(lines.concat()))
}

fn invalid(pattern: &str, error: &regex::Error) -> Note {
    Note::InvalidPattern {
        pattern: pattern.to_string(),
        detail: error.to_string(),
    }
}

fn byte_of(text: &str, (line, column): (usize, usize)) -> usize {
    let start: usize = text.split_inclusive('\n').take(line).map(str::len).sum();
    let rest = &text[start.min(text.len())..];
    start + rest.chars().take(column).map(char::len_utf8).sum::<usize>()
}

fn place_of(text: &str, byte: usize) -> (usize, usize) {
    let before = &text[..byte];
    let line = before.matches('\n').count();
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count());
    (line, column)
}

fn word_at(text: &str, cursor: (usize, usize)) -> Option<String> {
    let line = text.lines().nth(cursor.0)?;
    let chars: Vec<char> = line.chars().collect();
    let is_word = |c: &char| c.is_alphanumeric() || *c == '_';
    let at = cursor.1.min(chars.len().saturating_sub(1));
    if !chars.get(at).is_some_and(is_word) {
        return None;
    }
    let start = chars[..at].iter().rev().take_while(|c| is_word(c)).count();
    let end = chars[at..].iter().take_while(|c| is_word(c)).count();
    Some(chars[at - start..at + end].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_search_keeps_why_and_says_it_as_vim_does() {
        let invalid = substitute("s/(/x/", "a\n", 0);
        let Some(Err(Note::InvalidPattern { pattern, detail })) = &invalid else {
            panic!("{invalid:?}")
        };
        assert_eq!(pattern, "(");
        assert!(!detail.is_empty());
        assert!(matches!(
            find("zz", "a\n", (0, 0), true),
            Step::Failed(Note::PatternNotFound(pattern)) if pattern == "zz"
        ));
        assert!(matches!(
            find("(", "a\n", (0, 0), true),
            Step::Failed(Note::InvalidPattern { .. })
        ));
        assert_eq!(substitute("s/a/b/", "a\n", 0).map(Result::ok), Some(Some("b\n".into())));
    }
}
