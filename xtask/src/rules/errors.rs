//! How errors reach the user in the app's AppKit code: as typed errors that explain themselves
//! (`ide_core::explain::Explain`), never as a bare `String` whose language and origin nobody can tell, and never in
//! their `Debug` form.

use super::{arguments, code_only, line_of, occurrences};

/// Calls whose arguments the user reads, or that carry an error on to where the user reads it.
const SHOWN: [&str; 12] = [
    "setInformativeText(",
    "setMessageText(",
    "setStringValue(",
    "setToolTip(",
    "alert(",
    "notify(",
    "set_status(",
    "show_note(",
    "Problem::plain(",
    "Problem::Start(",
    "Failed(",
    "map_err(",
];

/// What breaks the rules on errors in the production file at `relative` with the content `text`.
pub(super) fn problems(relative: &str, text: &str) -> Vec<String> {
    let text = production(text);
    let code = code_only(text);
    let mut problems = string_errors(relative, &code);
    problems.extend(debug_shown(relative, text, &code));
    problems
}

/// `text` without its unit tests, which may compare errors as they like.
fn production(text: &str) -> &str {
    text.find("#[cfg(test)]\nmod tests").map_or(text, |end| &text[..end])
}

/// The `Result`s whose error is a bare `String`.
fn string_errors(relative: &str, code: &str) -> Vec<String> {
    occurrences(code, "Result<")
        .into_iter()
        .filter(|&at| last_argument(&code[at + "Result<".len()..]) == Some("String"))
        .map(|at| {
            format!(
                "{relative}:{}: an error as a bare String; give it a type that implements Explain, or Explained for \
                 one already in the user's words",
                line_of(code, at)
            )
        })
        .collect()
}

/// The last of two or more generic arguments of a type whose arguments start at `rest`, trimmed; `None` for one,
/// as in `io::Result<String>`, or if they never close.
fn last_argument(rest: &str) -> Option<&str> {
    let (mut depth, mut start) = (1, None);
    let mut previous = ' ';
    for (offset, c) in rest.char_indices() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' if previous == '-' => {}
            '>' | ')' | ']' => {
                depth -= 1;
                if depth == 0 {
                    return start.map(|start| rest[start..offset].trim());
                }
            }
            ',' if depth == 1 => start = Some(offset + 1),
            _ => {}
        }
        previous = c;
    }
    None
}

/// The calls of `SHOWN` that format a value with `{:?}`, which shows the user a type's insides.
fn debug_shown(relative: &str, text: &str, code: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for (number, (line, code_line)) in text.lines().zip(code.lines()).enumerate() {
        for name in SHOWN {
            if !code_line.contains(name) {
                continue;
            }
            let Some(at) = line.find(name) else { continue };
            let start = text_offset(text, number) + at + name.len();
            if arguments(text, start).contains(":?}") {
                problems.push(format!(
                    "{relative}:{}: `{name}..)` formats with {{:?}}; say it in the user's words with Explain",
                    number + 1
                ));
            }
        }
    }
    problems
}

/// The byte offset of the line `number`, from 0, of `text`.
fn text_offset(text: &str, number: usize) -> usize {
    text.split_inclusive('\n').take(number).map(str::len).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_string_error_is_found_and_typed_ones_are_not() {
        let source = "fn a() -> Result<(), String> {}\nfn b() -> Result<Vec<String>, Explained> {}\n\
                      fn c(f: impl FnOnce(Result<u8, String>)) {}\nfn d() -> io::Result<String> {}\n\
                      fn e(f: impl Fn() -> Result<u8, Error>) -> Result<u8, String> {}\n";
        let found = problems("src/a.rs", source);
        let lines: Vec<&str> = found.iter().map(|p| p.split(": ").next().unwrap()).collect();
        assert_eq!(lines, ["src/a.rs:1", "src/a.rs:3", "src/a.rs:5"], "{found:?}");
    }

    #[test]
    fn a_debug_form_shown_to_the_user_is_found() {
        let source = "alert(mtm, t(\"Fehler\"), &format!(\"{error:?}\"));\n\
                      alert.setInformativeText(&ns(&format!(\n    \"{:?}\",\n    error\n)));\n\
                      .map_err(|e| format!(\"{e:?}\"))?;\nlet shown = format!(\"{e:?}\");\n\
                      // alert(\"{e:?}\")\n";
        let found = problems("src/a.rs", source);
        let lines: Vec<&str> = found.iter().map(|p| p.split(": ").next().unwrap()).collect();
        assert_eq!(lines, ["src/a.rs:1", "src/a.rs:2", "src/a.rs:6"], "{found:?}");
    }

    #[test]
    fn unit_tests_may_compare_errors_as_they_like() {
        let source = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn b() -> Result<(), String> { Ok(()) }\n}\n";
        assert!(problems("src/a.rs", source).is_empty());
    }
}
