//! The reports of an order: the Markdown files its path scope names one by one. The take-over keeps a copy of each in
//! the project's own folder before the worktree goes, and deleting the order removes it. The copies sit in a folder
//! git ignores through a `.gitignore` of its own, so they never make the working copy dirty or reach a commit.

use std::io;
use std::path::{Component, Path, PathBuf};

use crate::OrderId;

/// The folder of the kept reports within the project folder.
pub fn root_folder(project: &Path) -> PathBuf {
    project.join(ide_product::PROJECT_DIR).join("reports")
}

/// The folder of the kept reports of `order` within the project folder.
pub fn folder(project: &Path, order: &OrderId) -> PathBuf {
    root_folder(project).join(&order.0)
}

/// The Markdown files `scope` names one by one, relative to the worktree; a pattern or a path that leaves the worktree
/// names none.
pub fn named(scope: &[String]) -> Vec<String> {
    scope
        .iter()
        .filter(|path| {
            let inside = Path::new(path.as_str())
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
            inside && !path.contains(['*', '?', '[', '{']) && path.to_ascii_lowercase().ends_with(".md")
        })
        .cloned()
        .collect()
}

/// Copies each of `reports` the order wrote as a plain file in `worktree` into its folder of the project; a link is
/// left behind, as it could point at any file of the Mac. The paths that were kept.
pub fn keep(worktree: &Path, project: &Path, order: &OrderId, reports: &[String]) -> io::Result<Vec<String>> {
    let mut kept = Vec::new();
    for report in named(reports) {
        let from = worktree.join(&report);
        let plain = std::fs::symlink_metadata(&from).is_ok_and(|m| m.file_type().is_file());
        if !plain {
            continue;
        }
        let to = folder(project, order).join(&report);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&from, &to)?;
        kept.push(report);
    }
    if !kept.is_empty() {
        let ignore = root_folder(project).join(".gitignore");
        if !ignore.exists() {
            std::fs::write(ignore, "*\n")?;
        }
    }
    Ok(kept)
}

/// Removes the kept reports of `order`; nothing when it has none.
pub fn remove(project: &Path, order: &OrderId) -> io::Result<()> {
    match std::fs::remove_dir_all(folder(project, order)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ide-factory-reports-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_report_is_a_markdown_file_the_path_scope_names_inside_the_worktree() {
        let scope: Vec<String> = [
            "src/**",
            "docs/review.md",
            "docs/*.md",
            "../outside.md",
            "/etc/notes.md",
            "README.MD",
            "Cargo.lock",
        ]
        .map(String::from)
        .into();
        assert_eq!(named(&scope), ["docs/review.md", "README.MD"]);
    }

    #[test]
    fn the_take_over_keeps_the_written_reports_in_an_ignored_folder_and_deleting_removes_them() {
        let dir = scratch("keep");
        let (worktree, project) = (dir.join("tree"), dir.join("project"));
        std::fs::create_dir_all(worktree.join("docs")).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(worktree.join("docs/review.md"), "# Review\n").unwrap();
        std::fs::write(dir.join("secret"), "key\n").unwrap();
        std::os::unix::fs::symlink(dir.join("secret"), worktree.join("LINK.md")).unwrap();
        let order = OrderId("o-1".into());
        let reports: Vec<String> = ["docs/review.md", "LINK.md", "MISSING.md", "src/**"]
            .map(String::from)
            .into();
        let kept = keep(&worktree, &project, &order, &reports).unwrap();
        assert_eq!(kept, ["docs/review.md"]);
        let copy = folder(&project, &order).join("docs/review.md");
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), "# Review\n");
        assert!(
            !folder(&project, &order).join("LINK.md").exists(),
            "a link is not followed"
        );
        assert_eq!(
            std::fs::read_to_string(root_folder(&project).join(".gitignore")).unwrap(),
            "*\n"
        );
        remove(&project, &order).unwrap();
        assert!(!folder(&project, &order).exists());
        remove(&project, &order).unwrap();
    }

    #[test]
    fn an_order_without_reports_leaves_the_project_folder_alone() {
        let dir = scratch("none");
        let order = OrderId("o-2".into());
        assert!(keep(&dir, &dir, &order, &[]).unwrap().is_empty());
        assert!(!root_folder(&dir).exists());
    }
}
