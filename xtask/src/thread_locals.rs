//! The check that keeps thread-locals to those docs/thread-locals.md allows to stay (E97): open objects, running work
//! and app state belong in the app context. The inventory's reader lives here too, for both of its tables.

use std::collections::BTreeSet;

use syn::Item;

use crate::root;
use crate::source::{Cx, Hooks, Workspace, walk};

/// The inventory of thread-locals and stateful statics.
pub(crate) const INVENTORY: &str = "docs/thread-locals.md";

/// The heading in the inventory above the table of statics; the thread-locals' table is above it.
pub(crate) const STATICS_HEADING: &str = "## Statics";

/// The folders whose Rust files are checked.
const FOLDERS: [&str; 3] = ["src", "crates", "xtask"];

/// A static by the file it is in, relative to the workspace, and its name.
pub(crate) type Static = (String, String);

/// What the inventory allows.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Inventory {
    /// The thread-locals whose target starts with "stays".
    pub(crate) thread_locals: BTreeSet<Static>,
    /// The statics below `STATICS_HEADING` that give a reason.
    pub(crate) statics: BTreeSet<Static>,
}

/// The rows of both tables of the inventory `text`.
pub(crate) fn inventory(text: &str) -> Inventory {
    let (thread_locals, statics) = text.split_once(STATICS_HEADING).unwrap_or((text, ""));
    Inventory {
        thread_locals: rows(thread_locals)
            .filter(|cells| cells.get(4).is_some_and(|target| target.starts_with("stays")))
            .filter_map(|cells| file_and_name(&cells))
            .collect(),
        statics: rows(statics)
            .filter(|cells| cells.get(3).is_some_and(|reason| !reason.is_empty()))
            .filter_map(|cells| file_and_name(&cells))
            .collect(),
    }
}

/// The cells of the table rows in `text`.
fn rows(text: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    text.lines().filter_map(|line| {
        let cells = line.trim().strip_prefix('|')?.strip_suffix('|')?;
        Some(cells.split('|').map(|cell| cell.trim().to_string()).collect())
    })
}

/// The file and static of a row, if its first cell names a Rust file.
fn file_and_name(cells: &[String]) -> Option<Static> {
    let [file, name, ..] = cells else { return None };
    let file = file.trim_matches('`');
    file.ends_with(".rs")
        .then(|| (file.to_string(), name.trim_matches('`').to_string()))
}

pub(crate) fn check() -> Result<(), String> {
    let root = root();
    let text = std::fs::read_to_string(root.join(INVENTORY)).map_err(|e| format!("{INVENTORY}: {e}"))?;
    let ws = Workspace::load(&root, &FOLDERS)?;
    let found = thread_locals(&ws);
    let problems = problems(&inventory(&text).thread_locals, &found);
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

/// The statics of the `thread_local!` blocks of `ws`, tests and self-test included.
fn thread_locals(ws: &Workspace) -> BTreeSet<Static> {
    let mut found = Found::default();
    for source in &ws.files {
        walk(source, None, &mut found);
    }
    found.0
}

#[derive(Default)]
struct Found(BTreeSet<Static>);

impl Hooks for Found {
    fn item(&mut self, cx: &Cx, item: &Item) {
        if let Item::Static(declared) = item
            && cx.in_macro("thread_local")
        {
            self.0.insert((cx.source.relative.clone(), declared.ident.to_string()));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::snippets;

    fn found(code: &str) -> Vec<String> {
        thread_locals(&snippets(&[("src/a.rs", code)]))
            .into_iter()
            .map(|(_, name)| name)
            .collect()
    }

    #[test]
    fn the_statics_of_every_block_are_found() {
        let source = "thread_local! {\n    /// A doc.\n    static A: Cell<u8> = const { Cell::new(0) };\n    static B: \
                      RefCell<HashMap<u8, u8>> = RefCell::new(HashMap::new());\n}\n\nstatic C: u8 = 0;\n\n\
                      #[cfg(test)]\nthread_local! {\n    static D: Cell<u8> = const { Cell::new(0) };\n}\n\
                      fn f() {\n    std::thread_local! { pub(crate) static E: Cell<u8> = const { Cell::new(0) } }\n}\n\
                      thread_local!(static\n    F: Cell<u8> = const { Cell::new(0) });\n";
        assert_eq!(found(source), ["A", "B", "D", "E", "F"]);
    }

    #[test]
    fn a_mention_in_a_comment_or_text_is_no_block() {
        let source = "// thread_local! { static A: u8 }\nconst S: &str = \"thread_local! { static B: u8 }\";\n\
                      /* thread_local! { static C: u8 = 0; } */\n";
        assert!(found(source).is_empty());
    }

    #[test]
    fn only_rows_that_stay_are_allowed() {
        let text = "| File | Static | Holds | Lives | Target |\n|---|---|---|---|---|\n\
                    | `src/a.rs` | `A` | x | app | stays: cache |\n\
                    | `src/b.rs` | `B` | y | app | AppContext: `b` |\n";
        let allowed = inventory(text).thread_locals;
        assert_eq!(allowed, BTreeSet::from([("src/a.rs".to_string(), "A".to_string())]));
    }

    #[test]
    fn the_inventory_lists_statics_only_below_their_heading() {
        let text = "| File | Static | Holds | Lives | Target |\n|---|---|---|---|---|\n\
                    | `src/a.rs` | `A` | x | app | stays: cache |\n\n## Statics\n\n\
                    | File | Static | Holds | Reason |\n|---|---|---|---|\n| `src/b.rs` | `B` | y | serial |\n\
                    | `src/c.rs` | `C` | z |  |\n";
        let read = inventory(text);
        assert_eq!(
            read.statics,
            BTreeSet::from([("src/b.rs".to_string(), "B".to_string())])
        );
        assert_eq!(
            read.thread_locals,
            BTreeSet::from([("src/a.rs".to_string(), "A".to_string())])
        );
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
