//! What a search for symbols across a project decides without a server: which servers it needs, and how well a
//! name matches the query.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{CustomServer, Language};

/// Loose TypeScript files a search opens at most on one server.
const LOOSE_SEEDS: usize = 50;

/// Folders inside a project that hold other people's code.
const DEPENDENCY_FOLDERS: &[&str] = &["node_modules", "vendor", ".venv", "venv", "site-packages", "Pods"];

/// A server a symbol search needs, and the files it opens on it first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// The language the server serves.
    pub language: Language,
    /// The project root it runs for.
    pub root: PathBuf,
    /// Files with their language id; TypeScript's server searches only the projects of its open files.
    pub seeds: Vec<(PathBuf, String)>,
}

/// The servers a symbol search needs in `folder`: one per language that has files there that ignore rules do not hide
/// and whose servers search symbols, and per project root of those files.
pub fn survey(folder: &Path, custom: &[Arc<CustomServer>]) -> Vec<Planned> {
    let Ok(index) = ide_project::FileIndex::build(folder, &ide_project::Cancel::new()) else {
        return vec![];
    };
    let mut roots: HashMap<(Language, PathBuf), PathBuf> = HashMap::new();
    let mut files: HashMap<(Language, PathBuf), Vec<(PathBuf, String)>> = HashMap::new();
    for file in index.files() {
        let path = folder.join(file);
        let Some((language, id)) = Language::detect(&path, custom) else {
            continue;
        };
        if !language.searches_project() {
            continue;
        }
        let directory = path.parent().unwrap_or(folder).to_path_buf();
        let root = roots
            .entry((language.clone(), directory))
            .or_insert_with_key(|(language, directory)| crate::root(language, directory, folder))
            .clone();
        let seeds = files.entry((language.clone(), root)).or_default();
        if language == Language::TypeScript {
            seeds.push((path, id));
        }
    }
    let mut plan: Vec<Planned> = files
        .into_iter()
        .map(|((language, root), mut seeds)| {
            seeds.sort();
            let configured = ["tsconfig.json", "jsconfig.json"]
                .iter()
                .any(|name| root.join(name).is_file());
            seeds.truncate(if configured { 1 } else { LOOSE_SEEDS });
            Planned { language, root, seeds }
        })
        .collect();
    plan.sort_by(|a, b| (a.language.name(), &a.root).cmp(&(b.language.name(), &b.root)));
    plan
}

/// Whether a symbol at `path` belongs to other people's code rather than the project in `folder`.
pub fn is_dependency(path: &Path, folder: &Path) -> bool {
    let Ok(inside) = path.strip_prefix(folder) else {
        return true;
    };
    inside
        .components()
        .any(|part| DEPENDENCY_FOLDERS.iter().any(|name| part.as_os_str() == *name))
}

/// How well `name` matches `query`: the exact name first, then without case, then names that start with it, then
/// those that contain it, then whatever else the server found.
pub fn rank(name: &str, query: &str, lowered: &str) -> u8 {
    let name = name.strip_suffix("()").unwrap_or(name);
    let name_lowered = name.to_lowercase();
    if name == query {
        0
    } else if name_lowered == lowered {
        1
    } else if name_lowered.starts_with(lowered) {
        2
    } else if name_lowered.contains(lowered) {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_folder_with_three_languages_gets_a_server_per_language_and_root() {
        let scratch = ide_core::testing::scratch("symbol-survey");
        let folder = scratch.path().to_path_buf();
        write(&folder, "Cargo.toml", "[package]\nname = \"a\"\n");
        write(&folder, "src/main.rs", "fn main() {}\n");
        write(&folder, "src/deep/more.rs", "fn more() {}\n");
        write(&folder, "go/go.mod", "module example.com/go\n");
        write(&folder, "go/cmd/main.go", "package main\n");
        write(&folder, "web/package.json", "{}\n");
        write(&folder, "web/src/index.ts", "export {};\n");
        write(&folder, ".gitignore", "build/\n");
        write(&folder, "build/generated.py", "x = 1\n");
        let plan: Vec<(Language, PathBuf)> = survey(&folder, &[])
            .into_iter()
            .map(|planned| (planned.language, planned.root))
            .collect();
        assert_eq!(
            plan,
            vec![
                (Language::Go, folder.join("go")),
                (Language::Rust, folder.clone()),
                (Language::TypeScript, folder.join("web")),
            ],
            "no Python from the ignored folder, no servers for TOML and JSON"
        );
    }

    #[test]
    fn exact_names_come_before_prefixes_and_other_matches() {
        let mut names = vec!["parse_all", "Parse", "reparse", "parse", "other"];
        names.sort_by_key(|n| (rank(n, "parse", "parse"), n.len()));
        assert_eq!(names, ["parse", "Parse", "parse_all", "reparse", "other"]);
    }

    #[test]
    fn a_function_name_with_parentheses_matches_exactly() {
        assert_eq!(rank("twice()", "twice", "twice"), 0);
        assert_eq!(rank("twiceMore()", "twice", "twice"), 2);
    }

    #[test]
    fn typescript_servers_open_one_file_of_a_configured_project_and_every_loose_file() {
        let scratch = ide_core::testing::scratch("symbol-seeds");
        let folder = scratch.path().to_path_buf();
        write(&folder, "app/tsconfig.json", "{}\n");
        write(&folder, "app/b.ts", "export const b = 1;\n");
        write(&folder, "app/a.ts", "export const a = 1;\n");
        write(&folder, "loose/one.ts", "function one() {}\n");
        write(&folder, "loose/two.js", "function two() {}\n");
        write(&folder, "go.mod", "module example.com/x\n");
        write(&folder, "main.go", "package main\n");
        let plan = survey(&folder, &[]);
        let seeds = |root: &Path| -> Vec<PathBuf> {
            plan.iter()
                .find(|p| p.language == Language::TypeScript && p.root == root)
                .map(|p| p.seeds.iter().map(|(path, _)| path.clone()).collect())
                .unwrap_or_default()
        };
        assert_eq!(seeds(&folder.join("app")), [folder.join("app/a.ts")]);
        assert_eq!(
            seeds(&folder),
            [folder.join("loose/one.ts"), folder.join("loose/two.js")]
        );
        let go = plan.iter().find(|p| p.language == Language::Go).expect("a Go server");
        assert!(go.seeds.is_empty(), "only TypeScript needs open files to search");
    }

    #[test]
    fn symbols_outside_the_folder_or_in_installed_packages_are_dependencies() {
        let folder = Path::new("/work/app");
        assert!(!is_dependency(Path::new("/work/app/src/main.ts"), folder));
        assert!(is_dependency(Path::new("/work/app/node_modules/x/index.d.ts"), folder));
        assert!(is_dependency(Path::new("/usr/local/go/src/fmt/print.go"), folder));
    }
}
