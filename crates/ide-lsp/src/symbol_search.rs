//! What a search for symbols across a project decides without a server: which servers it needs, and how well a
//! name matches the query.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{CustomServer, Language};

/// The servers a symbol search needs in `folder`: one per language that has files there that ignore rules do not hide
/// and whose servers search symbols, and per project root of those files.
pub fn survey(folder: &Path, custom: &[Arc<CustomServer>]) -> Vec<(Language, PathBuf)> {
    let Ok(index) = ide_project::FileIndex::build(folder, &ide_project::Cancel::new()) else {
        return vec![];
    };
    let mut directories: HashSet<(Language, PathBuf)> = HashSet::new();
    for file in index.files() {
        let path = folder.join(file);
        let Some((language, _)) = Language::detect(&path, custom) else {
            continue;
        };
        if language.searches_project() {
            let directory = path.parent().unwrap_or(folder).to_path_buf();
            directories.insert((language, directory));
        }
    }
    let mut plan: Vec<(Language, PathBuf)> = directories
        .into_iter()
        .map(|(language, directory)| {
            let root = crate::root(&language, &directory, folder);
            (language, root)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    plan.sort_by(|a, b| (a.0.name(), &a.1).cmp(&(b.0.name(), &b.1)));
    plan
}

/// How well `name` matches `query`: the exact name first, then without case, then names that start with it, then
/// those that contain it, then whatever else the server found.
pub fn rank(name: &str, query: &str, lowered: &str) -> u8 {
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
        let plan = survey(&folder, &[]);
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
}
