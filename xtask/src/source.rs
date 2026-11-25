//! The workspace's Rust sources parsed once with `syn`, the one layer under the static checks (`rules`, `structure`,
//! `thread-locals`) and the file list of `coverage`: each file with its syntax tree, the crate and module its `mod`
//! graph puts it in, and whether it is built into the product, only into the self-test or only into tests.

mod cfg;
mod index;
mod modules;
mod walk;

use std::path::{Path, PathBuf};

pub(crate) use cfg::Build;
pub(crate) use index::Index;
pub(crate) use walk::{Cx, Hooks, line, walk};

/// A parsed Rust file.
pub(crate) struct Source {
    /// The path from the workspace root, with `/`.
    pub(crate) relative: String,
    pub(crate) text: String,
    pub(crate) syntax: syn::File,
    /// What the file as a whole is built into; an item inside may be built into less.
    pub(crate) build: Build,
    /// Where the `mod` graph reaches the file; `None` for a file that no crate root reaches.
    pub(crate) place: Option<Place>,
}

/// The crate and module of a file.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Place {
    /// The crate's root file, relative like `Source::relative`.
    pub(crate) krate: String,
    pub(crate) module: Vec<String>,
}

/// The parsed files of some folders of the workspace and the names their modules define.
pub(crate) struct Workspace {
    pub(crate) files: Vec<Source>,
    pub(crate) index: Index,
}

impl Workspace {
    /// Every Rust file below the `folders` of `root` (`""` for all of it), parsed and placed in its crate.
    pub(crate) fn load(root: &Path, folders: &[&str]) -> Result<Workspace, String> {
        let mut files = Vec::new();
        let mut manifests = vec![(
            "Cargo.toml".to_string(),
            read(&root.join("Cargo.toml")).unwrap_or_default(),
        )];
        for folder in folders {
            for path in rust_files(&root.join(folder)) {
                files.push((relative(root, &path), read(&path)?));
            }
            for path in walk_files(&root.join(folder), &|path| path.ends_with("Cargo.toml")) {
                let manifest = relative(root, &path);
                if manifest != "Cargo.toml" {
                    manifests.push((manifest, read(&path)?));
                }
            }
        }
        Workspace::parse(files, &manifests)
    }

    /// The workspace of `files`, each its path from the workspace root and its content, with the crates that cargo's
    /// conventions and the `manifests` (path and content) give them.
    pub(crate) fn parse(files: Vec<(String, String)>, manifests: &[(String, String)]) -> Result<Workspace, String> {
        let mut parsed = Vec::new();
        for (relative, text) in files {
            let syntax = syn::parse_file(&text).map_err(|e| {
                let at = e.span().start();
                format!(
                    "{relative}:{}:{}: not Rust that syn can read: {e}",
                    at.line,
                    at.column + 1
                )
            })?;
            parsed.push(Source {
                relative,
                text,
                syntax,
                build: Build::Production,
                place: None,
            });
        }
        modules::place(&mut parsed, manifests);
        let index = Index::build(&parsed);
        Ok(Workspace { files: parsed, index })
    }

    /// The file at `relative`.
    #[cfg(test)]
    pub(crate) fn file(&self, relative: &str) -> Option<&Source> {
        self.files.iter().find(|file| file.relative == relative)
    }
}

/// A workspace of snippets for the checks' tests, each a path and its code.
#[cfg(test)]
pub(crate) fn snippets(files: &[(&str, &str)]) -> Workspace {
    let files = files
        .iter()
        .map(|(path, code)| ((*path).to_string(), (*code).to_string()))
        .collect();
    Workspace::parse(files, &[]).unwrap()
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The path of `file` from `root`, with `/`.
pub(crate) fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The `.rs` files below `folder`, sorted, without build output, copied crates and hidden folders.
pub(crate) fn rust_files(folder: &Path) -> Vec<PathBuf> {
    walk_files(folder, &|path| path.extension().is_some_and(|e| e == "rs"))
}

fn walk_files(folder: &Path, keep: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return files;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            // `vendor` holds copies of other projects' crates, patched in (E14, E104), which keep their own shape.
            if name != "target" && name != "vendor" && !name.starts_with('.') {
                files.extend(walk_files(&path, keep));
            }
        } else if keep(&path) {
            files.push(path);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mod_graph_places_files_and_decides_what_they_are_built_into() {
        let ws = snippets(&[
            (
                "crates/ide-ui-x/src/lib.rs",
                "mod a;\n#[cfg(feature = \"selftest\")]\nmod a_probe;\n#[cfg(test)]\n#[path = \"a_tests.rs\"]\nmod \
                 run_tests;\nmod b { pub mod c; }\n#[cfg(any(test, feature = \"selftest\"))]\nmod d;\n\
                 #[cfg(not(test))]\nmod e;\n",
            ),
            ("crates/ide-ui-x/src/a.rs", "mod inner;\n"),
            ("crates/ide-ui-x/src/a/inner.rs", ""),
            ("crates/ide-ui-x/src/a_probe.rs", ""),
            ("crates/ide-ui-x/src/a_tests.rs", ""),
            ("crates/ide-ui-x/src/b/c.rs", ""),
            ("crates/ide-ui-x/src/d.rs", ""),
            ("crates/ide-ui-x/src/e.rs", ""),
            ("crates/ide-ui-x/src/orphan.rs", ""),
            ("crates/ide-ui-x/tests/it.rs", "mod support;\n"),
            ("crates/ide-ui-x/tests/support/mod.rs", ""),
        ]);
        let built = |path: &str| ws.file(path).unwrap().build;
        assert_eq!(built("crates/ide-ui-x/src/a/inner.rs"), Build::Production);
        assert_eq!(built("crates/ide-ui-x/src/a_probe.rs"), Build::SelfTest);
        assert_eq!(built("crates/ide-ui-x/src/a_tests.rs"), Build::Test);
        assert_eq!(built("crates/ide-ui-x/src/b/c.rs"), Build::Production);
        assert_eq!(built("crates/ide-ui-x/src/d.rs"), Build::SelfTest);
        assert_eq!(built("crates/ide-ui-x/src/e.rs"), Build::Production);
        assert_eq!(built("crates/ide-ui-x/tests/support/mod.rs"), Build::Test);
        assert_eq!(built("crates/ide-ui-x/src/orphan.rs"), Build::Production);
        let place = ws.file("crates/ide-ui-x/src/b/c.rs").unwrap().place.clone().unwrap();
        assert_eq!(place.krate, "crates/ide-ui-x/src/lib.rs");
        assert_eq!(place.module, ["b", "c"]);
        assert!(ws.file("crates/ide-ui-x/src/orphan.rs").unwrap().place.is_none());
    }

    #[test]
    fn a_file_that_is_no_rust_fails_with_its_place() {
        let refused = Workspace::parse(vec![("src/a.rs".into(), "fn a( {".into())], &[])
            .err()
            .unwrap();
        assert!(refused.starts_with("src/a.rs:1:"), "{refused}");
    }

    #[test]
    fn a_manifest_names_crate_roots_beside_the_conventions() {
        let manifest = "[package]\nname = \"x\"\n\n[[bin]]\nname = \"fake\"\npath = \"tests/support/fake.rs\"\n\n\
                        [dependencies]\ny = { path = \"../y\" }\n";
        let files = vec![
            ("crates/x/tests/support/fake.rs".to_string(), "mod part;\n".to_string()),
            ("crates/x/tests/support/part.rs".to_string(), String::new()),
        ];
        let ws = Workspace::parse(files, &[("crates/x/Cargo.toml".to_string(), manifest.to_string())]).unwrap();
        let part = ws.file("crates/x/tests/support/part.rs").unwrap();
        assert_eq!(part.build, Build::Production);
        assert_eq!(part.place.as_ref().unwrap().krate, "crates/x/tests/support/fake.rs");
    }
}
