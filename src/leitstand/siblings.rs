//! Path dependencies beside the repository (E188, R13): an order's worktree lives in the app's data folder, so a
//! `../splat` of the repository's build files resolves nowhere there. Beside the worktree go links to the folders
//! they name beside the working copy, as Dango's own repository has `../splat` and `../lowspeak`, and those folders
//! are what the gate sandbox and the sessions may read besides the worktree. Everything here blocks.

use std::io;
use std::path::{Component, Path, PathBuf};

use super::manifest;

/// The folders beside `root` its path dependencies lie in, one per first part of `../part/…`, as `(part, folder)`
/// with the folder's links resolved; a dependency that leads further up than beside `root` has no place beside a
/// worktree and is left out.
fn beside(root: &Path) -> Vec<(String, PathBuf)> {
    let Some(parent) = root.parent() else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = manifest::dependencies(root)
        .iter()
        .filter_map(|path| {
            let mut parts = Path::new(path).components();
            if parts.next() != Some(Component::ParentDir) {
                return None;
            }
            match parts.next() {
                Some(Component::Normal(name)) => {
                    let name = name.to_string_lossy().into_owned();
                    let folder = parent.join(&name);
                    Some((name, folder.canonicalize().unwrap_or(folder)))
                }
                _ => None,
            }
        })
        .collect();
    found.sort();
    found.dedup();
    found
}

/// The folders beside `root` that its path dependencies lie in and that exist.
pub(super) fn targets(root: &Path) -> Vec<PathBuf> {
    beside(root)
        .into_iter()
        .map(|(_, folder)| folder)
        .filter(|folder| folder.is_dir())
        .collect()
}

/// The links beside `worktree` to the folders beside `root` that exist; a sandbox that hides their folder refuses a
/// path through them unless the link itself is readable, even with the folder it leads to readable.
pub(super) fn links(root: &Path, worktree: &Path) -> Vec<PathBuf> {
    let Some(holder) = worktree.parent() else {
        return Vec::new();
    };
    beside(root)
        .into_iter()
        .filter(|(_, folder)| folder.is_dir())
        .map(|(name, _)| holder.join(name))
        .collect()
}

/// Links each folder beside `root` its path dependencies need into the folder that holds `worktree`; one there
/// already counts when it leads to the same folder. What could not be linked, by name.
pub(super) fn link(root: &Path, worktree: &Path) -> Vec<String> {
    let Some(holder) = worktree.parent() else {
        return Vec::new();
    };
    let mut failed = Vec::new();
    for (name, folder) in beside(root) {
        if !folder.is_dir() {
            continue;
        }
        let link = holder.join(&name);
        match link_one(&link, &folder) {
            Ok(()) => {}
            Err(error) => {
                ide_core::diag::report("linking a path dependency beside an order's worktree", &error);
                failed.push(name);
            }
        }
    }
    failed
}

fn link_one(link: &Path, folder: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            let points = std::fs::read_link(link)?;
            if points == folder || link.canonicalize().is_ok_and(|real| real == folder) {
                return Ok(());
            }
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} leads to {}", link.display(), points.display()),
            ))
        }
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is taken", link.display()),
        )),
        Err(_) => std::os::unix::fs::symlink(folder, link),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dependencies_beside_the_repository_are_linked_beside_the_worktree() {
        let base = std::env::temp_dir().join(format!("leitstand-siblings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("repos/dango");
        std::fs::create_dir_all(root.join("crates/a")).unwrap();
        std::fs::create_dir_all(base.join("repos/splat/crates/x")).unwrap();
        std::fs::create_dir_all(base.join("repos/lowspeak")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace.dependencies]\nsplat = { path = \"../splat/crates/x\" }\ngone = { path = \"../gone\" }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("crates/a/Cargo.toml"),
            "[dependencies]\nlow = { path = \"../../../lowspeak\" }\n",
        )
        .unwrap();
        let worktree = base.join("data/worktrees/key/o-1");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(base.join("data/worktrees/key/thread-x")).unwrap();
        let real = |p: &str| base.join(p).canonicalize().unwrap();
        assert_eq!(targets(&root), [real("repos/lowspeak"), real("repos/splat")]);
        let holder = base.join("data/worktrees/key");
        assert_eq!(links(&root, &worktree), [holder.join("lowspeak"), holder.join("splat")]);
        assert!(link(&root, &worktree).is_empty());
        assert!(link(&root, &worktree).is_empty(), "linking again finds its own links");
        assert!(worktree.join("../splat/crates/x").is_dir());
        assert!(worktree.join("../lowspeak").is_dir());
        std::fs::remove_file(base.join("data/worktrees/key/splat")).unwrap();
        std::fs::create_dir_all(base.join("data/worktrees/key/splat")).unwrap();
        assert_eq!(
            link(&root, &worktree),
            ["splat"],
            "a folder of that name is not replaced"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
