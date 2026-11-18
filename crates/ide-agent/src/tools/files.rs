//! Reading, editing and writing files; every change comes back as hunks for the card's diff.

use std::fs;
use std::io::Read;
use std::path::Path;

use ide_thread::{Hunk, ToolOutput};
use serde_json::Value;
use similar::{ChangeTag, TextDiff};

use super::{Outcome, number, required};
use crate::notes::ToolNote;
use crate::workspace::Workspace;

/// Lines read when the call names no limit.
const DEFAULT_LINES: u64 = 2000;
/// Characters of one line that are shown.
const LINE_WIDTH: usize = 2000;
/// Files larger than this are not read whole.
const MAX_FILE: u64 = 20 * 1024 * 1024;

/// The failure of a file operation on `shown`, in the system's words.
fn io(shown: &str) -> impl FnOnce(std::io::Error) -> ToolNote + '_ {
    move |error| ToolNote::Io {
        path: shown.to_string(),
        detail: error.to_string(),
    }
}

/// The bytes of what is at `path` and no regular file, such as a device a link in the project leads to, which can
/// go on without end: at most `MAX_FILE` of them.
fn bounded(path: &Path, shown: &str) -> Result<Vec<u8>, ToolNote> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(MAX_FILE + 1).read_to_end(&mut bytes))
        .map_err(io(shown))?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > MAX_FILE {
        return Err(ToolNote::TooLarge {
            path: shown.to_string(),
            bytes: read,
        });
    }
    Ok(bytes)
}

/// The text of the file at `path`, as `fs::read_to_string` reads it; [`bounded`] when it is no regular file.
fn text_of(path: &Path, shown: &str) -> Result<String, ToolNote> {
    if fs::metadata(path).is_ok_and(|meta| meta.is_file()) {
        return fs::read_to_string(path).map_err(io(shown));
    }
    String::from_utf8(bounded(path, shown)?).map_err(|_| {
        io(shown)(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        ))
    })
}

pub(super) fn read(input: &Value, workspace: &Workspace) -> Result<Outcome, ToolNote> {
    let path = workspace.resolve(required(input, "path")?);
    let shown = workspace.display(&path);
    let meta = fs::metadata(&path).map_err(io(&shown))?;
    if meta.is_dir() {
        return Err(ToolNote::Folder(shown));
    }
    if meta.len() > MAX_FILE {
        return Err(ToolNote::TooLarge {
            path: shown,
            bytes: meta.len(),
        });
    }
    let bytes = if meta.is_file() {
        fs::read(&path).map_err(io(&shown))?
    } else {
        bounded(&path, &shown)?
    };
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Err(ToolNote::Binary(shown));
    }
    let text = String::from_utf8_lossy(&bytes);
    let first = number(input, "offset").unwrap_or(1).max(1);
    let limit = number(input, "limit").unwrap_or(DEFAULT_LINES).max(1);
    let total = text.lines().count();
    let skip = usize::try_from(first - 1).unwrap_or(usize::MAX);
    let take = usize::try_from(limit).unwrap_or(usize::MAX);
    let lines: Vec<String> = text
        .lines()
        .enumerate()
        .skip(skip)
        .take(take)
        .map(|(n, line)| {
            let shown: String = line.chars().take(LINE_WIDTH).collect();
            let cut = if shown.len() < line.len() { " …" } else { "" };
            format!("{:>6}\t{shown}{cut}", n + 1)
        })
        .collect();
    if lines.is_empty() {
        return Ok(Outcome::noted(if total == 0 {
            ToolNote::EmptyFile(shown)
        } else {
            ToolNote::PastEnd {
                path: shown,
                lines: total,
            }
        }));
    }
    let shown_lines = lines.join("\n");
    let mut content = shown_lines.clone();
    let rest = total.saturating_sub(skip + lines.len());
    if rest > 0 {
        content.push_str(&format!(
            "\n({rest} more lines; read on with offset {})",
            skip + lines.len() + 1
        ));
    }
    Ok(Outcome {
        content,
        output: Some(ToolOutput::Text(shown_lines)),
        ..Outcome::default()
    })
}

pub(super) fn edit(input: &Value, workspace: &Workspace) -> Result<Outcome, ToolNote> {
    let path = workspace.resolve(required(input, "path")?);
    let shown = workspace.display(&path);
    let (old, new) = (required(input, "old_string")?, required(input, "new_string")?);
    let all = input.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
    if old.is_empty() {
        return Err(ToolNote::EmptyEdit);
    }
    if old == new {
        return Err(ToolNote::NoChange);
    }
    let before = text_of(&path, &shown)?;
    let crlf = before.contains("\r\n") && !old.contains('\r');
    let (old, new) = if crlf {
        (old.replace('\n', "\r\n"), new.replace('\n', "\r\n"))
    } else {
        (old.to_string(), new.to_string())
    };
    let count = before.matches(old.as_str()).count();
    if count == 0 {
        return Err(ToolNote::NotFound(shown));
    }
    if count > 1 && !all {
        return Err(ToolNote::Ambiguous { path: shown, count });
    }
    let after = if all {
        before.replace(&old, &new)
    } else {
        before.replacen(&old, &new, 1)
    };
    fs::write(&path, &after).map_err(io(&shown))?;
    let replaced = if all { count } else { 1 };
    Ok(Outcome {
        content: format!("Edited {shown}: {replaced} replacement(s)."),
        output: Some(ToolOutput::Diff {
            path: shown,
            hunks: hunks(&before, &after),
        }),
        ..Outcome::default()
    })
}

pub(super) fn write(input: &Value, workspace: &Workspace) -> Result<Outcome, ToolNote> {
    let path = workspace.resolve(required(input, "path")?);
    let shown = workspace.display(&path);
    let content = required(input, "content")?;
    if path.is_dir() {
        return Err(ToolNote::Folder(shown));
    }
    let before = text_of(&path, &shown).ok();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io(&workspace.display(parent)))?;
    }
    fs::write(&path, content).map_err(io(&shown))?;
    let verb = if before.is_some() { "Wrote" } else { "Created" };
    Ok(Outcome {
        content: format!("{verb} {shown} ({} lines).", content.lines().count()),
        output: Some(ToolOutput::Diff {
            path: shown,
            hunks: hunks(before.as_deref().unwrap_or_default(), content),
        }),
        ..Outcome::default()
    })
}

/// The hunks that turn `old` into `new`, with three lines of context each.
pub(crate) fn hunks(old: &str, new: &str) -> Vec<Hunk> {
    let count = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
    let diff = TextDiff::from_lines(old, new);
    diff.grouped_ops(3)
        .iter()
        .filter_map(|group| {
            let (first, last) = (group.first()?, group.last()?);
            let (old_range, new_range) = (
                first.old_range().start..last.old_range().end,
                first.new_range().start..last.new_range().end,
            );
            let lines = group
                .iter()
                .flat_map(|op| diff.iter_changes(op))
                .map(|change| {
                    let sign = match change.tag() {
                        ChangeTag::Equal => ' ',
                        ChangeTag::Delete => '-',
                        ChangeTag::Insert => '+',
                    };
                    format!("{sign}{}", change.value().trim_end_matches(['\n', '\r']))
                })
                .collect();
            Some(Hunk {
                old_start: count(old_range.start) + 1,
                old_lines: count(old_range.len()),
                new_start: count(new_range.start) + 1,
                new_lines: count(new_range.len()),
                lines,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunks_carry_context_and_line_numbers() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let new = "a\nb\nc\nD\ne\nf\ng\nh\nI\n";
        let hunks = hunks(old, new);
        assert_eq!(hunks.len(), 1);
        let hunk = &hunks[0];
        assert_eq!(
            (hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines),
            (1, 8, 1, 9)
        );
        assert!(hunk.lines.contains(&"-d".to_string()) && hunk.lines.contains(&"+D".to_string()));
        assert_eq!(hunk.lines.last().map(String::as_str), Some("+I"));
        assert!(super::hunks("x\n", "x\n").is_empty());
    }

    #[test]
    fn a_link_to_a_device_without_end_is_read_no_further_than_a_large_file() {
        let dir = ide_core::testing::scratch("agent-files-device");
        std::os::unix::fs::symlink("/dev/zero", dir.join("zero")).unwrap();
        let workspace = Workspace::new(
            dir.path(),
            Vec::new(),
            ide_process::Supervisor::new(),
            crate::http::Http::new(),
        )
        .unwrap();
        let input = serde_json::json!({"path": "zero"});
        assert!(matches!(read(&input, &workspace), Err(ToolNote::TooLarge { .. })));
        let change = serde_json::json!({"path": "zero", "old_string": "a", "new_string": "b"});
        assert!(matches!(edit(&change, &workspace), Err(ToolNote::TooLarge { .. })));
        let _ = std::fs::remove_dir_all(dir);
    }
}
