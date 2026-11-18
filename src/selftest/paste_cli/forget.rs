//! Taking the scenario's folders out of the CLIs' own settings: Claude Code keeps an entry per folder under `projects`
//! in `~/.claude.json`, agy a list of `trustedWorkspaces` in `~/.gemini/antigravity-cli/settings.json`, Codex a
//! `[projects.'…']` table in `~/.codex/config.toml`. Only entries for the scenario's own folders go; the rest of each
//! file stays byte for byte, a JSON file is written only when it reads back as the same document minus those entries,
//! and only when it did not change while it was edited, as running CLIs write these files too.

use std::path::Path;

use serde_json::Value;

/// A JSON value's end, from its first byte at `at`.
fn value_end(text: &[u8], at: usize) -> Option<usize> {
    match text.get(at)? {
        b'"' => {
            let mut i = at + 1;
            while i < text.len() {
                match text[i] {
                    b'\\' => i += 2,
                    b'"' => return Some(i + 1),
                    _ => i += 1,
                }
            }
            None
        }
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut i = at;
            while i < text.len() {
                match text[i] {
                    b'"' => {
                        i = value_end(text, i)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(i + 1);
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            None
        }
        _ => {
            let end = text[at..]
                .iter()
                .position(|&b| matches!(b, b',' | b'}' | b']') || b.is_ascii_whitespace())
                .map_or(text.len(), |n| at + n);
            Some(end)
        }
    }
}

fn skip_space(text: &[u8], mut at: usize) -> usize {
    while text.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}

/// One member of an object or element of an array: where it starts and ends, and its key or string value.
struct Entry {
    start: usize,
    end: usize,
    name: Option<String>,
}

/// The entries of the object or array that starts at `open`, and where it closes.
fn entries(source: &str, open: usize) -> Option<(Vec<Entry>, usize)> {
    let text = source.as_bytes();
    let object = text.get(open)? == &b'{';
    let mut entries = Vec::new();
    let mut at = skip_space(text, open + 1);
    while text.get(at).is_some_and(|&b| b != b'}' && b != b']') {
        let start = at;
        let first_end = value_end(text, at)?;
        let name = serde_json::from_str::<String>(&source[at..first_end]).ok();
        let end = if object {
            let colon = skip_space(text, first_end);
            value_end(text, skip_space(text, colon + 1))?
        } else {
            first_end
        };
        entries.push(Entry { start, end, name });
        at = skip_space(text, end);
        if text.get(at) == Some(&b',') {
            at = skip_space(text, at + 1);
        }
    }
    Some((entries, at))
}

/// The value of member `key` of the object at `open`, where it starts.
fn member(source: &str, open: usize, key: &str) -> Option<usize> {
    let (entries, _) = entries(source, open)?;
    let entry = entries.into_iter().find(|e| e.name.as_deref() == Some(key))?;
    let text = source.as_bytes();
    let colon = skip_space(text, value_end(text, entry.start)?);
    Some(skip_space(text, colon + 1))
}

/// `source` without the entries of the container at `path` whose key, or string value, `doomed` picks, and how many
/// went; `None` when none matched or the result is not the same document minus them.
pub(crate) fn remove(source: &str, path: &[&str], doomed: impl Fn(&str) -> bool) -> Option<(String, usize)> {
    let mut open = skip_space(source.as_bytes(), 0);
    for key in path {
        open = member(source, open, key)?;
    }
    let (entries, close) = entries(source, open)?;
    let kept: Vec<&Entry> = entries
        .iter()
        .filter(|e| !e.name.as_deref().is_some_and(&doomed))
        .collect();
    let removed = entries.len() - kept.len();
    if removed == 0 {
        return None;
    }
    let inner = match (entries.first(), kept.is_empty()) {
        (Some(first), false) => {
            let lead = &source[open + 1..first.start];
            let separator = entries
                .windows(2)
                .next()
                .map_or(",", |pair| &source[pair[0].end..pair[1].start]);
            let trail = &source[entries.last()?.end..close];
            let body: Vec<&str> = kept.iter().map(|e| &source[e.start..e.end]).collect();
            format!("{lead}{}{trail}", body.join(separator))
        }
        _ => String::new(),
    };
    let edited = format!("{}{inner}{}", &source[..=open], &source[close..]);
    let mut expected: Value = serde_json::from_str(source).ok()?;
    let container = path.iter().try_fold(&mut expected, |value, key| value.get_mut(*key))?;
    match container {
        Value::Object(map) => map.retain(|key, _| !doomed(key)),
        Value::Array(items) => items.retain(|item| !item.as_str().is_some_and(&doomed)),
        _ => return None,
    }
    let reread: Value = serde_json::from_str(&edited).ok()?;
    (reread == expected).then_some((edited, removed))
}

/// `source`, a TOML file, without the `[projects."…"]` or `[projects.'…']` tables whose folder `doomed` picks, and how
/// many went.
pub(crate) fn remove_toml_projects(source: &str, doomed: impl Fn(&str) -> bool) -> Option<(String, usize)> {
    let mut out = String::with_capacity(source.len());
    let mut skipping = false;
    let mut removed = 0;
    for line in source.split_inclusive('\n') {
        let header = line.trim();
        if header.starts_with('[') {
            let folder = header
                .strip_prefix("[projects.")
                .and_then(|rest| rest.strip_suffix(']'));
            skipping = folder
                .and_then(|quoted| {
                    quoted
                        .strip_prefix('"')
                        .and_then(|q| q.strip_suffix('"'))
                        .or_else(|| quoted.strip_prefix('\'').and_then(|q| q.strip_suffix('\'')))
                })
                .is_some_and(&doomed);
            removed += usize::from(skipping);
        }
        if !skipping {
            out.push_str(line);
        }
    }
    (removed > 0).then_some((out, removed))
}

/// Replaces `file`, which held `before`, with `text` through a new file beside it with the old one's permissions;
/// `false` when the file changed meanwhile and was left alone.
fn replace(file: &Path, before: &str, text: &str) -> std::io::Result<bool> {
    let permissions = std::fs::metadata(file)?.permissions();
    let fresh = file.with_extension(format!("ide-selftest-{}", std::process::id()));
    std::fs::write(&fresh, text)?;
    std::fs::set_permissions(&fresh, permissions)?;
    if std::fs::read_to_string(file)? != before {
        std::fs::remove_file(&fresh)?;
        return Ok(false);
    }
    std::fs::rename(&fresh, file)?;
    Ok(true)
}

/// Edits `file` with `edit` until the edit lands on the content it was made from, at most a few times.
/// The files among the CLIs' settings that still name a folder `doomed` picks.
pub(crate) fn named(home: &Path, doomed: &dyn Fn(&str) -> bool) -> Vec<String> {
    [
        (home.join(".claude.json"), &["projects"][..]),
        (
            home.join(".gemini/antigravity-cli/settings.json"),
            &["trustedWorkspaces"][..],
        ),
    ]
    .into_iter()
    .filter(|(file, path)| {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|source| remove(&source, path, doomed))
            .is_some()
    })
    .map(|(file, _)| file.display().to_string())
    .chain(
        std::fs::read_to_string(home.join(".codex/config.toml"))
            .ok()
            .and_then(|source| remove_toml_projects(&source, doomed))
            .map(|_| home.join(".codex/config.toml").display().to_string()),
    )
    .collect()
}

fn edit_file(file: &Path, edit: impl Fn(&str) -> Option<(String, usize)>) -> Option<String> {
    for _ in 0..5 {
        let source = std::fs::read_to_string(file).ok()?;
        let (text, removed) = edit(&source)?;
        match replace(file, &source, &text) {
            Ok(true) => return Some(format!("{}: {removed} removed", file.display())),
            Ok(false) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(error) => return Some(format!("{}: {error}", file.display())),
        }
    }
    Some(format!(
        "{}: changed while it was edited, left as it is",
        file.display()
    ))
}

/// Takes out of the CLIs' settings every entry for a folder `doomed` picks; what was removed, per file.
pub(crate) fn forget(home: &Path, doomed: &dyn Fn(&str) -> bool) -> Vec<String> {
    [
        edit_file(&home.join(".claude.json"), |source| {
            remove(source, &["projects"], doomed)
        }),
        edit_file(&home.join(".gemini/antigravity-cli/settings.json"), |source| {
            remove(source, &["trustedWorkspaces"], doomed)
        }),
        edit_file(&home.join(".codex/config.toml"), |source| {
            remove_toml_projects(source, doomed)
        }),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_go_and_everything_else_stays_as_written() {
        let source = "{\n  \"a\": 1,\n  \"projects\": {\n    \"/keep\": {\"x\": [1, \"}\"]},\n    \"/tmp/t-1/p\": {\n      \"trust\": true\n    },\n    \"/keep2\": {}\n  },\n  \"z\": \"\\\"q\"\n}\n";
        let (edited, removed) = remove(source, &["projects"], |key| key.starts_with("/tmp/")).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(
            edited,
            "{\n  \"a\": 1,\n  \"projects\": {\n    \"/keep\": {\"x\": [1, \"}\"]},\n    \"/keep2\": {}\n  },\n  \"z\": \"\\\"q\"\n}\n"
        );
        let list = "{\"trustedWorkspaces\": [\"/a\", \"/tmp/t-2/p\", \"/tmp/t-3/p\"], \"b\": 2}";
        let (edited, removed) = remove(list, &["trustedWorkspaces"], |item| item.starts_with("/tmp/")).unwrap();
        assert_eq!(
            (edited.as_str(), removed),
            ("{\"trustedWorkspaces\": [\"/a\"], \"b\": 2}", 2)
        );
        let only = "{\"trustedWorkspaces\": [\"/tmp/x\"]}";
        assert_eq!(
            remove(only, &["trustedWorkspaces"], |item| item.starts_with("/tmp/")).map(|(t, _)| t),
            Some("{\"trustedWorkspaces\": []}".to_string())
        );
        assert!(remove(list, &["trustedWorkspaces"], |_| false).is_none());
    }

    #[test]
    fn a_codex_project_table_goes_with_its_lines() {
        let source = "model = \"x\"\n\n[projects.'/keep']\ntrust_level = 'trusted'\n\n[projects.\"/tmp/t-1/p\"]\ntrust_level = \"trusted\"\n\n[projects.'/tmp/t-2/p']\ntrust_level = 'trusted'\n\n[tui]\na = 1\n";
        let (edited, removed) = remove_toml_projects(source, |path| path.starts_with("/tmp/")).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(
            edited,
            "model = \"x\"\n\n[projects.'/keep']\ntrust_level = 'trusted'\n\n[tui]\na = 1\n"
        );
    }
}
