//! Reading, editing and writing files; every change comes back as hunks for the card's diff.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use ide_thread::{Hunk, ToolOutput};
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;
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

/// The bytes of `file`, also of a device a link in the project leads to, which can go on without end: at most
/// `MAX_FILE` of them.
fn bounded(file: fs::File, shown: &str) -> Result<Vec<u8>, ToolNote> {
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1).read_to_end(&mut bytes).map_err(io(shown))?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > MAX_FILE {
        return Err(ToolNote::TooLarge {
            path: shown.to_string(),
            bytes: read,
        });
    }
    Ok(bytes)
}

/// The folder a write to `path`, a resolved path, goes into, opened and still the folder `path` names, and the
/// file's name in it.
fn folder_of<'a>(path: &'a Path, shown: &str) -> Result<(fs::File, &'a OsStr), ToolNote> {
    let moved = || io(shown)(Errno::LOOP.into());
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(moved());
    };
    let folder = fs::File::open(parent).map_err(io(shown))?;
    let real = rustix::fs::getpath(&folder).map(|real| PathBuf::from(OsString::from_vec(real.into_bytes())));
    if real.ok().as_deref() != Some(parent) {
        return Err(moved());
    }
    Ok((folder, name))
}

/// The file `name` in `folder`, opened with `flags` but never through a link.
fn open_in(folder: &fs::File, name: &OsStr, flags: OFlags) -> std::io::Result<fs::File> {
    let flags = flags | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    Ok(rustix::fs::openat(folder, name, flags, Mode::from_bits_truncate(0o666))?.into())
}

/// The text of the file `name` in `folder`, at most `MAX_FILE` of it.
fn text_in(folder: &fs::File, name: &OsStr, shown: &str) -> Result<String, ToolNote> {
    let file = open_in(folder, name, OFlags::RDONLY).map_err(io(shown))?;
    if let Ok(meta) = file.metadata()
        && meta.is_file()
        && meta.len() > MAX_FILE
    {
        return Err(ToolNote::TooLarge {
            path: shown.to_string(),
            bytes: meta.len(),
        });
    }
    String::from_utf8(bounded(file, shown)?).map_err(|_| {
        io(shown)(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        ))
    })
}

/// Writes `content` as the file `name` in `folder`, creating it; an existing file keeps its mode. A file with further
/// hard links gets a new file in its place, so the write never reaches a link outside the project.
fn write_in(folder: &fs::File, name: &OsStr, content: &str, shown: &str) -> Result<(), ToolNote> {
    let mut file = open_in(folder, name, OFlags::WRONLY | OFlags::CREATE).map_err(io(shown))?;
    let meta = file.metadata().map_err(io(shown))?;
    if std::os::unix::fs::MetadataExt::nlink(&meta) > 1 {
        return replace_in(folder, name, content, meta.permissions()).map_err(io(shown));
    }
    file.set_len(0)
        .and_then(|()| file.write_all(content.as_bytes()))
        .map_err(io(shown))
}

/// Puts a new file with `content` and `permissions` in the place of `name` in `folder`.
fn replace_in(folder: &fs::File, name: &OsStr, content: &str, permissions: fs::Permissions) -> std::io::Result<()> {
    let mut attempt = 0;
    let (temporary, mut file) = loop {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let mut temporary = OsString::from(".");
        temporary.push(name);
        temporary.push(format!(".{}-{nanos}-{attempt}", std::process::id()));
        match open_in(folder, &temporary, OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL) {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && attempt < 8 => attempt += 1,
            Err(error) => return Err(error),
        }
    };
    let written = file
        .set_permissions(permissions)
        .and_then(|()| file.write_all(content.as_bytes()))
        .and_then(|()| Ok(rustix::fs::renameat(folder, &temporary, folder, name)?));
    if written.is_err() {
        let _ = rustix::fs::unlinkat(folder, &temporary, rustix::fs::AtFlags::empty());
    }
    written
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
        bounded(fs::File::open(&path).map_err(io(&shown))?, &shown)?
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

/// Edits the file at `path`, the resolved path the policy judged.
pub(super) fn edit(input: &Value, workspace: &Workspace, path: &Path) -> Result<Outcome, ToolNote> {
    let shown = workspace.display(path);
    let (old, new) = (required(input, "old_string")?, required(input, "new_string")?);
    let all = input.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
    if old.is_empty() {
        return Err(ToolNote::EmptyEdit);
    }
    if old == new {
        return Err(ToolNote::NoChange);
    }
    let (folder, name) = folder_of(path, &shown)?;
    let before = text_in(&folder, name, &shown)?;
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
    write_in(&folder, name, &after, &shown)?;
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

/// Writes the file at `path`, the resolved path the policy judged.
pub(super) fn write(input: &Value, workspace: &Workspace, path: &Path) -> Result<Outcome, ToolNote> {
    let shown = workspace.display(path);
    let content = required(input, "content")?;
    if path.is_dir() {
        return Err(ToolNote::Folder(shown));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io(&workspace.display(parent)))?;
    }
    let (folder, name) = folder_of(path, &shown)?;
    let before = text_in(&folder, name, &shown).ok();
    write_in(&folder, name, content, &shown)?;
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

    fn project(name: &str) -> (ide_core::testing::Scratch, Workspace) {
        let dir = ide_core::testing::scratch(&format!("agent-files-{name}"));
        fs::create_dir_all(dir.join("project/sub")).unwrap();
        fs::create_dir_all(dir.join("outside")).unwrap();
        let workspace = Workspace::new(
            &dir.join("project"),
            Vec::new(),
            ide_process::Supervisor::new(),
            crate::http::Http::new(),
        )
        .unwrap();
        (dir, workspace)
    }

    /// The path the policy judges a write of `input` at, and whether it found it inside.
    fn judged(name: &str, input: &Value, workspace: &Workspace) -> (PathBuf, bool) {
        match crate::tools::checked(name, input, workspace) {
            Ok((crate::policy::Action::Write { inside, .. }, Some(path))) => (path, inside),
            other => panic!("no write: {other:?}"),
        }
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
        let (path, _) = judged("edit_file", &change, &workspace);
        assert!(matches!(
            edit(&change, &workspace, &path),
            Err(ToolNote::TooLarge { .. })
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_too_large_to_read_is_not_edited() {
        let (dir, workspace) = project("large");
        let large = fs::File::create(workspace.root().join("large.txt")).unwrap();
        large.set_len(MAX_FILE + 1).unwrap();
        let change = serde_json::json!({"path": "large.txt", "old_string": "a", "new_string": "b"});
        let (path, _) = judged("edit_file", &change, &workspace);
        assert!(matches!(
            edit(&change, &workspace, &path),
            Err(ToolNote::TooLarge { bytes, .. }) if bytes == MAX_FILE + 1
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn files_are_written_created_and_edited_as_before() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, workspace) = project("plain");
        let root = workspace.root().to_path_buf();
        let input = serde_json::json!({"path": "new/deeper/a.txt", "content": "one\ntwo\n"});
        let (path, inside) = judged("write_file", &input, &workspace);
        assert!(inside);
        let written = write(&input, &workspace, &path).unwrap();
        assert!(
            written.content.starts_with("Created new/deeper/a.txt"),
            "{}",
            written.content
        );
        assert_eq!(fs::read_to_string(root.join("new/deeper/a.txt")).unwrap(), "one\ntwo\n");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let change = serde_json::json!({"path": "new/deeper/a.txt", "old_string": "two", "new_string": "three"});
        let (path, _) = judged("edit_file", &change, &workspace);
        edit(&change, &workspace, &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\nthree\n");
        let again = serde_json::json!({"path": "new/deeper/a.txt", "content": "four\n"});
        assert!(write(&again, &workspace, &path).unwrap().content.starts_with("Wrote"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "four\n");
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "an existing file keeps its mode");
        std::os::unix::fs::symlink("sub/target.txt", root.join("inner")).unwrap();
        let through = serde_json::json!({"path": "inner", "content": "linked\n"});
        let (path, inside) = judged("write_file", &through, &workspace);
        assert!(inside);
        write(&through, &workspace, &path).unwrap();
        assert_eq!(fs::read_to_string(root.join("sub/target.txt")).unwrap(), "linked\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_link_out_of_the_project_is_judged_outside_and_never_written_through() {
        let (dir, workspace) = project("link-out");
        let root = workspace.root().to_path_buf();
        let target = dir.join("outside/.zshrc");
        fs::write(&target, "kept\n").unwrap();
        std::os::unix::fs::symlink(&target, root.join("notes.md")).unwrap();
        let input = serde_json::json!({"path": "notes.md", "content": "planted\n"});
        let (_, inside) = judged("write_file", &input, &workspace);
        assert!(!inside, "the policy sees where the link leads");
        let link = root.join("notes.md");
        assert!(write(&input, &workspace, &link).is_err());
        let change = serde_json::json!({"path": "notes.md", "old_string": "kept", "new_string": "planted"});
        assert!(edit(&change, &workspace, &link).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "kept\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_hard_link_to_a_file_outside_gets_a_file_of_its_own() {
        let (dir, workspace) = project("hard-link");
        let root = workspace.root().to_path_buf();
        let target = dir.join("outside/.zshrc");
        fs::write(&target, "kept\n").unwrap();
        fs::hard_link(&target, root.join("notes.md")).unwrap();
        let input = serde_json::json!({"path": "notes.md", "content": "planted\n"});
        let (path, inside) = judged("write_file", &input, &workspace);
        assert!(inside, "a hard link looks like a file of the project");
        write(&input, &workspace, &path).unwrap();
        let change = serde_json::json!({"path": "notes.md", "old_string": "planted", "new_string": "edited"});
        edit(&change, &workspace, &path).unwrap();
        assert_eq!(fs::read_to_string(root.join("notes.md")).unwrap(), "edited\n");
        assert_eq!(fs::read_to_string(&target).unwrap(), "kept\n");
        let left = fs::read_dir(&root)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".notes.md")
            })
            .count();
        assert_eq!(left, 0, "no temporary file is left behind");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_link_swapped_in_after_the_check_is_refused() {
        let (dir, workspace) = project("swap");
        let root = workspace.root().to_path_buf();
        let target = dir.join("outside/.zshrc");
        fs::write(&target, "kept\n").unwrap();
        fs::write(root.join("notes.md"), "kept\n").unwrap();
        let input = serde_json::json!({"path": "notes.md", "content": "planted\n"});
        let change = serde_json::json!({"path": "notes.md", "old_string": "kept", "new_string": "planted"});
        let (path, inside) = judged("write_file", &input, &workspace);
        assert!(inside);
        fs::remove_file(root.join("notes.md")).unwrap();
        std::os::unix::fs::symlink(&target, root.join("notes.md")).unwrap();
        assert!(write(&input, &workspace, &path).is_err());
        assert!(edit(&change, &workspace, &path).is_err());
        let file = serde_json::json!({"path": "sub/a.txt", "content": "planted\n"});
        let (path, inside) = judged("write_file", &file, &workspace);
        assert!(inside);
        fs::rename(root.join("sub"), root.join("moved")).unwrap();
        std::os::unix::fs::symlink(dir.join("outside"), root.join("sub")).unwrap();
        assert!(write(&file, &workspace, &path).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "kept\n");
        assert!(!dir.join("outside/a.txt").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
