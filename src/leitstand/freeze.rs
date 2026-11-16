//! Freezing an order's agent and scanner configuration (E172, C52, C57): the files of the fixed group are copied from
//! the user's working copy, never the worktree, into a folder of the order's outside anything its builder may write,
//! and the secrets scan reads its gitleaks rules from there. Copies are clones (`clonefile`), so they cost no space
//! until one side changes. Everything here blocks.

use std::io;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ide_factory::workflow::rules::copies_along;

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

/// The patterns as one set, matching without regard to case as the Mac's volumes do; a pattern that does not parse
/// matches nothing.
fn set(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = GlobBuilder::new(pattern).case_insensitive(true).build() {
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

/// Clones the path `listed` of the worktree list `copy` from the working copy `root` into the worktree `into`, what is
/// there already kept. A folder that holds files with credentials goes file by file without them, unless `copy` names
/// them on their own, which needs their own confirmation (C50).
pub(super) fn copy_listed(root: &Path, into: &Path, listed: &str, copy: &[String]) -> io::Result<()> {
    let (from, target) = (root.join(listed), into.join(listed));
    if from.is_dir() && (target.exists() || leaves_out(root, Path::new(listed), listed, copy)) {
        return copy_folder(root, into, Path::new(listed), listed, copy);
    }
    if target.exists() {
        return Ok(());
    }
    clone(&from, &target)
}

/// Whether the folder `relative` of `root` holds a file that may not go along with `listed`.
fn leaves_out(root: &Path, relative: &Path, listed: &str, copy: &[String]) -> bool {
    let Ok(entries) = std::fs::read_dir(root.join(relative)) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = relative.join(entry.file_name());
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            leaves_out(root, &path, listed, copy)
        } else {
            !copies_along(copy, listed, &path.to_string_lossy())
        }
    })
}

fn copy_folder(root: &Path, into: &Path, relative: &Path, listed: &str, copy: &[String]) -> io::Result<()> {
    std::fs::create_dir_all(into.join(relative))?;
    for entry in std::fs::read_dir(root.join(relative))?.flatten() {
        let path = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_folder(root, into, &path, listed, copy)?;
        } else if copies_along(copy, listed, &path.to_string_lossy()) && !into.join(&path).exists() {
            clone(&root.join(&path), &into.join(&path))?;
        }
    }
    Ok(())
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
            ("crates/b/Claude.md", "rules in another case"),
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
                "crates/a/CLAUDE.md",
                "crates/b/Claude.md"
            ]
        );
        assert_eq!(std::fs::read_to_string(into.join("CLAUDE.md")).unwrap(), "rules");
        let rules = frozen_rules(&into);
        assert!(rules.config.is_some() && rules.ignore.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    fn files(root: &Path) -> Vec<String> {
        matching(root, &["**".to_string()])
            .iter()
            .filter_map(|p| p.to_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn a_listed_folder_goes_without_credentials_that_are_not_listed_on_their_own() {
        let base = std::env::temp_dir().join(format!("leitstand-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("repo");
        for path in [
            "config/app.toml",
            "config/.env",
            "config/secrets/token",
            "plain/readme.md",
            "keys/id_ed25519",
            "secrets/id_ed25519",
        ] {
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::write(root.join(path), path).unwrap();
        }
        let copy = |list: &[&str], into: &str| {
            let list: Vec<String> = list.iter().map(|p| p.to_string()).collect();
            let into = base.join(into);
            for listed in &list {
                copy_listed(&root, &into, listed, &list).unwrap();
            }
            files(&into)
        };
        assert_eq!(copy(&["config", "plain"], "a"), ["config/app.toml", "plain/readme.md"]);
        assert_eq!(
            copy(&["config/.env", "config"], "b"),
            ["config/.env", "config/app.toml"],
            "a file named on its own goes along, whatever the order"
        );
        assert!(copy(&["keys"], "c").is_empty(), "a key goes only when named on its own");
        assert_eq!(
            copy(&["secrets"], "d"),
            ["secrets/id_ed25519"],
            "a folder named like credentials was confirmed as a whole"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
