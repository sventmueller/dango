//! `cargo xtask toolchain`, the first step of `make lint` (B114, B115): the rustc that runs the checks is the version
//! `rust-toolchain.toml` pins, which Homebrew's Rust does not read, and every package's `rust-version` is that pin's
//! minor version, so the resolver picks no dependency the pinned builds cannot compile.

use std::process::Command;

use serde_json::Value;

use crate::*;

pub(crate) fn check() -> Result<(), String> {
    let root = root();
    let pin = crate::coverage::toolchain(&files::read_to_string(root.join("rust-toolchain.toml"))?)?;
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compiler = stdout(Command::new(rustc).arg("-V"))?;
    let metadata = stdout(Command::new(env!("CARGO")).current_dir(&root).args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--offline",
        "--locked",
    ]))?;
    let problems = review(&pin, &compiler, &packages(&metadata)?);
    if problems.is_empty() {
        println!("rustc {pin} as rust-toolchain.toml pins it, and every package's rust-version matches");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Each workspace member's name and `rust-version`, from `cargo metadata --no-deps`.
fn packages(metadata: &str) -> Result<Vec<(String, Option<String>)>, String> {
    let metadata: Value = serde_json::from_str(metadata).map_err(|e| format!("cargo metadata: {e}"))?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata lists no packages")?;
    Ok(packages
        .iter()
        .map(|package| {
            let name = package["name"].as_str().unwrap_or_default().to_string();
            (name, package["rust_version"].as_str().map(str::to_string))
        })
        .collect())
}

/// What is wrong with the pin `pin` for the compiler that printed `compiler` and for `packages`.
fn review(pin: &str, compiler: &str, packages: &[(String, Option<String>)]) -> Vec<String> {
    let mut parts = pin.split('.');
    let (Some(major), Some(minor), Some(_), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return vec![format!(
            "rust-toolchain.toml pins {pin:?}, not a version such as 1.100.0"
        )];
    };
    let mut problems = Vec::new();
    let running = compiler.split_whitespace().nth(1).unwrap_or_default();
    if running != pin {
        problems.push(format!(
            "{} runs the checks, but rust-toolchain.toml pins {pin}: bump the pin or the compiler",
            compiler.trim()
        ));
    }
    let wanted = format!("{major}.{minor}");
    for (name, version) in packages {
        match version {
            Some(version) if *version == wanted || version == pin => {}
            Some(version) => problems.push(format!(
                "{name} has rust-version {version}, but rust-toolchain.toml pins {pin}: set rust-version = \
                 \"{wanted}\" in [workspace.package]"
            )),
            None => problems.push(format!(
                "{name} has no rust-version: add rust-version.workspace = true to its Cargo.toml"
            )),
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOMEBREW: &str = "rustc 1.100.0 (0ef1bc386 2026-11-09) (Homebrew)\n";

    fn members(versions: &[Option<&str>]) -> Vec<(String, Option<String>)> {
        versions
            .iter()
            .enumerate()
            .map(|(n, version)| (format!("crate-{n}"), version.map(str::to_string)))
            .collect()
    }

    #[test]
    fn the_pinned_compiler_and_its_minor_version_everywhere_pass() {
        assert!(review("1.100.0", HOMEBREW, &members(&[Some("1.100"), Some("1.100.0")])).is_empty());
    }

    #[test]
    fn another_compiler_asks_for_a_bump() {
        let problems = review("1.99.1", HOMEBREW, &members(&[]));
        assert_eq!(
            problems,
            [
                "rustc 1.100.0 (0ef1bc386 2026-11-09) (Homebrew) runs the checks, but rust-toolchain.toml pins 1.99.1: \
              bump the pin or the compiler"
            ]
        );
    }

    #[test]
    fn a_package_without_the_pins_rust_version_is_named() {
        let problems = review("1.100.0", HOMEBREW, &members(&[Some("1.99"), None, Some("1.100")]));
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems[0].starts_with("crate-0 has rust-version 1.99, but"),
            "{}",
            problems[0]
        );
        assert!(
            problems[1].starts_with("crate-1 has no rust-version"),
            "{}",
            problems[1]
        );
    }

    #[test]
    fn a_channel_that_is_no_version_is_refused() {
        for pin in ["stable", "nightly-2026-11-04", "1.100"] {
            let problems = review(pin, HOMEBREW, &members(&[]));
            assert!(problems[0].contains("not a version"), "{pin}: {problems:?}");
        }
    }

    #[test]
    fn the_metadata_names_each_member_with_its_rust_version() {
        let metadata = r#"{"packages":[{"name":"ide-app","rust_version":"1.100"},{"name":"old","rust_version":null}]}"#;
        assert_eq!(
            packages(metadata).unwrap(),
            [
                ("ide-app".to_string(), Some("1.100".to_string())),
                ("old".to_string(), None)
            ]
        );
        assert!(packages("{}").is_err());
    }
}
