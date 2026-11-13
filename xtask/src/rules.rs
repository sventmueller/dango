//! The checks that keep the app's AppKit code to the rules CLAUDE.md names and the compiler cannot see: no state in
//! statics that bypass the app context unless docs/thread-locals.md lists them (E97), no leaked objects, system
//! callbacks inside `guarded`, sizes from `style.rs`, and no thread whose failure to start goes unnoticed.

use std::collections::BTreeSet;
use std::path::Path;

use crate::root;
use crate::thread_locals::rust_files;

/// The inventory that lists the statics allowed to keep state.
const INVENTORY: &str = "docs/thread-locals.md";

/// The heading in the inventory above the table of statics.
pub(crate) const STATICS_HEADING: &str = "## Statics";

/// The folders whose files every rule covers besides `crates/ide-ui-*`.
const APP: &str = "src";

/// The self-test's own harness, which is not app state and may block, leak and wait as a test does.
const SELFTEST: &str = "src/selftest";

/// The child module of a production file that holds its self-test hooks, built only with the self-test.
const SELFTEST_PROBE: &str = "/selftest_probe.rs";

/// The folders the rule about discarded threads covers.
const WORKSPACE: [&str; 3] = ["src", "crates", "xtask"];

/// The types whose statics hold state that changes.
const STATEFUL: [&str; 13] = [
    "Mutex", "RwLock", "OnceLock", "OnceCell", "LazyLock", "LazyCell", "Lazy<", "Atomic", "RefCell", "Cell<", "Once",
    "Lane", "Condvar",
];

/// How many times the app's code may look the app context up through `context::app()` or `try_app()` instead of being
/// handed it or a window id (E97); a ratchet: lower it whenever the count drops, it never rises.
const APP_LOOKUPS: usize = 73;

/// A static by the file it is in, relative to the workspace, and its name.
type Static = (String, String);

pub(crate) fn check() -> Result<(), String> {
    let root = root();
    let inventory = std::fs::read_to_string(root.join(INVENTORY)).map_err(|e| format!("{INVENTORY}: {e}"))?;
    let allowed = allowed(&inventory);
    let mut found = BTreeSet::new();
    let mut problems = Vec::new();
    let mut files = 0;
    let mut lookups = 0;
    for folder in WORKSPACE {
        for file in rust_files(&root.join(folder)) {
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let relative = relative(&root, &file);
            files += 1;
            problems.extend(discarded_threads(&relative, &text));
            if relative.starts_with(&format!("{APP}/")) || relative.starts_with("crates/ide-ui-") {
                lookups += app_lookups(&code_only(&text));
            }
            if in_app(&relative) {
                problems.extend(file_problems(&relative, &text));
                if !relative.ends_with(SELFTEST_PROBE) {
                    found.extend(statics(&text).into_iter().map(|name| (relative.clone(), name)));
                }
            }
        }
    }
    problems.extend(unlisted(&allowed, &found));
    problems.extend(lookup_problem(lookups));
    if problems.is_empty() {
        println!(
            "{files} Rust files: {} stateful statics, all listed in {INVENTORY}; callbacks guarded, no leaks, sizes \
             from style.rs, no thread that may fail unseen; {lookups} lookups of the app context",
            found.len()
        );
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

/// How often `code` calls `app()` or `try_app()`, the lookups of the app context, leaving out their definitions.
fn app_lookups(code: &str) -> usize {
    ["app()", "try_app()"]
        .into_iter()
        .flat_map(|name| occurrences(code, name).into_iter().map(move |at| (name, at)))
        .filter(|&(_, at)| !is_ident(code[..at].chars().next_back().as_ref()))
        .filter(|&(_, at)| !code[..at].trim_end().ends_with("fn"))
        .count()
}

/// The ratchet on the lookups of the app context: more than `APP_LOOKUPS` is a regression, fewer must lower it.
fn lookup_problem(lookups: usize) -> Option<String> {
    match lookups.cmp(&APP_LOOKUPS) {
        std::cmp::Ordering::Greater => Some(format!(
            "{lookups} lookups of the app context through app()/try_app(), at most {APP_LOOKUPS}: hand the context, \
             the window controller or a window id along instead (E97)"
        )),
        std::cmp::Ordering::Less => Some(format!(
            "{lookups} lookups of the app context through app()/try_app(), fewer than {APP_LOOKUPS}: lower \
             APP_LOOKUPS in xtask/src/rules.rs to {lookups}, so the count cannot rise again"
        )),
        std::cmp::Ordering::Equal => None,
    }
}

/// Whether the file at `relative` belongs to the app's AppKit code that the rules cover.
fn in_app(relative: &str) -> bool {
    let app = relative.starts_with(&format!("{APP}/")) && !relative.starts_with(&format!("{SELFTEST}/"));
    let ui_crate = relative.starts_with("crates/ide-ui-");
    (app || ui_crate) && relative != format!("{SELFTEST}.rs")
}

/// What breaks a rule in the app file at `relative` with the content `text`, but for its statics.
fn file_problems(relative: &str, text: &str) -> Vec<String> {
    let code = code_only(text);
    let mut problems = Vec::new();
    for at in occurrences(&code, "mem::forget(") {
        problems.push(format!(
            "{relative}:{}: mem::forget leaks; keep the value where its owner drops it",
            line_of(&code, at)
        ));
    }
    for name in ["RcBlock::new(", "StackBlock::new("] {
        for at in occurrences(&code, name) {
            let body = arguments(&code, at + name.len());
            let empty = body.trim_end().ends_with("{}") && body.matches('{').count() == 1;
            if !empty && !body.contains("guarded(") && !body.contains("contained(") {
                problems.push(format!(
                    "{relative}:{}: the body of a block the system calls runs outside `guarded(..)`",
                    line_of(&code, at)
                ));
            }
        }
    }
    if !relative.ends_with("/style.rs") {
        for name in [
            "FontOfSize(",
            "FontOfSize_weight(",
            "setSpacing(",
            "setCustomSpacing_afterView(",
        ] {
            for at in occurrences(&code, name) {
                let first = code[at + name.len()..].trim_start().chars().next();
                if first.is_some_and(|c| c.is_ascii_digit()) {
                    problems.push(format!(
                        "{relative}:{}: a literal size in `{name}..)`; take a token from ide_ui_common::style",
                        line_of(&code, at)
                    ));
                }
            }
        }
    }
    problems
}

/// The `let _ = ..spawn(..)` statements of the file at `relative`, whose thread may fail to start without anyone
/// hearing of it.
fn discarded_threads(relative: &str, text: &str) -> Vec<String> {
    let code = code_only(text);
    occurrences(&code, "let _ =")
        .into_iter()
        .filter(|&at| {
            let statement = &code[at..];
            let end = statement.find(';').unwrap_or(statement.len());
            statement[..end].contains(".spawn(")
        })
        .map(|at| {
            format!(
                "{relative}:{}: the result of spawning is discarded; hand a failure to the caller or the user",
                line_of(&code, at)
            )
        })
        .collect()
}

/// What is wrong between the statics `allowed` to keep state and those `found`.
fn unlisted(allowed: &BTreeSet<Static>, found: &BTreeSet<Static>) -> Vec<String> {
    let new = found.difference(allowed).map(|(file, name)| {
        format!(
            "{file}: static {name} keeps state outside the app context (src/context.rs) and is not listed under \
             \"{STATICS_HEADING}\" in {INVENTORY}"
        )
    });
    let gone = allowed
        .difference(found)
        .map(|(file, name)| format!("{INVENTORY}: {file} {name} is listed under \"{STATICS_HEADING}\" but is gone"));
    new.chain(gone).collect()
}

/// The statics of the rows below the statics heading of the inventory.
fn allowed(inventory: &str) -> BTreeSet<Static> {
    let Some(start) = inventory.find(STATICS_HEADING) else {
        return BTreeSet::new();
    };
    inventory[start..]
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim().strip_prefix('|')?.split('|').map(str::trim).collect();
            let [file, name, _holds, reason, ..] = cells.as_slice() else {
                return None;
            };
            let file = file.trim_matches('`');
            (file.ends_with(".rs") && !reason.is_empty())
                .then(|| (file.to_string(), name.trim_matches('`').to_string()))
        })
        .collect()
}

/// The names of the statics of `source` whose type keeps state that changes, outside `thread_local!` blocks (the
/// thread-locals check covers those) and outside code built only for tests and the self-test.
fn statics(source: &str) -> Vec<String> {
    let code = code_only(source);
    let original: Vec<&str> = source.lines().collect();
    let mut names = Vec::new();
    let mut depth = 0i32;
    let mut skip_from: Option<i32> = None;
    for (index, line) in code.lines().enumerate() {
        let trimmed = line.trim_start();
        let test_only =
            attributes_above(&original, index).any(|a| a.contains("cfg(test)") || a.contains("\"selftest\""));
        if skip_from.is_none() && (trimmed.starts_with("thread_local!") || (test_only && opens_module(trimmed))) {
            skip_from = Some(depth);
        }
        if skip_from.is_none()
            && !test_only
            && let Some(rest) = static_declaration(trimmed)
            && let Some((name, kind)) = rest.split_once(':')
        {
            let kind = kind.split('=').next().unwrap_or_default();
            if STATEFUL.iter().any(|s| kind.contains(s)) {
                names.push(name.trim().to_string());
            }
        }
        for c in line.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if skip_from.is_some_and(|from| depth <= from && line.contains('}')) {
            skip_from = None;
        }
    }
    names
}

/// What follows `static` in a declaration, with or without `pub` and `mut`.
fn static_declaration(line: &str) -> Option<&str> {
    let line = line
        .strip_prefix("pub(crate) ")
        .or_else(|| line.strip_prefix("pub(super) "))
        .or_else(|| line.strip_prefix("pub "))
        .unwrap_or(line);
    let rest = line.strip_prefix("static ")?;
    Some(rest.strip_prefix("mut ").unwrap_or(rest))
}

fn opens_module(line: &str) -> bool {
    let line = line
        .strip_prefix("pub ")
        .or_else(|| line.strip_prefix("pub(crate) "))
        .unwrap_or(line);
    line.starts_with("mod ") && line.trim_end().ends_with('{')
}

/// The attribute lines right above line `index`.
fn attributes_above<'a>(lines: &'a [&'a str], index: usize) -> impl Iterator<Item = &'a str> {
    lines[..index]
        .iter()
        .rev()
        .map(|line| line.trim())
        .take_while(|line| line.starts_with("#[") || line.starts_with("///"))
        .filter(|line| line.starts_with("#["))
}

/// Where `needle` starts in `code`.
fn occurrences(code: &str, needle: &str) -> Vec<usize> {
    code.match_indices(needle).map(|(at, _)| at).collect()
}

/// The line number, from 1, of the byte offset `at`.
fn line_of(code: &str, at: usize) -> usize {
    code[..at].matches('\n').count() + 1
}

/// The text from `start`, just past an opening parenthesis, to its closing one.
fn arguments(code: &str, start: usize) -> &str {
    let mut depth = 1;
    for (offset, c) in code[start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &code[start..start + offset];
                }
            }
            _ => {}
        }
    }
    &code[start..]
}

/// `source` with comments and the insides of string and character literals blanked out, so the rules see only code;
/// lines stay where they were.
fn code_only(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
        } else if c == 'r' && (next == Some('"') || next == Some('#')) && !is_ident(chars.get(i.wrapping_sub(1))) {
            let hashes = chars[i + 1..].iter().take_while(|&&h| h == '#').count();
            if chars.get(i + 1 + hashes) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            out.push('r');
            out.extend(std::iter::repeat_n('#', hashes));
            out.push('"');
            i += 2 + hashes;
            while i < chars.len() {
                if chars[i] == '"' && chars[i + 1..].iter().take(hashes).filter(|&&h| h == '#').count() == hashes {
                    out.push('"');
                    out.extend(std::iter::repeat_n('#', hashes));
                    i += 1 + hashes;
                    break;
                }
                out.push(blank(chars[i]));
                i += 1;
            }
        } else if c == '"' {
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    out.push(' ');
                    i += 1;
                }
                if i < chars.len() {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
            out.push('"');
            i += 1;
        } else if c == '\'' && next == Some('\\') {
            let end = chars[i + 2..]
                .iter()
                .position(|&q| q == '\'')
                .map_or(chars.len(), |p| i + 3 + p);
            out.push_str("' '");
            out.extend(std::iter::repeat_n(' ', end.saturating_sub(i + 3)));
            i = end;
        } else if c == '\'' && chars.get(i + 2) == Some(&'\'') {
            out.push_str("' '");
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn is_ident(c: Option<&char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || *c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_texts_are_no_code() {
        let code =
            code_only("let a = \"mem::forget(x)\"; // RcBlock::new(|| 1)\nlet b = '\"'; /* setSpacing(4.0) */ c");
        assert!(!code.contains("forget") && !code.contains("RcBlock") && !code.contains("setSpacing"));
        assert!(code.ends_with(" c"));
        assert_eq!(code.lines().count(), 2);
        assert!(!code_only("r#\"a \" mem::forget( \"#").contains("forget"));
    }

    #[test]
    fn lookups_of_the_app_context_are_counted_without_their_definitions() {
        let code = "pub fn app() -> Rc<AppContext> {}\nfn try_app() {}\nlet a = crate::context::app();\n\
                    let b = try_app()?; let c = my_app(); let d = app();";
        assert_eq!(app_lookups(code), 3);
        assert_eq!(lookup_problem(APP_LOOKUPS), None);
        assert!(lookup_problem(APP_LOOKUPS + 1).is_some());
    }

    #[test]
    fn a_leak_an_unguarded_block_and_a_literal_size_are_found() {
        let source = "fn f() {\n    std::mem::forget(token);\n    let b = RcBlock::new(move || {\n        go();\n    });\n    \
                      let g = RcBlock::new(move || guarded((), || go()));\n    let e = RcBlock::new(|_: Bool| {});\n    \
                      let s = NSFont::systemFontOfSize(\n        13.0,\n    );\n    stack.setSpacing(style::space::M);\n}\n";
        let problems = file_problems("src/a.rs", source);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].starts_with("src/a.rs:2: mem::forget"));
        assert!(problems[1].starts_with("src/a.rs:3: the body"));
        assert!(problems[2].starts_with("src/a.rs:8: a literal size"));
        assert!(file_problems("crates/ide-ui-common/src/style.rs", "NSFont::systemFontOfSize(13.0)").is_empty());
    }

    #[test]
    fn a_discarded_spawn_is_found_and_a_handled_one_is_not() {
        let source = "let _ = std::thread::Builder::new()\n    .name(\"x\".into())\n    .spawn(move || {\n        a();\n    });\n\
                      let _ = done; spawn(x);\nif let Err(e) = Builder::new().spawn(f) { report(e) }\n";
        let problems = discarded_threads("crates/x/src/lib.rs", source);
        assert_eq!(
            problems,
            [
                "crates/x/src/lib.rs:1: the result of spawning is discarded; hand a failure to the \
                               caller or the user"
            ]
        );
    }

    #[test]
    fn statics_with_state_count_unless_built_for_tests() {
        let source = "static A: Mutex<u8> = Mutex::new(0);\nstatic B: &str = \"x\";\npub static C: AtomicU64 = \
                      AtomicU64::new(0);\nthread_local! {\n    static D: RefCell<u8> = RefCell::new(0);\n}\n\
                      #[cfg(feature = \"selftest\")]\nstatic E: Mutex<u8> = Mutex::new(0);\nfn f() {\n    static F: \
                      OnceLock<u8> = OnceLock::new();\n}\n#[cfg(test)]\nmod tests {\n    static G: AtomicU64 = \
                      AtomicU64::new(0);\n}\nstatic H: Lane = Lane::new(\"h\", 1, 1);\n";
        assert_eq!(statics(source), ["A", "C", "F", "H"]);
    }

    #[test]
    fn the_inventory_lists_statics_only_below_their_heading() {
        let inventory = "| File | Static | Holds | Lives | Target |\n|---|---|---|---|---|\n\
                         | `src/a.rs` | `A` | x | app | stays: cache |\n\n## Statics\n\n\
                         | File | Static | Holds | Reason |\n|---|---|---|---|\n| `src/b.rs` | `B` | y | serial |\n";
        assert_eq!(
            allowed(inventory),
            BTreeSet::from([("src/b.rs".to_string(), "B".to_string())])
        );
        let found = BTreeSet::from([("src/c.rs".to_string(), "C".to_string())]);
        assert_eq!(unlisted(&allowed(inventory), &found).len(), 2);
    }

    #[test]
    fn the_app_code_as_it_is_passes() {
        let root = root();
        let inventory = std::fs::read_to_string(root.join(INVENTORY)).unwrap();
        let allowed = allowed(&inventory);
        let mut found = BTreeSet::new();
        let mut problems = Vec::new();
        for file in rust_files(&root.join(APP))
            .into_iter()
            .chain(rust_files(&root.join("crates")))
        {
            let relative = relative(&root, &file);
            if in_app(&relative) {
                let text = std::fs::read_to_string(&file).unwrap();
                problems.extend(file_problems(&relative, &text));
                problems.extend(discarded_threads(&relative, &text));
                if !relative.ends_with(SELFTEST_PROBE) {
                    found.extend(statics(&text).into_iter().map(|name| (relative.clone(), name)));
                }
            }
        }
        problems.extend(unlisted(&allowed, &found));
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
