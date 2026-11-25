//! `cargo xtask test-affected`, the tests of `make check`: those of the workspace crates a change touches and of every
//! crate that depends on them, with the change counted from the upstream branch, so commits not yet pushed count too.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::{flags, root, stdout};

/// Files at the top of the workspace whose change can alter the build or the result of any crate.
const EVERYTHING: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "clippy.toml",
    ".cargo/",
    ".config/",
];

/// A crate built from a folder, with the crates it depends on.
#[derive(Debug, Clone)]
pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) dir: PathBuf,
    pub(crate) member: bool,
    pub(crate) deps: Vec<String>,
}

/// What `cargo test` runs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    Workspace,
    Packages(Vec<String>),
}

pub(crate) fn test_affected(args: &[String]) -> Result<(), String> {
    let [all] = flags(args, ["--all"])?;
    let root = root();
    let packages = packages(&root)?;
    let plan = if all {
        Plan::Workspace
    } else {
        let mut changed = changed_files(&root)?;
        changed.extend(changed_outside(&root, &packages)?);
        plan(&root, &packages, &changed)
    };
    let mut cargo = Command::new("cargo");
    cargo.args(["test", "--locked"]);
    match &plan {
        Plan::Workspace => {
            println!("test-affected: every crate of the workspace");
            cargo.arg("--workspace");
        }
        Plan::Packages(names) if names.is_empty() => {
            println!("test-affected: no crate changed since the upstream branch, no tests to run");
            return Ok(());
        }
        Plan::Packages(names) => {
            println!("test-affected: {}", names.join(", "));
            for name in names {
                cargo.args(["--package", name]);
            }
        }
    }
    let status = cargo
        .current_dir(&root)
        .status()
        .map_err(|e| format!("cargo test: {e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("cargo test failed: {status}"))
}

/// The workspace crates to test after `changed`, absolute paths of changed files.
pub(crate) fn plan(root: &Path, packages: &[Package], changed: &[PathBuf]) -> Plan {
    let everything = changed.iter().any(|file| {
        file.strip_prefix(root).is_ok_and(|relative| {
            let relative = relative.to_string_lossy();
            EVERYTHING
                .iter()
                .any(|top| relative == top.trim_end_matches('/') || (top.ends_with('/') && relative.starts_with(top)))
        })
    });
    if everything {
        return Plan::Workspace;
    }
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    for package in packages {
        for dep in &package.deps {
            dependents.entry(dep.as_str()).or_default().push(&package.name);
        }
    }
    let mut todo: Vec<&str> = changed.iter().filter_map(|file| owner(packages, file)).collect();
    let mut reached: HashSet<&str> = HashSet::new();
    while let Some(name) = todo.pop() {
        if reached.insert(name) {
            todo.extend(dependents.get(name).into_iter().flatten().copied());
        }
    }
    let members: BTreeSet<&str> = packages.iter().filter(|p| p.member).map(|p| p.name.as_str()).collect();
    let chosen: Vec<String> = members
        .iter()
        .filter(|name| reached.contains(*name))
        .map(|name| (*name).to_string())
        .collect();
    if !chosen.is_empty() && chosen.len() == members.len() {
        Plan::Workspace
    } else {
        Plan::Packages(chosen)
    }
}

/// The crate whose folder holds `file` most closely.
fn owner<'a>(packages: &'a [Package], file: &Path) -> Option<&'a str> {
    packages
        .iter()
        .filter(|package| file.starts_with(&package.dir))
        .max_by_key(|package| package.dir.components().count())
        .map(|package| package.name.as_str())
}

/// The crates built from folders, the workspace's and the path dependencies, from `cargo metadata`.
fn packages(root: &Path) -> Result<Vec<Package>, String> {
    let text = stdout(
        Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--locked"])
            .current_dir(root),
    )?;
    let metadata: Value = serde_json::from_str(&text).map_err(|e| format!("cargo metadata: {e}"))?;
    parse_metadata(&metadata).ok_or_else(|| "cargo metadata: unexpected shape".into())
}

pub(crate) fn parse_metadata(metadata: &Value) -> Option<Vec<Package>> {
    let members: HashSet<&str> = metadata["workspace_members"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut names: HashMap<&str, &str> = HashMap::new();
    let mut found = Vec::new();
    for package in metadata["packages"].as_array()? {
        let id = package["id"].as_str()?;
        let name = package["name"].as_str()?;
        names.insert(id, name);
        if package["source"].is_null() {
            let dir = Path::new(package["manifest_path"].as_str()?).parent()?.to_path_buf();
            found.push((id, name, dir));
        }
    }
    let mut deps: HashMap<&str, Vec<String>> = HashMap::new();
    for node in metadata["resolve"]["nodes"].as_array()? {
        let id = node["id"].as_str()?;
        let of: Vec<String> = node["deps"]
            .as_array()?
            .iter()
            .filter_map(|dep| names.get(dep["pkg"].as_str()?).map(|name| (*name).to_string()))
            .collect();
        deps.insert(id, of);
    }
    Some(
        found
            .into_iter()
            .map(|(id, name, dir)| Package {
                name: name.to_string(),
                dir,
                member: members.contains(id),
                deps: deps.remove(id).unwrap_or_default(),
            })
            .collect(),
    )
}

/// The files of this repository that differ from the upstream branch, or from HEAD without one, untracked included.
fn changed_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let git = |args: &[&str]| stdout(Command::new("git").arg("-C").arg(root).args(args));
    let base = git(&["merge-base", "HEAD", "@{upstream}"]).unwrap_or_else(|_| "HEAD".into());
    let tracked = git(&["diff", "--name-only", "-z", base.trim()])?;
    let untracked = git(&["ls-files", "--others", "--exclude-standard", "-z"])?;
    Ok(tracked
        .split('\0')
        .chain(untracked.split('\0'))
        .filter(|path| !path.is_empty())
        .map(|path| root.join(path))
        .collect())
}

/// The folders of path dependencies outside this repository, such as the sibling repositories, with changes not yet
/// committed there.
fn changed_outside(root: &Path, packages: &[Package]) -> Result<Vec<PathBuf>, String> {
    let mut changed = Vec::new();
    for package in packages.iter().filter(|p| !p.dir.starts_with(root)) {
        let status =
            stdout(
                Command::new("git")
                    .arg("-C")
                    .arg(&package.dir)
                    .args(["status", "--porcelain", "-z", "--", "."]),
            )?;
        if !status.is_empty() {
            changed.push(package.dir.clone());
        }
    }
    Ok(changed)
}

#[cfg(test)]
#[path = "affected_tests.rs"]
mod tests;
