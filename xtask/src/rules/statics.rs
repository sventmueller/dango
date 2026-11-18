//! The statics of the app's production code whose type keeps state that changes, which bypass the app context like a
//! thread-local (E97) and so need a row in docs/thread-locals.md: by their type, seen through `type` aliases, or
//! because they are `static mut`.

use std::collections::BTreeSet;

use syn::visit::Visit;
use syn::{Item, Type};

use crate::source::{Build, Cx, Hooks, Index};

/// The types whose values change behind a shared reference.
const STATEFUL: [&str; 14] = [
    "Mutex",
    "ReentrantMutex",
    "RwLock",
    "OnceLock",
    "OnceCell",
    "LazyLock",
    "LazyCell",
    "Lazy",
    "RefCell",
    "Cell",
    "UnsafeCell",
    "Once",
    "Lane",
    "Condvar",
];

/// How many aliases deep a type is followed, which only a cycle reaches.
const DEPTH: usize = 8;

/// The stateful statics found, by file and name; `thread_local!` blocks are left to the thread-locals check.
#[derive(Default)]
pub(super) struct Statics(pub(super) BTreeSet<(String, String)>);

impl Hooks for Statics {
    fn item(&mut self, cx: &Cx, item: &Item) {
        let Item::Static(declared) = item else { return };
        if cx.build() != Build::Production || cx.in_macro("thread_local") {
            return;
        }
        let mutable = matches!(declared.mutability, syn::StaticMutability::Mut(_));
        let place = cx.krate().zip(cx.index());
        if mutable || stateful(place, cx.module(), &declared.ty, 0) {
            self.0.insert((cx.source.relative.clone(), declared.ident.to_string()));
        }
    }
}

/// Whether `ty`, written in `module` of the crate and index `place`, keeps state that changes.
fn stateful(place: Option<(&str, &Index)>, module: &[String], ty: &Type, depth: usize) -> bool {
    let mut paths = Paths(Vec::new());
    paths.visit_type(ty);
    paths.0.iter().any(|names| {
        let direct = names
            .iter()
            .any(|name| name.starts_with("Atomic") || STATEFUL.contains(&name.as_str()));
        direct
            || place.is_some_and(|(krate, index)| {
                let resolved = index.resolve(krate, module, &[], names);
                depth < DEPTH
                    && index
                        .alias(krate, &resolved)
                        .is_some_and(|(module, aliased)| stateful(place, module, aliased, depth + 1))
            })
    })
}

/// The paths of a type, each as its names.
struct Paths(Vec<Vec<String>>);

impl<'ast> Visit<'ast> for Paths {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.0
            .push(path.segments.iter().map(|segment| segment.ident.to_string()).collect());
        syn::visit::visit_path(self, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{snippets, walk};

    fn statics(files: &[(&str, &str)]) -> Vec<String> {
        let mut all = vec![("src/main.rs", "mod a;\nmod b;\n")];
        all.extend_from_slice(files);
        let ws = snippets(&all);
        let mut statics = Statics::default();
        for file in &ws.files {
            walk(file, Some(&ws.index), &mut statics);
        }
        statics
            .0
            .into_iter()
            .map(|(file, name)| format!("{file} {name}"))
            .collect()
    }

    #[test]
    fn statics_with_state_count_unless_built_for_tests() {
        let source = "static A: Mutex<u8> = Mutex::new(0);\nstatic B: &str = \"x\";\npub static C: AtomicU64 = \
                      AtomicU64::new(0);\nthread_local! {\n    static D: RefCell<u8> = RefCell::new(0);\n}\n\
                      #[cfg(feature = \"selftest\")]\nstatic E: Mutex<u8> = Mutex::new(0);\nfn f() {\n    static F: \
                      OnceLock<u8> = OnceLock::new();\n}\n#[cfg(test)]\nmod tests {\n    static G: AtomicU64 = \
                      AtomicU64::new(0);\n}\nstatic H: Lane = Lane::new(\"h\", 1, 1);\nstatic I: Box<dyn FnOnce()> = x();\n\
                      static mut J: u64 = 0;\n";
        assert_eq!(
            statics(&[("src/a.rs", source)]),
            ["src/a.rs A", "src/a.rs C", "src/a.rs F", "src/a.rs H", "src/a.rs J"]
        );
    }

    #[test]
    fn a_static_after_a_test_module_and_one_split_over_lines_count() {
        let source = "#[cfg(test)]\nmod tests {}\npub(crate)\nstatic\nK:\n    std::sync::Mutex<\n        u8,\n    > = \
                      std::sync::Mutex::new(0);\n";
        assert_eq!(statics(&[("src/a.rs", source)]), ["src/a.rs K"]);
    }

    #[test]
    fn a_type_alias_is_seen_through_in_its_own_module() {
        let a = "pub type Shared = std::sync::Arc<Guarded>;\ntype Guarded = Mutex<Vec<u8>>;\ntype Plain = Vec<u8>;\n\
                 static L: Shared = make();\nstatic M: Plain = Vec::new();\n";
        let b = "use crate::a::Shared as Kept;\nstatic N: Option<Kept> = None;\n";
        assert_eq!(
            statics(&[("src/a.rs", a), ("src/b.rs", b)]),
            ["src/a.rs L", "src/b.rs N"]
        );
    }

    #[test]
    fn a_self_test_file_is_left_out_by_its_declaration() {
        let main = "#[cfg(feature = \"selftest\")]\nmod hooks;\n";
        let ws = snippets(&[
            ("src/main.rs", main),
            ("src/hooks.rs", "static P: Mutex<u8> = Mutex::new(0);\n"),
        ]);
        let mut found = Statics::default();
        walk(&ws.files[1], Some(&ws.index), &mut found);
        assert!(found.0.is_empty());
    }
}
