//! The names each module of each crate defines and imports, so that a path resolves to what it names: through `use`
//! with renames and globs, `self`, `super` and re-exports, within the crate. Paths into other crates stay as written.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use syn::{Item, UseTree};

use super::Source;
use super::walk::{Cx, Hooks, name, walk};

/// How deep resolving may follow imports before it gives up, which only a cycle of imports reaches.
const DEPTH: usize = 32;

/// What a name means in a module, by crate root, module and name; `None` also while it is being worked out, which
/// ends a cycle of glob imports.
type Lookups = HashMap<(String, Vec<String>, String), Option<Vec<String>>>;

/// The modules of every crate by crate root and module path.
#[derive(Default)]
pub(crate) struct Index {
    modules: HashMap<(String, Vec<String>), Module>,
    /// The `type` aliases of every crate by crate root and their path from `crate`, with the module they are in.
    aliases: HashMap<(String, Vec<String>), (Vec<String>, syn::Type)>,
    /// The names looked up so far, since glob imports would otherwise make every lookup walk the crate again.
    looked_up: RefCell<Lookups>,
}

#[derive(Default)]
struct Module {
    items: BTreeSet<String>,
    imports: Imports,
}

/// What a module's or a block's `use` declarations bring in.
#[derive(Default)]
pub(crate) struct Imports {
    /// The names by the path they import, as written.
    named: BTreeMap<String, Vec<String>>,
    /// The paths of glob imports, as written.
    globs: Vec<Vec<String>>,
}

impl Imports {
    pub(crate) fn add(&mut self, tree: &UseTree) {
        self.add_below(&[], tree);
    }

    fn add_below(&mut self, prefix: &[String], tree: &UseTree) {
        let below = |ident: &syn::Ident| {
            let mut path = prefix.to_vec();
            path.push(name(ident));
            path
        };
        match tree {
            UseTree::Path(path) => self.add_below(&below(&path.ident), &path.tree),
            UseTree::Name(leaf) if leaf.ident == "self" => {
                if let Some(last) = prefix.last() {
                    self.named.insert(last.clone(), prefix.to_vec());
                }
            }
            UseTree::Name(leaf) => {
                self.named.insert(name(&leaf.ident), below(&leaf.ident));
            }
            UseTree::Rename(rename) if rename.rename != "_" => {
                let target = if rename.ident == "self" {
                    prefix.to_vec()
                } else {
                    below(&rename.ident)
                };
                self.named.insert(name(&rename.rename), target);
            }
            UseTree::Rename(_) => {}
            UseTree::Glob(_) => self.globs.push(prefix.to_vec()),
            UseTree::Group(group) => {
                for tree in &group.items {
                    self.add_below(prefix, tree);
                }
            }
        }
    }
}

/// The items and imports of a block in a function, which only the block sees.
pub(crate) struct BlockScope {
    /// The functions declared in the block by their path from `crate`.
    pub(crate) items: BTreeMap<String, Vec<String>>,
    pub(crate) imports: Imports,
}

/// Collects the index while walking the files.
#[derive(Default)]
struct Indexer {
    index: Index,
}

impl Hooks for Indexer {
    fn item(&mut self, cx: &Cx, item: &Item) {
        if cx.function().is_some() {
            return;
        }
        let Some(krate) = cx.krate() else { return };
        let module_path = cx.module().to_vec();
        let module = self
            .index
            .modules
            .entry((krate.to_string(), module_path.clone()))
            .or_default();
        let ident = match item {
            Item::Use(declaration) => {
                module.imports.add(&declaration.tree);
                return;
            }
            Item::Type(alias) => {
                let mut path = vec!["crate".to_string()];
                path.extend(module_path.iter().cloned());
                path.push(name(&alias.ident));
                self.index
                    .aliases
                    .insert((krate.to_string(), path), (module_path.clone(), (*alias.ty).clone()));
                &alias.ident
            }
            Item::Mod(child) => {
                let mut inner = module_path.clone();
                inner.push(name(&child.ident));
                module.items.insert(name(&child.ident));
                self.index.modules.entry((krate.to_string(), inner)).or_default();
                return;
            }
            Item::ExternCrate(declaration) => {
                let alias = declaration
                    .rename
                    .as_ref()
                    .map_or(&declaration.ident, |(_, rename)| rename);
                module.imports.named.insert(name(alias), vec![name(&declaration.ident)]);
                return;
            }
            Item::Fn(item) => &item.sig.ident,
            Item::Struct(item) => &item.ident,
            Item::Enum(item) => &item.ident,
            Item::Union(item) => &item.ident,
            Item::Trait(item) => &item.ident,
            Item::Const(item) => &item.ident,
            Item::Static(item) => &item.ident,
            _ => return,
        };
        module.items.insert(name(ident));
    }
}

impl Index {
    pub(super) fn build(files: &[Source]) -> Index {
        let mut indexer = Indexer::default();
        for source in files {
            if let Some(place) = &source.place {
                indexer
                    .index
                    .modules
                    .entry((place.krate.clone(), place.module.clone()))
                    .or_default();
                walk(source, None, &mut indexer);
            }
        }
        indexer.index
    }

    /// The module and the aliased type of the `type` alias at `path`, from `crate`, of the crate at `krate`.
    pub(crate) fn alias(&self, krate: &str, path: &[String]) -> Option<(&[String], &syn::Type)> {
        self.aliases
            .get(&(krate.to_string(), path.to_vec()))
            .map(|(module, ty)| (module.as_slice(), ty))
    }

    /// What `names` names in `module` of the crate at `krate`, inside `blocks`.
    pub(crate) fn resolve(
        &self,
        krate: &str,
        module: &[String],
        blocks: &[BlockScope],
        names: &[String],
    ) -> Vec<String> {
        self.resolve_in(krate, module, blocks, names, 0)
    }

    fn resolve_in(
        &self,
        krate: &str,
        module: &[String],
        blocks: &[BlockScope],
        names: &[String],
        depth: usize,
    ) -> Vec<String> {
        let Some(first) = names.first() else {
            return Vec::new();
        };
        if depth > DEPTH {
            return names.to_vec();
        }
        let rest = &names[1..];
        let absolute = match first.as_str() {
            "crate" => names.to_vec(),
            "self" => from_crate(module, rest),
            "super" => {
                let ups = names.iter().take_while(|n| *n == "super").count();
                from_crate(&module[..module.len().saturating_sub(ups)], &names[ups..])
            }
            _ => {
                for (at, block) in blocks.iter().enumerate().rev() {
                    if let Some(target) = block.items.get(first) {
                        return joined(target, rest);
                    }
                    if let Some(target) = block.imports.named.get(first) {
                        return self.resolve_in(krate, module, &blocks[..at], &joined(target, rest), depth + 1);
                    }
                }
                match self.lookup(krate, module, first, depth + 1) {
                    Some(target) => joined(&target, rest),
                    None => return names.to_vec(),
                }
            }
        };
        self.canonical(krate, absolute, depth + 1)
    }

    /// The path from `crate` that `name` stands for in `module`, if the module defines or imports it.
    fn lookup(&self, krate: &str, module: &[String], name: &str, depth: usize) -> Option<Vec<String>> {
        if depth > DEPTH {
            return None;
        }
        let key = (krate.to_string(), module.to_vec(), name.to_string());
        if let Some(known) = self.looked_up.borrow().get(&key) {
            return known.clone();
        }
        self.looked_up.borrow_mut().insert(key.clone(), None);
        let found = self.lookup_anew(krate, module, name, depth);
        self.looked_up.borrow_mut().insert(key, found.clone());
        found
    }

    fn lookup_anew(&self, krate: &str, module: &[String], name: &str, depth: usize) -> Option<Vec<String>> {
        let info = self.modules.get(&(krate.to_string(), module.to_vec()))?;
        if info.items.contains(name) {
            return Some(from_crate(module, &[name.to_string()]));
        }
        if let Some(target) = info.imports.named.get(name) {
            if target.len() == 1 && target[0] == name {
                return Some(target.clone());
            }
            return Some(self.resolve_in(krate, module, &[], target, depth + 1));
        }
        info.imports.globs.iter().find_map(|glob| {
            let glob = self.resolve_in(krate, module, &[], glob, depth + 1);
            let inner = glob.strip_prefix(&["crate".to_string()])?;
            self.lookup(krate, inner, name, depth + 1)
        })
    }

    /// `path`, from `crate`, with the re-exports and imports of the modules it passes followed to what they name.
    fn canonical(&self, krate: &str, path: Vec<String>, depth: usize) -> Vec<String> {
        if path.first().is_none_or(|first| first != "crate") {
            return path;
        }
        let mut module = Vec::new();
        let mut at = 1;
        while at < path.len()
            && self
                .modules
                .contains_key(&(krate.to_string(), joined(&module, &path[at..=at])))
        {
            module.push(path[at].clone());
            at += 1;
        }
        let Some(name) = path.get(at) else {
            return path;
        };
        match self.lookup(krate, &module, name, depth + 1) {
            Some(target) if target != path[..=at] => {
                let full = joined(&target, &path[at + 1..]);
                if full.first().is_some_and(|f| f == "crate") && depth < DEPTH {
                    self.canonical(krate, full, depth + 1)
                } else {
                    full
                }
            }
            _ => path,
        }
    }
}

fn from_crate(module: &[String], rest: &[String]) -> Vec<String> {
    std::iter::once("crate".to_string())
        .chain(module.iter().cloned())
        .chain(rest.iter().cloned())
        .collect()
}

fn joined(head: &[String], rest: &[String]) -> Vec<String> {
    head.iter().chain(rest).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::super::snippets;
    use super::*;

    fn resolved(index: &Index, module: &[&str], path: &str) -> String {
        let module: Vec<String> = module.iter().map(ToString::to_string).collect();
        let names: Vec<String> = path.split("::").map(ToString::to_string).collect();
        index.resolve("src/main.rs", &module, &[], &names).join("::")
    }

    #[test]
    fn paths_resolve_through_imports_renames_globs_and_re_exports() {
        let ws = snippets(&[
            (
                "src/main.rs",
                "mod context;\nmod session;\nmod util;\nuse std::thread;\nuse crate::context::app as lookup;\n",
            ),
            ("src/context.rs", "pub fn app() {}\npub struct AppContext;\n"),
            (
                "src/session.rs",
                "use crate::context::{self, app};\nuse super::*;\nmod inner { use super::super::util::*; }\n",
            ),
            (
                "src/util.rs",
                "pub use crate::context::app as reach;\npub fn spawn() {}\n",
            ),
        ]);
        let index = &ws.index;
        assert_eq!(resolved(index, &["session"], "app"), "crate::context::app");
        assert_eq!(
            resolved(index, &["session"], "context::AppContext"),
            "crate::context::AppContext"
        );
        assert_eq!(resolved(index, &[], "lookup"), "crate::context::app");
        assert_eq!(resolved(index, &["session"], "lookup"), "crate::context::app");
        assert_eq!(resolved(index, &["session", "inner"], "reach"), "crate::context::app");
        assert_eq!(resolved(index, &[], "util::reach"), "crate::context::app");
        assert_eq!(resolved(index, &["session", "inner"], "spawn"), "crate::util::spawn");
        assert_eq!(resolved(index, &[], "thread::spawn"), "std::thread::spawn");
        assert_eq!(
            resolved(index, &["session"], "super::util::spawn"),
            "crate::util::spawn"
        );
        assert_eq!(
            resolved(index, &[], "ide_ui_common::util::guarded"),
            "ide_ui_common::util::guarded"
        );
    }
}
