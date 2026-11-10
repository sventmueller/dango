//! Files the folder's watcher saw change, passed to each server that registered a watcher for them, since servers
//! are told that the app watches files for them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ide_lsp::lsp_types::{FileChangeType, FileEvent};
use ide_lsp::uri;
use ide_project::FileChange;
use serde_json::Value;

/// Whether `path` matches the glob `pattern`: `**` any run of folders, `*` and `?` within a name, `{a,b}`
/// alternatives and `[…]` classes, as the protocol's `GlobPattern` has them.
pub fn matches(pattern: &str, path: &str) -> bool {
    alternatives(pattern).iter().any(|p| {
        let pattern: Vec<char> = p.chars().collect();
        let path: Vec<char> = path.chars().collect();
        glob(&pattern, &path)
    })
}

/// The patterns `{a,b}` stands for, braces expanded from the first.
fn alternatives(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_string()];
    };
    let mut depth = 0;
    let mut close = None;
    let mut cuts = vec![];
    for (i, c) in pattern[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            ',' if depth == 1 => cuts.push(open + i),
            _ => {}
        }
    }
    let Some(close) = close else {
        return vec![pattern.to_string()];
    };
    let mut bounds = vec![open];
    bounds.extend(cuts);
    bounds.push(close);
    let (before, after) = (&pattern[..open], &pattern[close + 1..]);
    bounds
        .windows(2)
        .flat_map(|w| alternatives(&format!("{before}{}{after}", &pattern[w[0] + 1..w[1]])))
        .collect()
}

fn glob(pattern: &[char], path: &[char]) -> bool {
    match pattern.first() {
        None => path.is_empty(),
        Some('*') if pattern.get(1) == Some(&'*') => {
            let rest = &pattern[2..];
            if rest.first() == Some(&'/') && glob(&rest[1..], path) {
                return true;
            }
            (0..=path.len()).any(|skip| glob(rest, &path[skip..]))
        }
        Some('*') => (0..=path.len())
            .take_while(|&skip| skip == 0 || path[skip - 1] != '/')
            .any(|skip| glob(&pattern[1..], &path[skip..])),
        Some('?') => path.first().is_some_and(|c| *c != '/') && glob(&pattern[1..], &path[1..]),
        Some('[') => {
            let Some(end) = pattern.iter().position(|c| *c == ']') else {
                return path.first() == Some(&'[') && glob(&pattern[1..], &path[1..]);
            };
            let Some(c) = path.first() else { return false };
            class(&pattern[1..end], *c) && glob(&pattern[end + 1..], &path[1..])
        }
        Some(c) => path.first() == Some(c) && glob(&pattern[1..], &path[1..]),
    }
}

/// Whether `c` belongs to the class `[set]`, such as `[a-z]` or `[!.]`.
fn class(set: &[char], c: char) -> bool {
    let (negated, set) = match set.first() {
        Some('!' | '^') => (true, &set[1..]),
        _ => (false, set),
    };
    let mut found = false;
    let mut i = 0;
    while i < set.len() {
        if set.get(i + 1) == Some(&'-') && i + 2 < set.len() {
            found |= (set[i]..=set[i + 2]).contains(&c);
            i += 3;
        } else {
            found |= set[i] == c;
            i += 1;
        }
    }
    found != negated
}

/// The kinds a watcher asks for as the protocol's bits: 1 created, 2 changed, 4 deleted; all when it says none.
fn wants(watcher: &Value, change: FileChange) -> bool {
    let kinds = watcher.get("kind").and_then(Value::as_u64).unwrap_or(7);
    let bit = match change {
        FileChange::Created => 1,
        FileChange::Changed => 2,
        FileChange::Deleted => 4,
    };
    kinds & bit != 0
}

/// Whether the watcher `watcher`, as the server registered it, covers `path`.
fn covers(watcher: &Value, path: &Path) -> bool {
    let full = path.to_string_lossy();
    match watcher.get("globPattern") {
        Some(Value::String(pattern)) => matches(pattern, &full),
        Some(Value::Object(relative)) => {
            let base = relative
                .get("baseUri")
                .and_then(|b| b.as_str().or_else(|| b.get("uri").and_then(Value::as_str)))
                .and_then(|b| b.parse().ok())
                .and_then(|b| uri::to_path(&b));
            let pattern = relative.get("pattern").and_then(Value::as_str).unwrap_or("");
            base.and_then(|base| path.strip_prefix(base).ok().map(Path::to_path_buf))
                .is_some_and(|rest| matches(pattern, &rest.to_string_lossy()))
        }
        _ => false,
    }
}

/// The events of `files` that the registrations `registered` (each `DidChangeWatchedFilesRegistrationOptions`)
/// ask for.
pub fn events(registered: &[Value], files: &BTreeMap<PathBuf, FileChange>) -> Vec<FileEvent> {
    let watchers: Vec<&Value> = registered
        .iter()
        .filter_map(|options| options.get("watchers").and_then(Value::as_array))
        .flatten()
        .collect();
    files
        .iter()
        .filter(|(path, change)| watchers.iter().any(|w| wants(w, **change) && covers(w, path)))
        .map(|(path, change)| FileEvent {
            uri: uri::from_path(path),
            typ: match change {
                FileChange::Created => FileChangeType::CREATED,
                FileChange::Changed => FileChangeType::CHANGED,
                FileChange::Deleted => FileChangeType::DELETED,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn globs_match_as_the_protocol_describes() {
        assert!(matches("**/*.rs", "/p/src/main.rs"));
        assert!(matches("**/*.rs", "main.rs"), "** also matches no folder");
        assert!(!matches("*.rs", "src/main.rs"), "* stays within a name");
        assert!(matches("/p/**/Cargo.{toml,lock}", "/p/crates/a/Cargo.lock"));
        assert!(matches("src/?.go", "src/a.go"));
        assert!(!matches("src/?.go", "src/ab.go"));
        assert!(matches("[a-c]x[!y]", "bxz"));
        assert!(!matches("[a-c]x[!y]", "bxy"));
        assert!(matches("**/{go.mod,go.work,*.go}", "/p/go.work"));
    }

    #[test]
    fn only_registered_files_and_kinds_are_passed_on() {
        let registered = vec![json!({"watchers": [
            {"globPattern": "**/*.rs"},
            {"globPattern": {"baseUri": "file:///p", "pattern": "**/Cargo.toml"}, "kind": 2},
        ]})];
        let files = BTreeMap::from([
            (PathBuf::from("/p/src/new.rs"), FileChange::Created),
            (PathBuf::from("/p/Cargo.toml"), FileChange::Changed),
            (PathBuf::from("/p/old/Cargo.toml"), FileChange::Deleted),
            (PathBuf::from("/p/README.md"), FileChange::Changed),
        ]);
        let seen: Vec<(String, FileChangeType)> = events(&registered, &files)
            .into_iter()
            .map(|e| (e.uri.as_str().to_string(), e.typ))
            .collect();
        assert_eq!(
            seen,
            [
                ("file:///p/Cargo.toml".to_string(), FileChangeType::CHANGED),
                ("file:///p/src/new.rs".to_string(), FileChangeType::CREATED),
            ]
        );
    }
}
