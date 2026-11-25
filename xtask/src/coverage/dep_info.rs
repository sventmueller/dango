//! Which source files a build compiled, from the dep-info files cargo writes beside its artifacts.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::files;

/// Every source file the instrumented build compiled, from the dep-info files cargo writes beside each artifact, which
/// name the workspace's files from `root`.
pub(super) fn compiled(build: &Path, root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut sources = BTreeSet::new();
    for profile in ["debug", "release"] {
        for path in dep_info_files(&build.join(profile)) {
            sources.extend(dep_info_sources(&files::read_to_string(&path)?).map(|source| root.join(source)));
        }
    }
    Ok(sources)
}

/// The dep-info files below the profile folder `profile`: in `deps/` of a target folder, and in
/// `build/<package>/<hash>/out/` of a build folder since cargo 1.100.
fn dep_info_files(profile: &Path) -> Vec<PathBuf> {
    let children = |folder: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(folder)
            .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
            .unwrap_or_default()
    };
    let mut folders = vec![profile.join("deps")];
    for package in children(&profile.join("build")) {
        folders.extend(children(&package).into_iter().map(|hash| hash.join("out")));
    }
    folders
        .iter()
        .flat_map(|folder| children(folder))
        .filter(|path| path.extension().is_some_and(|ext| ext == "d"))
        .collect()
}

/// The Rust files a dep-info file names, in make's syntax with spaces escaped as `\ `.
fn dep_info_sources(text: &str) -> impl Iterator<Item = PathBuf> + '_ {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .flat_map(|line| {
            let escaped = line.replace("\\ ", "\u{0}");
            escaped
                .split_whitespace()
                .map(|word| word.trim_end_matches(':').replace('\u{0}', " "))
                .collect::<Vec<_>>()
        })
        .filter(|word| word.ends_with(".rs"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_info_names_the_compiled_sources() {
        let text = "/b/debug/deps/libx.rlib: /r/src/lib.rs /r/src/a\\ b.rs /r/README.md\n\n/r/src/lib.rs:\n\
                    # env-dep:CARGO_PKG_NAME=x\n";
        let sources: Vec<PathBuf> = dep_info_sources(text).collect();
        assert_eq!(
            sources,
            [
                PathBuf::from("/r/src/lib.rs"),
                PathBuf::from("/r/src/a b.rs"),
                PathBuf::from("/r/src/lib.rs")
            ]
        );
    }

    #[test]
    fn dep_info_is_found_in_a_target_folder_and_in_a_build_folder() {
        let dir = crate::shell::fake::scratch("dep-info");
        let write = |path: PathBuf| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "x: src/lib.rs\n").unwrap();
        };
        write(dir.join("debug/deps/x-1.d"));
        write(dir.join("debug/build/y/2/out/y-2.d"));
        write(dir.join("debug/build/y/2/out/y-2.rlib"));
        let mut found = dep_info_files(&dir.join("debug"));
        found.sort();
        assert_eq!(
            found,
            [dir.join("debug/build/y/2/out/y-2.d"), dir.join("debug/deps/x-1.d")]
        );
    }
}
