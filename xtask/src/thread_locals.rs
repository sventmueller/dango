//! The check that keeps thread-locals to those docs/thread-locals.md allows to stay (E97): open objects, running work
//! and app state belong in the app context.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::root;

/// The inventory the check reads.
const INVENTORY: &str = "docs/thread-locals.md";

/// The folders whose Rust files are checked.
const FOLDERS: [&str; 3] = ["src", "crates", "xtask"];

/// A static by the file it is in, relative to the workspace, and its name.
type Static = (String, String);

pub(crate) fn check() -> Result<(), String> {
    let root = root();
    let inventory = std::fs::read_to_string(root.join(INVENTORY)).map_err(|e| format!("{INVENTORY}: {e}"))?;
    let allowed = allowed(&inventory);
    let mut found = BTreeSet::new();
    for folder in FOLDERS {
        for file in rust_files(&root.join(folder)) {
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let relative = file
                .strip_prefix(&root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            for name in statics(&text) {
                found.insert((relative.clone(), name));
            }
        }
    }
    let problems = problems(&allowed, &found);
    if problems.is_empty() {
        println!(
            "{} thread-locals, all listed in {INVENTORY} as allowed to stay",
            found.len()
        );
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// What is wrong between the statics `allowed` to stay and those `found`.
fn problems(allowed: &BTreeSet<Static>, found: &BTreeSet<Static>) -> Vec<String> {
    let new = found.difference(allowed).map(|(file, name)| {
        format!(
            "{file}: thread-local {name} is not listed in {INVENTORY} as allowed to stay; put it into the app context \
             (src/context.rs)"
        )
    });
    let gone = allowed
        .difference(found)
        .map(|(file, name)| format!("{INVENTORY}: {file} {name} is listed as staying but no longer exists"));
    new.chain(gone).collect()
}

/// The statics of the inventory's rows whose target starts with "stays".
fn allowed(inventory: &str) -> BTreeSet<Static> {
    let thread_locals = inventory
        .find(crate::rules::STATICS_HEADING)
        .map_or(inventory, |end| &inventory[..end]);
    thread_locals
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim().strip_prefix('|')?.split('|').map(str::trim).collect();
            let [file, name, _holds, _lives, target, ..] = cells.as_slice() else {
                return None;
            };
            target
                .starts_with("stays")
                .then(|| (unquote(file).to_string(), unquote(name).to_string()))
        })
        .collect()
}

fn unquote(cell: &str) -> &str {
    cell.trim_matches('`')
}

/// The names of the statics declared in the `thread_local!` blocks of `source`.
fn statics(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut depth: Option<i32> = None;
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or_default();
        if depth.is_none() && code.trim_start().starts_with("thread_local!") {
            depth = Some(0);
        }
        let Some(level) = depth.as_mut() else { continue };
        if *level == 1
            && let Some(rest) = code.trim_start().strip_prefix("static ").or_else(|| {
                code.trim_start()
                    .strip_prefix("pub static ")
                    .or_else(|| code.trim_start().strip_prefix("pub(crate) static "))
            })
        {
            names.push(rest.split(':').next().unwrap_or_default().trim().to_string());
        }
        for c in code.chars() {
            match c {
                '{' => *level += 1,
                '}' => *level -= 1,
                _ => {}
            }
        }
        if *level <= 0 && code.contains('}') {
            depth = None;
        }
    }
    names
}

/// The `.rs` files under `folder`, without build output.
pub(crate) fn rust_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
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
    fn the_statics_of_every_block_are_found() {
        let source = "thread_local! {\n    /// A doc.\n    static A: Cell<u8> = const { Cell::new(0) };\n    static B: \
                      RefCell<HashMap<u8, u8>> = RefCell::new(HashMap::new());\n}\n\nstatic C: u8 = 0;\n\n\
                      #[cfg(test)]\nthread_local! {\n    static D: Cell<u8> = const { Cell::new(0) };\n}\n";
        assert_eq!(statics(source), ["A", "B", "D"]);
    }

    #[test]
    fn a_mention_in_a_comment_or_text_is_no_block() {
        let source = "// thread_local! { static A: u8 }\nconst S: &str = \"thread_local! { static B: u8 }\";\n";
        assert!(statics(source).is_empty());
    }

    #[test]
    fn only_rows_that_stay_are_allowed() {
        let inventory = "| File | Static | Holds | Lives | Target |\n|---|---|---|---|---|\n\
                         | `src/a.rs` | `A` | x | app | stays: cache |\n\
                         | `src/b.rs` | `B` | y | app | AppContext: `b` |\n";
        let allowed = allowed(inventory);
        assert_eq!(allowed.len(), 1);
        assert!(allowed.contains(&("src/a.rs".to_string(), "A".to_string())));
    }

    #[test]
    fn a_new_thread_local_and_a_stale_row_both_fail() {
        let allowed = BTreeSet::from([("src/a.rs".to_string(), "A".to_string())]);
        let found = BTreeSet::from([("src/b.rs".to_string(), "B".to_string())]);
        let problems = problems(&allowed, &found);
        assert_eq!(problems.len(), 2);
        assert!(problems[0].contains("src/b.rs: thread-local B"));
        assert!(problems[1].contains("no longer exists"));
    }

    #[test]
    fn the_code_as_it_is_passes() {
        check().unwrap();
    }
}
