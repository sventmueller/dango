//! Freezing an order's agent and scanner configuration (E172, C52, C57): the files of the fixed group are copied from
//! the user's working copy, never the worktree, into a folder of the order's outside anything its builder may write,
//! and the secrets scan reads its gitleaks rules from there. Copies are clones (`clonefile`), so they cost no space
//! until one side changes. Everything here blocks.

use std::io;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

/// Folders a walk for the group does not enter: what they hold is no configuration of the project and can be huge.
const SKIPPED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".build",
    "dist",
    "build",
    ".venv",
    "vendor",
];

/// How deep the walk goes, which keeps a monorepo's walk short.
const DEPTH: usize = 6;

/// The patterns as one set; a pattern that does not parse matches nothing.
fn set(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// The files below `root` that `patterns` name, relative to it.
pub(super) fn matching(root: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let set = set(patterns);
    let mut found = Vec::new();
    walk(root, Path::new(""), &set, 0, &mut found);
    found.sort();
    found
}

fn walk(root: &Path, relative: &Path, set: &GlobSet, depth: usize, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root.join(relative)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let path = relative.join(&name);
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < DEPTH && !SKIPPED.iter().any(|skipped| name == *skipped) {
                walk(root, &path, set, depth + 1, found);
            }
        } else if (kind.is_file() || kind.is_symlink()) && set.is_match(&path) {
            found.push(path);
        }
    }
}

/// Clones `from` to `to`, a file or a folder with all it holds, making `to`'s parents; `to` must not exist.
pub(super) fn clone(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let source = std::ffi::CString::new(from.as_os_str().as_bytes()).map_err(io::Error::other)?;
    let target = std::ffi::CString::new(to.as_os_str().as_bytes()).map_err(io::Error::other)?;
    // SAFETY: Both are NUL-terminated paths that live for the call; `clonefile` reads them and keeps neither.
    let result = unsafe { libc::clonefile(source.as_ptr(), target.as_ptr(), 0) };
    if result == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ENOTSUP) || error.raw_os_error() == Some(libc::EXDEV) {
        return copy(from, to);
    }
    Err(error)
}

/// Copies `from` to `to` byte for byte, for a volume that cannot clone.
fn copy(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)?.flatten() {
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    std::fs::copy(from, to).map(|_| ())
}

/// Freezes the files of `patterns` below `root` into `into`, which is made anew; the folder.
pub(super) fn freeze(root: &Path, patterns: &[String], into: &Path) -> io::Result<PathBuf> {
    if into.exists() {
        std::fs::remove_dir_all(into)?;
    }
    std::fs::create_dir_all(into)?;
    for relative in matching(root, patterns) {
        clone(&root.join(&relative), &into.join(&relative))?;
    }
    Ok(into.to_path_buf())
}

/// The frozen gitleaks rules of an order (C52): `.gitleaks.toml` and `.gitleaksignore` from its frozen folder.
pub(super) fn frozen_rules(frozen: &Path) -> ide_git::FrozenRules {
    let file = |name: &str| Some(frozen.join(name)).filter(|path| path.is_file());
    ide_git::FrozenRules {
        config: file(".gitleaks.toml"),
        ignore: file(".gitleaksignore"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_group_is_frozen_from_the_working_copy_and_nothing_else_is() {
        let base = std::env::temp_dir().join(format!("leitstand-freeze-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("repo");
        for (path, text) in [
            ("CLAUDE.md", "rules"),
            ("crates/a/CLAUDE.md", "more rules"),
            (".claude/settings.json", "{}"),
            (".gitleaks.toml", "[allowlist]"),
            ("node_modules/x/CLAUDE.md", "not this"),
            ("src/lib.rs", "fn main() {}"),
        ] {
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::write(root.join(path), text).unwrap();
        }
        let patterns: Vec<String> = ide_factory::relevance::AGENT_CONFIG
            .iter()
            .map(|p| p.to_string())
            .collect();
        let into = freeze(&root, &patterns, &base.join("frozen")).unwrap();
        let frozen = matching(&into, &["**".to_string()]);
        let names: Vec<&str> = frozen.iter().filter_map(|p| p.to_str()).collect();
        assert_eq!(
            names,
            [
                ".claude/settings.json",
                ".gitleaks.toml",
                "CLAUDE.md",
                "crates/a/CLAUDE.md"
            ]
        );
        assert_eq!(std::fs::read_to_string(into.join("CLAUDE.md")).unwrap(), "rules");
        let rules = frozen_rules(&into);
        assert!(rules.config.is_some() && rules.ignore.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }
}
