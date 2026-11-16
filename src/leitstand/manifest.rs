//! The manifest of an order's inputs outside its tree (C69): the files copied into its worktree with a hash each, and
//! the state of path dependencies outside the repository, such as `../splat` beside Dango's own repository (R13).
//! Any change to one makes the evidence invalid. Everything here blocks.

use std::path::{Component, Path, PathBuf};

use ide_factory::evidence::{Manifest, ManifestEntry};
use sha2::{Digest, Sha256};

/// The hash of what `path` holds: a file's bytes, or for a folder the names, sizes and times of what it holds,
/// which is enough to see a change of `node_modules` without reading it whole.
pub(super) fn hash(path: &Path) -> String {
    let mut hasher = Sha256::new();
    feed(path, Path::new(""), &mut hasher);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn feed(base: &Path, relative: &Path, hasher: &mut Sha256) {
    let path = base.join(relative);
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        hasher.update(b"missing");
        return;
    };
    if meta.is_dir() {
        let Ok(entries) = std::fs::read_dir(&path) else { return };
        let mut names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
        names.sort();
        for name in names {
            feed(base, &relative.join(name), hasher);
        }
    } else if relative.as_os_str().is_empty() {
        match std::fs::read(&path) {
            Ok(bytes) => hasher.update(&bytes),
            Err(_) => hasher.update(b"unreadable"),
        }
    } else {
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update(meta.len().to_le_bytes());
        hasher.update(modified.to_le_bytes());
    }
}

/// How many bytes the files below `folder` take.
pub(super) fn size(folder: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

/// The path dependencies outside `root` its build files name: `path = "../…"` in a `Cargo.toml` of the root or one
/// level below, and `file:../…` in its `package.json`.
pub(super) fn dependencies(root: &Path) -> Vec<String> {
    let mut manifests = vec![root.join("Cargo.toml")];
    for parent in ["crates", "packages"] {
        if let Ok(entries) = std::fs::read_dir(root.join(parent)) {
            manifests.extend(entries.flatten().map(|e| e.path().join("Cargo.toml")));
        }
    }
    let cargo = regex::Regex::new(r#"path\s*=\s*"([^"]+)""#).ok();
    let mut found = Vec::new();
    for manifest in manifests {
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let folder = manifest.parent().unwrap_or(root);
        for captures in cargo.iter().flat_map(|re| re.captures_iter(&text)) {
            found.extend(outside(root, folder, &captures[1]));
        }
    }
    let npm = regex::Regex::new(r#""file:([^"]+)""#).ok();
    if let Ok(text) = std::fs::read_to_string(root.join("package.json")) {
        for captures in npm.iter().flat_map(|re| re.captures_iter(&text)) {
            found.extend(outside(root, root, &captures[1]));
        }
    }
    found.sort();
    found.dedup();
    found
}

/// `path`, written in `folder`, relative to `root` when it leads out of it.
fn outside(root: &Path, folder: &Path, path: &str) -> Option<String> {
    let mut resolved = PathBuf::new();
    for component in folder.join(path).components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            other => resolved.push(other),
        }
    }
    if resolved.starts_with(root) {
        return None;
    }
    let parent = root.parent()?;
    let below = resolved.strip_prefix(parent).ok()?;
    Some(format!("../{}", below.to_string_lossy()))
}

/// A dependency's state: its commit and a hash of its uncommitted changes, or of its files outside git.
fn state(git: Option<&ide_git::Git>, folder: &Path) -> String {
    let repo = git.and_then(|git| git.discover(folder).ok());
    match repo {
        Some(repo) => {
            let head = repo.head().ok().flatten().map(|c| c.to_string()).unwrap_or_default();
            let changes = repo
                .diff_since("HEAD", &[] as &[PathBuf])
                .map(|diffs| {
                    let mut hasher = Sha256::new();
                    for diff in diffs {
                        hasher.update(diff.path().to_string_lossy().as_bytes());
                        for line in diff.hunks.iter().flat_map(|h| &h.lines) {
                            hasher.update(&line.content);
                        }
                    }
                    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>()
                })
                .unwrap_or_default();
            format!("{head} {changes}")
        }
        None => hash(folder),
    }
}

/// The manifest of the order: `copied` hashed in the working copy at `root`, and the path dependencies' states.
pub(super) fn read(root: &Path, copied: &[String], git: Option<&ide_git::Git>) -> Manifest {
    let mut entries: Vec<ManifestEntry> = copied
        .iter()
        .map(|path| ManifestEntry::Copied {
            path: path.clone(),
            hash: hash(&root.join(path)),
        })
        .collect();
    for path in dependencies(root) {
        let folder = root.join(&path);
        entries.push(ManifestEntry::Dependency {
            state: state(git, &folder),
            path,
        });
    }
    Manifest::new(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_of_a_copied_file_or_folder_changes_its_hash() {
        let base = std::env::temp_dir().join(format!("leitstand-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("modules/a")).unwrap();
        std::fs::write(base.join(".env.local"), "A=1").unwrap();
        std::fs::write(base.join("modules/a/index.js"), "x").unwrap();
        let (file, folder) = (hash(&base.join(".env.local")), hash(&base.join("modules")));
        std::fs::write(base.join(".env.local"), "A=2").unwrap();
        std::fs::write(base.join("modules/a/more.js"), "y").unwrap();
        assert_ne!(hash(&base.join(".env.local")), file);
        assert_ne!(hash(&base.join("modules")), folder);
        assert!(size(&base) >= 5);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn path_dependencies_outside_the_repository_are_found() {
        let base = std::env::temp_dir().join(format!("leitstand-deps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("dango");
        std::fs::create_dir_all(root.join("crates/a")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace.dependencies]\nsplat = { path = \"../splat/crates/x\" }\nown = { path = \"crates/a\" }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates/a/Cargo.toml"),
            "[dependencies]\nlow = { path = \"../../../lowspeak\" }\n",
        )
        .unwrap();
        assert_eq!(dependencies(&root), ["../lowspeak", "../splat/crates/x"]);
        let _ = std::fs::remove_dir_all(&base);
    }
}
