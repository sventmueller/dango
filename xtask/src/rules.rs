//! The checks that keep the app's AppKit code to the rules CLAUDE.md names and the compiler cannot see: no state in
//! statics that bypass the app context unless docs/thread-locals.md lists them (E97), state handed along rather than
//! looked up (E143), no leaked objects, system callbacks inside `guarded`, sizes from `style.rs`, no thread whose
//! failure to start goes unnoticed (E142), errors the user reads in their own words (E141), and self-test scenarios
//! that end early only through `give_up` or `skip`. They read the syntax
//! trees of `crate::source`, so comments, strings, imports, renames and the `cfg` of tests and the self-test count as
//! the compiler sees them.

mod callbacks;
mod endings;
mod errors;
mod lookups;
mod statics;
mod threads;

use std::collections::BTreeSet;

use crate::root;
use crate::source::{Hooks, Source, Workspace, walk};
use crate::thread_locals::{INVENTORY, Inventory, STATICS_HEADING, Static, inventory};

/// The self-test's own harness, which is not app state and may block, leak and wait as a test does.
const SELFTEST: &str = "src/selftest";

/// The folders the rule about discarded threads covers.
const WORKSPACE: [&str; 3] = ["src", "crates", "xtask"];

/// How many times the app's production code may look the app context up through `context::app()`, `try_app()`,
/// `AppContext::get` or `AppContext::of_delegate`, or call a function that does, instead of being handed it or a
/// window id (E97, E143); a ratchet: lower it whenever the count drops, it never rises.
const APP_LOOKUPS: usize = 404;

/// How many times the UI crates' production code may look their state up through their host's `state()` or call a
/// function that does, instead of being handed it, a ratchet like `APP_LOOKUPS` (E143).
const STATE_LOOKUPS: usize = 481;

pub(crate) fn check() -> Result<(), String> {
    let root = root();
    let text = std::fs::read_to_string(root.join(INVENTORY)).map_err(|e| format!("{INVENTORY}: {e}"))?;
    let ws = Workspace::load(&root, &WORKSPACE)?;
    let summary = review(&ws, &inventory(&text))?;
    println!("{summary}");
    Ok(())
}

/// What `ws` breaks of the rules, or the summary of what it keeps to.
fn review(ws: &Workspace, inventory: &Inventory) -> Result<String, String> {
    let mut problems = Vec::new();
    let mut found = BTreeSet::new();
    for source in &ws.files {
        problems.extend(hooked(source, ws, threads::Discarded::default()).problems);
        if !in_app(&source.relative) {
            continue;
        }
        let callbacks = hooked(source, ws, callbacks::Callbacks::default());
        problems.extend(callbacks.problems);
        problems.extend(hooked(source, ws, errors::Errors::default()).problems);
        problems.extend(hooked(source, ws, threads::Raw::default()).problems);
        found.extend(hooked(source, ws, statics::Statics::default()).0);
    }
    problems.extend(endings::problems(ws));
    problems.extend(unlisted(&inventory.statics, &found));
    let lookups = lookups::count(ws);
    problems.extend(ratchet(
        lookups.app_total(),
        APP_LOOKUPS,
        "APP_LOOKUPS",
        &format!("lookups of the app context ({})", split(&lookups.app)),
        "hand the context, the window controller or a window id along instead (E97, E143)",
    ));
    problems.extend(ratchet(
        lookups.state_total(),
        STATE_LOOKUPS,
        "STATE_LOOKUPS",
        &format!("lookups of a UI crate's state ({})", split(&lookups.state)),
        "hand the state or the object that holds it along instead (E143)",
    ));
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    Ok(format!(
        "{} Rust files: {} stateful statics, all listed in {INVENTORY}; callbacks guarded, no leaks, sizes from style.rs, no thread that may fail unseen, no scenario that ends early unseen; {} lookups of the app context ({}) and {} of the \
         UI crates' state ({})",
        ws.files.len(),
        found.len(),
        lookups.app_total(),
        split(&lookups.app),
        lookups.state_total(),
        split(&lookups.state),
    ))
}

fn hooked<H: Hooks>(source: &Source, ws: &Workspace, mut hooks: H) -> H {
    walk(source, Some(&ws.index), &mut hooks);
    hooks
}

/// The counts of `kinds` as `name n`, joined.
fn split(kinds: &std::collections::BTreeMap<String, usize>) -> String {
    let parts: Vec<String> = kinds.iter().map(|(kind, n)| format!("{kind} {n}")).collect();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

/// A ratchet held to `limit` exactly: more is a regression, fewer must lower the constant `name` to the count.
fn ratchet(count: usize, limit: usize, name: &str, what: &str, instead: &str) -> Option<String> {
    match count.cmp(&limit) {
        std::cmp::Ordering::Greater => Some(format!("{count} {what}, at most {limit}: {instead}")),
        std::cmp::Ordering::Less => Some(format!(
            "{count} {what}, fewer than {limit}: lower {name} in xtask/src/rules.rs to {count}, so the count cannot \
             rise again"
        )),
        std::cmp::Ordering::Equal => None,
    }
}

/// Whether the file at `relative` belongs to the app's AppKit code that the rules cover.
fn in_app(relative: &str) -> bool {
    let app = relative.starts_with("src/")
        && !relative.starts_with(&format!("{SELFTEST}/"))
        && relative != format!("{SELFTEST}.rs");
    app || relative.starts_with("crates/ide-ui-")
}

/// What is wrong between the statics `allowed` to keep state and those `found`.
fn unlisted(allowed: &BTreeSet<Static>, found: &BTreeSet<Static>) -> Vec<String> {
    let new = found.difference(allowed).map(|(file, name)| {
        format!(
            "{file}: static {name} keeps state outside the app context (src/context.rs) and is not listed under \
             \"{STATICS_HEADING}\" in {INVENTORY}"
        )
    });
    let gone = allowed
        .difference(found)
        .map(|(file, name)| format!("{INVENTORY}: {file} {name} is listed under \"{STATICS_HEADING}\" but is gone"));
    new.chain(gone).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::snippets;

    #[test]
    fn a_ratchet_fails_both_ways_with_the_value_to_put_in() {
        assert_eq!(ratchet(5, 5, "X", "lookups", "pass it"), None);
        assert_eq!(
            ratchet(6, 5, "X", "lookups", "pass it").unwrap(),
            "6 lookups, at most 5: pass it"
        );
        let lower = ratchet(4, 5, "X", "lookups", "pass it").unwrap();
        assert!(lower.contains("lower X in xtask/src/rules.rs to 4"), "{lower}");
    }

    #[test]
    fn every_rule_and_both_ratchets_take_part_in_the_review() {
        let ws = snippets(&[
            ("src/main.rs", "mod context;\nmod a;\n"),
            ("src/context.rs", "pub fn app() -> Rc<AppContext> { todo() }\n"),
            (
                "src/a.rs",
                "static S: Mutex<u8> = Mutex::new(0);\nfn f() {\n    std::mem::forget(x);\n    let b = \
                 RcBlock::new(|| go());\n    let _ = Builder::new().spawn(g);\n    std::thread::spawn(h);\n    \
                 let c = crate::context::app();\n}\nfn e() -> Result<(), String> { Ok(()) }\n",
            ),
            ("crates/ide-ui-x/src/lib.rs", ""),
        ]);
        let problems = review(&ws, &Inventory::default()).unwrap_err();
        for expected in [
            "src/a.rs:3: mem::forget",
            "src/a.rs:4: the body of a block",
            "src/a.rs:5: the result of spawning",
            "src/a.rs:6: a thread started past",
            "src/a.rs:9: an error as a bare String",
            "src/a.rs: static S keeps state",
            "1 lookups of the app context (app 1), fewer than",
            "0 lookups of a UI crate's state (none), fewer than",
        ] {
            assert!(problems.contains(expected), "{expected} missing in\n{problems}");
        }
    }

    #[test]
    fn the_harness_of_the_self_test_is_left_to_itself() {
        assert!(in_app("src/editor/mod.rs") && in_app("crates/ide-ui-git/src/panel.rs"));
        assert!(!in_app("src/selftest.rs") && !in_app("src/selftest/probe.rs") && !in_app("crates/ide-git/src/lib.rs"));
    }

    #[test]
    fn the_app_code_as_it_is_passes() {
        check().unwrap();
    }
}
