//! The checks that keep the code's shape as E98 wants it: no Rust file over a thousand lines, and models of panels and
//! tabs that know nothing of AppKit.

use std::path::{Path, PathBuf};

use crate::root;

/// The most lines a Rust file may have.
pub(crate) const MAX_LINES: usize = 1000;

pub(crate) fn check() -> Result<(), String> {
    let root = root();
    let mut problems = Vec::new();
    let files = rust_files(&root);
    for file in &files {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let relative = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        problems.extend(file_problems(&relative, &text));
    }
    if problems.is_empty() {
        println!(
            "{} Rust files, none over {MAX_LINES} lines, no model that uses objc2",
            files.len()
        );
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// What is wrong with the file at `relative` whose content is `text`.
fn file_problems(relative: &str, text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let lines = text.lines().count();
    if lines > MAX_LINES {
        problems.push(format!(
            "{relative}: {lines} lines, more than {MAX_LINES}; split it along its seams"
        ));
    }
    if is_model(relative) {
        for (index, line) in text.lines().enumerate() {
            if line.split("//").next().unwrap_or_default().contains("objc2") {
                problems.push(format!(
                    "{relative}:{}: a model uses objc2; AppKit belongs in the panel's view or controller",
                    index + 1
                ));
            }
        }
    }
    problems
}

/// Whether the file at `relative` is a model: a `model.rs` or a file in a `model` folder.
fn is_model(relative: &str) -> bool {
    let mut parts = relative.rsplit('/');
    let name = parts.next().unwrap_or_default();
    name == "model.rs" || parts.any(|folder| folder == "model")
}

/// The `.rs` files of the workspace, without build output and hidden folders.
fn rust_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name != "target" && !name.starts_with('.') {
                files.extend(rust_files(&path));
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_file_fails() {
        let text = "fn a() {}\n".repeat(MAX_LINES + 1);
        assert_eq!(file_problems("src/a.rs", &text).len(), 1);
        assert!(file_problems("src/a.rs", &"\n".repeat(MAX_LINES)).is_empty());
    }

    #[test]
    fn objc2_in_a_model_fails_but_not_in_a_comment_or_elsewhere() {
        let code = "use objc2::rc::Retained;\n";
        assert_eq!(file_problems("src/git_panel/model.rs", code).len(), 1);
        assert_eq!(file_problems("src/agent/model/rows.rs", code).len(), 1);
        assert!(file_problems("src/agent/view.rs", code).is_empty());
        assert!(file_problems("src/git_panel/model.rs", "// no objc2 here\n").is_empty());
    }

    #[test]
    fn the_code_as_it_is_passes() {
        check().unwrap();
    }
}
