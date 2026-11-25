//! Where the `mod` graph puts each file: from the crate roots that cargo's conventions and the manifests name, along
//! `mod` declarations and their `#[path]`, with the `cfg` of every declaration on the way.

use std::collections::{BTreeSet, HashMap};

use syn::Item;

use super::{Build, Place, Source, cfg};

/// A file the graph reaches, and how.
struct Reached {
    file: String,
    build: Build,
    place: Place,
    /// Whether the file's own folder holds its child modules, as for `mod.rs`, a crate root or a `#[path]` file;
    /// otherwise they are in the folder named after it.
    owns_folder: bool,
}

/// Sets `build` and `place` of every file in `files` that a crate root reaches.
pub(super) fn place(files: &mut [Source], manifests: &[(String, String)]) {
    let at: HashMap<String, usize> = files.iter().enumerate().map(|(i, f)| (f.relative.clone(), i)).collect();
    let mut queue: Vec<Reached> = roots(files, manifests)
        .into_iter()
        .map(|(file, build)| Reached {
            place: Place {
                krate: file.clone(),
                module: Vec::new(),
            },
            file,
            build,
            owns_folder: true,
        })
        .collect();
    let mut placed = vec![false; files.len()];
    while let Some(reached) = queue.pop() {
        let Some(&index) = at.get(&reached.file) else {
            continue;
        };
        if placed[index] && files[index].build <= reached.build {
            continue;
        }
        placed[index] = true;
        let source = &mut files[index];
        source.build = reached.build;
        source.place = Some(reached.place.clone());
        let folder = parent(&reached.file);
        let children = if reached.owns_folder {
            folder.clone()
        } else {
            join(&folder, stem(&reached.file))
        };
        let declared = Declared {
            at: &at,
            reached: &reached,
            base: &folder,
        };
        declared.children(
            &source.syntax.items,
            &children,
            true,
            reached.build,
            &reached.place.module,
            &mut queue,
        );
    }
}

/// The `mod` declarations of one file.
struct Declared<'a> {
    at: &'a HashMap<String, usize>,
    reached: &'a Reached,
    /// The folder `#[path]` is relative to outside inline modules: the file's own.
    base: &'a str,
}

impl Declared<'_> {
    fn children(
        &self,
        items: &[Item],
        folder: &str,
        top: bool,
        build: Build,
        module: &[String],
        queue: &mut Vec<Reached>,
    ) {
        for item in items {
            let Item::Mod(declaration) = item else {
                continue;
            };
            let build = build.max(cfg::build_of(&declaration.attrs));
            let name = super::walk::name(&declaration.ident);
            let mut inner = module.to_vec();
            inner.push(name.clone());
            let path = path_attribute(&declaration.attrs);
            let base = if top { self.base } else { folder };
            match &declaration.content {
                Some((_, items)) => {
                    let folder = path.map_or_else(|| join(folder, &name), |path| join(base, &path));
                    self.children(items, &folder, false, build, &inner, queue);
                }
                None => {
                    let (file, owns_folder) = match path {
                        Some(path) => (join(base, &path), true),
                        None => {
                            let flat = join(folder, &format!("{name}.rs"));
                            if self.at.contains_key(&flat) {
                                (flat, false)
                            } else {
                                (join(&join(folder, &name), "mod.rs"), true)
                            }
                        }
                    };
                    queue.push(Reached {
                        file,
                        build,
                        place: Place {
                            krate: self.reached.place.krate.clone(),
                            module: inner,
                        },
                        owns_folder,
                    });
                }
            }
        }
    }
}

fn path_attribute(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| match &attr.meta {
        syn::Meta::NameValue(value) if value.path.is_ident("path") => match &value.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(path),
                ..
            }) => Some(path.value()),
            _ => None,
        },
        _ => None,
    })
}

/// The crate roots among `files` with what they are built into.
fn roots(files: &[Source], manifests: &[(String, String)]) -> Vec<(String, Build)> {
    let mut packages: BTreeSet<String> = manifests.iter().map(|(path, _)| parent(path)).collect();
    for file in files {
        for entry in ["src/lib.rs", "src/main.rs"] {
            if let Some(package) = package_of(&file.relative, entry) {
                packages.insert(package);
            }
        }
    }
    let mut roots = Vec::new();
    for file in files.iter().map(|f| f.relative.as_str()) {
        let (folder, name) = (parent(file), last(file));
        let grand = parent(&folder);
        let package = |dir: &str| packages.contains(dir);
        let product = (last(&folder) == "src" && (name == "lib.rs" || name == "main.rs"))
            || (last(&folder) == "bin" && last(&grand) == "src" && package(&parent(&grand)))
            || (name == "main.rs" && last(&grand) == "bin" && last(&parent(&grand)) == "src")
            || (name == "build.rs" && package(&folder));
        let test = ["tests", "benches", "examples"]
            .iter()
            .any(|kind| (last(&folder) == *kind && package(&grand)) || (name == "main.rs" && last(&grand) == *kind));
        if product {
            roots.push((file.to_string(), Build::Production));
        } else if test {
            roots.push((file.to_string(), Build::Test));
        }
    }
    for (manifest, text) in manifests {
        for (path, build) in declared_targets(text) {
            roots.push((join(&parent(manifest), &path), build));
        }
    }
    roots
}

/// The package folder of `file` if it is the package's `entry`, such as `src/lib.rs`.
fn package_of(file: &str, entry: &str) -> Option<String> {
    if file == entry {
        return Some(String::new());
    }
    file.strip_suffix(entry)?.strip_suffix('/').map(str::to_string)
}

/// The `path`s of a manifest's `[lib]`, `[[bin]]`, `[[test]]`, `[[bench]]` and `[[example]]` sections.
fn declared_targets(manifest: &str) -> Vec<(String, Build)> {
    let mut section = "";
    let mut targets = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line;
            continue;
        }
        let Some(value) = line
            .strip_prefix("path")
            .and_then(|rest| rest.trim_start().strip_prefix('='))
        else {
            continue;
        };
        let path = value.trim().trim_matches('"').to_string();
        match section {
            "[lib]" | "[[bin]]" => targets.push((path, Build::Production)),
            "[[test]]" | "[[bench]]" | "[[example]]" => targets.push((path, Build::Test)),
            _ => {}
        }
    }
    targets
}

fn parent(path: &str) -> String {
    path.rsplit_once('/')
        .map_or_else(String::new, |(parent, _)| parent.to_string())
}

fn last(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn stem(path: &str) -> &str {
    let name = last(path);
    name.strip_suffix(".rs").unwrap_or(name)
}

/// `path` below `folder`, with `.` and `..` taken out.
fn join(folder: &str, path: &str) -> String {
    let mut parts: Vec<&str> = folder.split('/').filter(|part| !part.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_join_without_dots() {
        assert_eq!(join("xtask/src", "coverage_tests.rs"), "xtask/src/coverage_tests.rs");
        assert_eq!(join("src/a", "../b/c.rs"), "src/b/c.rs");
        assert_eq!(join("", "src/main.rs"), "src/main.rs");
        assert_eq!(stem("src/a/b.rs"), "b");
    }

    #[test]
    fn manifests_name_targets_but_not_dependencies() {
        let manifest = "[lib]\npath = \"src/x.rs\"\n[[test]]\npath = \"t/a.rs\"\n[dependencies]\n\
                        y = { path = \"../y\" }\n[dependencies.z]\npath = \"../z\"\n";
        assert_eq!(
            declared_targets(manifest),
            [
                ("src/x.rs".to_string(), Build::Production),
                ("t/a.rs".to_string(), Build::Test)
            ]
        );
    }
}
