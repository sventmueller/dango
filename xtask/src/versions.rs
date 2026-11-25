//! The one version of the project (docs/development.md, "Builds"): `version` in `[workspace.package]` of the root
//! `Cargo.toml`, which every member inherits. `cargo xtask versions`, a step of `make lint`, fails for any place that
//! holds another; `deploy` raises it in the manifest and in the lock files, and the seal's fingerprint leaves it out.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::*;

/// The manifest that holds the version.
pub(crate) const MANIFEST: &str = "Cargo.toml";

/// Every file a version bump changes: the manifest and the lock files that list the workspace's members.
pub(crate) const FILES: [&str; 3] = [MANIFEST, "Cargo.lock", "fuzz/Cargo.lock"];

/// What the fingerprint puts in place of the version.
const NEUTRAL: &str = "0.0.0";

const SECTION: &str = "[workspace.package]";

/// The line range of `section`'s keys in `manifest`.
fn section_lines<'a>(manifest: &'a str, section: &str) -> impl Iterator<Item = (usize, &'a str)> {
    manifest
        .lines()
        .enumerate()
        .skip_while(move |(_, line)| line.trim() != section)
        .skip(1)
        .take_while(|(_, line)| !line.trim_start().starts_with('['))
}

/// The value of a `version = "…"` line.
fn quoted_version(line: &str) -> Option<&str> {
    let value = line.strip_prefix("version")?.trim_start().strip_prefix('=')?.trim();
    value.strip_prefix('"')?.strip_suffix('"')
}

/// The version in `[workspace.package]` of `manifest`.
pub(crate) fn workspace_version(manifest: &str) -> Option<&str> {
    section_lines(manifest, SECTION).find_map(|(_, line)| quoted_version(line))
}

/// `manifest` with `version` in `[workspace.package]`.
fn with_workspace_version(manifest: &str, version: &str) -> Result<String, String> {
    let (index, _) = section_lines(manifest, SECTION)
        .find(|(_, line)| quoted_version(line).is_some())
        .ok_or_else(|| format!("{MANIFEST} has no version in {SECTION}"))?;
    let mut lines: Vec<String> = manifest.lines().map(str::to_string).collect();
    lines[index] = format!("version = \"{version}\"");
    let mut text = lines.join("\n");
    if manifest.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// One `[[package]]` entry of a lock file: its name, its version and whether it comes from a registry or git.
struct Locked<'a> {
    name: &'a str,
    version: &'a str,
    sourced: bool,
}

fn locked(block: &str) -> Option<Locked<'_>> {
    let value = |key: &str| {
        block.lines().find_map(|line| {
            let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?.trim();
            rest.strip_prefix('"')?.strip_suffix('"')
        })
    };
    Some(Locked {
        name: value("name")?,
        version: value("version")?,
        sourced: value("source").is_some(),
    })
}

/// The lock file split before each `[[package]]`, so the parts joined again are the file.
fn blocks(lock: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = lock.match_indices("[[package]]\n").map(|(at, _)| at).collect();
    starts.insert(0, 0);
    starts.dedup();
    starts.push(lock.len());
    starts.windows(2).map(|pair| &lock[pair[0]..pair[1]]).collect()
}

/// `lock` with `version` for each of `members`.
fn lock_with(lock: &str, members: &BTreeSet<String>, version: &str) -> String {
    blocks(lock)
        .into_iter()
        .map(|block| match locked(block) {
            Some(entry) if !entry.sourced && members.contains(entry.name) => block.replacen(
                &format!("version = \"{}\"", entry.version),
                &format!("version = \"{version}\""),
                1,
            ),
            _ => block.to_string(),
        })
        .collect()
}

/// Each of `members` in `lock` with the version it is locked at.
fn locked_members<'a>(lock: &'a str, members: &BTreeSet<String>) -> Vec<(&'a str, &'a str)> {
    blocks(lock)
        .into_iter()
        .filter_map(locked)
        .filter(|entry| !entry.sourced && members.contains(entry.name))
        .map(|entry| (entry.name, entry.version))
        .collect()
}

/// `text` of the file `path` with the version left out, for the fingerprint.
pub(crate) fn neutral(path: &str, text: &str, members: &BTreeSet<String>) -> String {
    if path == MANIFEST {
        with_workspace_version(text, NEUTRAL).unwrap_or_else(|_| text.to_string())
    } else {
        lock_with(text, members, NEUTRAL)
    }
}

/// The version of the checkout in `dir`.
pub(crate) fn read(dir: &Path) -> Result<String, String> {
    let path = dir.join(MANIFEST);
    let text = files::read_to_string(&path)?;
    match workspace_version(&text) {
        Some(version) if is_version(version) => Ok(version.to_string()),
        Some(version) => Err(format!("{}: {version:?} is not major.minor.build", path.display())),
        None => Err(format!("{}: no version in {SECTION}", path.display())),
    }
}

/// The names of the workspace's members in `dir`, none where there is no manifest.
pub(crate) fn members(sh: &mut dyn Shell, dir: &Path) -> Result<BTreeSet<String>, String> {
    if !dir.join(MANIFEST).exists() {
        return Ok(BTreeSet::new());
    }
    let metadata = metadata(sh, dir, &[])?;
    Ok(packages(&metadata)?.into_iter().map(|package| package.name).collect())
}

fn metadata(sh: &mut dyn Shell, dir: &Path, extra: &[&str]) -> Result<String, String> {
    sh.stdout(
        Command::new(env!("CARGO"))
            .current_dir(dir)
            .args(["metadata", "--no-deps", "--format-version", "1", "--offline"])
            .args(extra),
    )
}

/// Writes `version` into the manifest in `dir` and into each of its lock files there is, for `members`.
pub(crate) fn bump(dir: &Path, members: &BTreeSet<String>, version: &str) -> Result<(), String> {
    let manifest = dir.join(MANIFEST);
    files::write(
        &manifest,
        with_workspace_version(&files::read_to_string(&manifest)?, version)?,
    )?;
    for lock in &FILES[1..] {
        let path = dir.join(lock);
        if path.exists() {
            files::write(&path, lock_with(&files::read_to_string(&path)?, members, version))?;
        }
    }
    Ok(())
}

/// A workspace member as `cargo metadata` lists it.
struct Package {
    name: String,
    version: String,
    manifest: String,
}

fn packages(metadata: &str) -> Result<Vec<Package>, String> {
    let metadata: Value = serde_json::from_str(metadata).map_err(|e| format!("cargo metadata: {e}"))?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata lists no packages")?;
    Ok(packages
        .iter()
        .map(|package| Package {
            name: package["name"].as_str().unwrap_or_default().to_string(),
            version: package["version"].as_str().unwrap_or_default().to_string(),
            manifest: package["manifest_path"].as_str().unwrap_or_default().to_string(),
        })
        .collect())
}

/// Whether the `[package]` section of `manifest` names a version of its own, and whether it inherits the workspace's.
fn package_version(manifest: &str) -> (bool, bool) {
    let mut own = false;
    let mut inherited = false;
    for (_, line) in section_lines(manifest, "[package]") {
        let line = line.trim();
        if line.starts_with("version.workspace") || line.replace(' ', "").starts_with("version={workspace=true") {
            inherited = true;
        } else if quoted_version(line).is_some() {
            own = true;
        }
    }
    (own, inherited)
}

/// `cargo xtask versions`.
pub(crate) fn check() -> Result<(), String> {
    let sh: &mut dyn Shell = &mut System;
    let dir = root();
    let version = read(&dir)?;
    let mut problems = Vec::new();
    if dir.join("VERSION").exists() {
        problems.push(format!("VERSION is back: the version is in {SECTION} of {MANIFEST}"));
    }
    let members = packages(&metadata(sh, &dir, &["--locked"]).map_err(|e| {
        format!("{e}\nCargo.lock does not match the manifests: run cargo metadata once and commit Cargo.lock")
    })?)?;
    let names: BTreeSet<String> = members.iter().map(|package| package.name.clone()).collect();
    let mut manifests = BTreeSet::new();
    for package in &members {
        manifests.insert(crate::source::relative(&dir, Path::new(&package.manifest)));
        let (own, inherited) = package_version(&files::read_to_string(&package.manifest)?);
        if own || !inherited {
            problems.push(format!(
                "{}: set version.workspace = true instead of a version of its own",
                package.manifest
            ));
        }
        if package.version != version {
            problems.push(format!(
                "{} has version {}, not {version}",
                package.name, package.version
            ));
        }
    }
    for lock in &FILES[1..] {
        let path = dir.join(lock);
        if !path.exists() {
            continue;
        }
        for (name, locked) in locked_members(&files::read_to_string(&path)?, &names) {
            if locked != version {
                problems.push(format!("{lock} locks {name} at {locked}, not {version}"));
            }
        }
    }
    problems.extend(others(sh, &dir, &manifests, &version)?);
    if problems.is_empty() {
        println!(
            "{version} everywhere: {} members, their lock entries and the tag v{version}",
            members.len()
        );
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// What is wrong outside the members: a manifest with a version of its own, a crate that reads its own package version
/// instead of `ide_product::VERSION`, and a version without its tag.
fn others(sh: &mut dyn Shell, dir: &Path, members: &BTreeSet<String>, version: &str) -> Result<Vec<String>, String> {
    let git = |sh: &mut dyn Shell, args: &[&str]| sh.stdout(Command::new("git").arg("-C").arg(dir).args(args));
    let mut problems = Vec::new();
    for manifest in git(sh, &["ls-files", "--", "*Cargo.toml", ":!:vendor/**"])?.lines() {
        if members.contains(manifest) {
            continue;
        }
        if package_version(&files::read_to_string(dir.join(manifest))?).0 {
            problems.push(format!(
                "{manifest} names a version of its own; leave it out, since only the workspace has one"
            ));
        }
    }
    for file in git(
        sh,
        &[
            "grep",
            "-l",
            "--untracked",
            "CARGO_PKG_VERSION",
            "--",
            "src",
            "crates",
            ":!:crates/ide-product",
        ],
    )
    .unwrap_or_default()
    .lines()
    {
        problems.push(format!("{file} reads CARGO_PKG_VERSION; use ide_product::VERSION"));
    }
    if !git(sh, &["tag", "--list", "v*"])?.trim().is_empty()
        && git(sh, &["tag", "--list", &format!("v{version}")])?.trim().is_empty()
    {
        problems.push(format!(
            "{version} has no tag v{version}: only make deploy raises the version, and an interrupted one finishes or \
             undoes it when run again"
        ));
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST_TEXT: &str = "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"0.3.4\"\n\
                                 edition = \"2024\"\n\n[package]\nname = \"app\"\nversion.workspace = true\n";

    const LOCK: &str = "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.3.4\"\ndependencies = [\n \"serde\",\n \
                        \"sibling\",\n]\n\n[[package]]\nname = \"serde\"\nversion = \"0.3.4\"\nsource = \
                        \"registry+https://github.com/rust-lang/crates.io-index\"\n\n[[package]]\nname = \
                        \"sibling\"\nversion = \"0.3.4\"\n";

    fn app() -> BTreeSet<String> {
        ["app".to_string()].into()
    }

    #[test]
    fn the_version_is_read_from_the_workspace_section_only() {
        assert_eq!(workspace_version(MANIFEST_TEXT), Some("0.3.4"));
        assert_eq!(workspace_version("[package]\nversion = \"1.0.0\"\n"), None);
        let raised = with_workspace_version(MANIFEST_TEXT, "0.3.5").unwrap();
        assert_eq!(raised, MANIFEST_TEXT.replace("0.3.4", "0.3.5"));
        assert!(with_workspace_version("[package]\nname = \"x\"\n", "1.0.0").is_err());
    }

    #[test]
    fn a_bump_changes_the_members_in_the_lock_and_nothing_else() {
        let raised = lock_with(LOCK, &app(), "0.3.5");
        assert_eq!(
            raised,
            LOCK.replacen(
                "name = \"app\"\nversion = \"0.3.4\"",
                "name = \"app\"\nversion = \"0.3.5\"",
                1
            )
        );
        assert_eq!(locked_members(&raised, &app()), [("app", "0.3.5")]);
        assert_eq!(blocks(LOCK).concat(), LOCK);
    }

    #[test]
    fn the_neutral_text_is_the_same_on_both_sides_of_a_bump() {
        let lock = lock_with(LOCK, &app(), "0.3.5");
        let manifest = with_workspace_version(MANIFEST_TEXT, "0.3.5").unwrap();
        assert_eq!(
            neutral("Cargo.lock", &lock, &app()),
            neutral("Cargo.lock", LOCK, &app())
        );
        assert_eq!(
            neutral(MANIFEST, &manifest, &app()),
            neutral(MANIFEST, MANIFEST_TEXT, &app())
        );
        assert_ne!(neutral(MANIFEST, MANIFEST_TEXT, &app()), MANIFEST_TEXT);
    }

    #[test]
    fn a_member_must_inherit_and_other_manifests_name_none() {
        assert_eq!(package_version(MANIFEST_TEXT), (false, true));
        assert_eq!(
            package_version("[package]\nname = \"x\"\nversion = { workspace = true }\n"),
            (false, true)
        );
        assert_eq!(
            package_version("[package]\nname = \"x\"\nversion = \"0.1.0\"\n"),
            (true, false)
        );
        assert_eq!(
            package_version("[package]\nname = \"x\"\n\n[dependencies]\nversion = \"1\"\n"),
            (false, false)
        );
    }
}
