use std::path::{Path, PathBuf};

use super::*;

fn package(name: &str, dir: &str, member: bool, deps: &[&str]) -> Package {
    Package {
        name: name.into(),
        dir: PathBuf::from(dir),
        member,
        deps: deps.iter().map(|dep| (*dep).to_string()).collect(),
    }
}

fn workspace() -> Vec<Package> {
    vec![
        package("ide-app", "/repo", true, &["ide-ui-git", "ide-git", "splat-i18n"]),
        package(
            "ide-ui-git",
            "/repo/crates/ide-ui-git",
            true,
            &["ide-git", "splat-i18n"],
        ),
        package("ide-git", "/repo/crates/ide-git", true, &[]),
        package("ide-lsp", "/repo/crates/ide-lsp", true, &[]),
        package("xtask", "/repo/xtask", true, &[]),
        package("modalkit", "/repo/vendor/modalkit", false, &[]),
        package("ide-vim", "/repo/crates/ide-vim", true, &["modalkit"]),
        package("splat-i18n", "/splat/crates/splat-i18n", false, &[]),
    ]
}

fn plan_for(files: &[&str]) -> Plan {
    let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
    plan(Path::new("/repo"), &workspace(), &files)
}

fn packages(names: &[&str]) -> Plan {
    Plan::Packages(names.iter().map(|name| (*name).to_string()).collect())
}

#[test]
fn a_crate_brings_the_crates_that_depend_on_it() {
    assert_eq!(
        plan_for(&["/repo/crates/ide-ui-git/src/rows.rs"]),
        packages(&["ide-app", "ide-ui-git"])
    );
    assert_eq!(
        plan_for(&["/repo/crates/ide-git/src/status.rs"]),
        packages(&["ide-app", "ide-git", "ide-ui-git"])
    );
}

#[test]
fn a_file_of_the_app_or_its_resources_tests_the_app_alone() {
    assert_eq!(plan_for(&["/repo/src/window/mod.rs"]), packages(&["ide-app"]));
    assert_eq!(
        plan_for(&["/repo/resources/en.lproj/Localizable.strings"]),
        packages(&["ide-app"])
    );
}

#[test]
fn a_vendored_or_sibling_crate_tests_the_members_that_use_it() {
    assert_eq!(plan_for(&["/repo/vendor/modalkit/src/lib.rs"]), packages(&["ide-vim"]));
    assert_eq!(
        plan_for(&["/splat/crates/splat-i18n"]),
        packages(&["ide-app", "ide-ui-git"])
    );
}

#[test]
fn the_manifest_lock_or_toolchain_test_everything() {
    for file in [
        "/repo/Cargo.toml",
        "/repo/Cargo.lock",
        "/repo/rust-toolchain.toml",
        "/repo/.cargo/config.toml",
    ] {
        assert_eq!(plan_for(&[file]), Plan::Workspace, "{file}");
    }
    assert_eq!(
        plan_for(&["/repo/crates/ide-git/Cargo.toml"]),
        packages(&["ide-app", "ide-git", "ide-ui-git"])
    );
}

#[test]
fn no_change_tests_nothing_and_every_crate_is_the_workspace() {
    assert_eq!(plan_for(&[]), packages(&[]));
    assert_eq!(
        plan_for(&[
            "/repo/crates/ide-git/a.rs",
            "/repo/crates/ide-lsp/a.rs",
            "/repo/xtask/src/main.rs",
            "/repo/vendor/modalkit/a.rs"
        ]),
        Plan::Workspace
    );
}

#[test]
fn metadata_gives_path_crates_with_their_dependencies() {
    let metadata = serde_json::json!({
        "workspace_members": ["path+file:///repo#ide-app@0.0.0"],
        "packages": [
            {"id": "path+file:///repo#ide-app@0.0.0", "name": "ide-app", "source": null,
             "manifest_path": "/repo/Cargo.toml"},
            {"id": "path+file:///splat/crates/splat-i18n#0.0.0", "name": "splat-i18n", "source": null,
             "manifest_path": "/splat/crates/splat-i18n/Cargo.toml"},
            {"id": "registry+https://x#serde@1.0.0", "name": "serde",
             "source": "registry+https://github.com/rust-lang/crates.io-index",
             "manifest_path": "/home/.cargo/serde/Cargo.toml"}
        ],
        "resolve": {"nodes": [
            {"id": "path+file:///repo#ide-app@0.0.0", "deps": [
                {"pkg": "path+file:///splat/crates/splat-i18n#0.0.0"}, {"pkg": "registry+https://x#serde@1.0.0"}]},
            {"id": "path+file:///splat/crates/splat-i18n#0.0.0", "deps": []},
            {"id": "registry+https://x#serde@1.0.0", "deps": []}
        ]}
    });
    let found = parse_metadata(&metadata).expect("parses");
    let shown: Vec<(&str, &Path, bool, Vec<&str>)> = found
        .iter()
        .map(|p| {
            (
                p.name.as_str(),
                p.dir.as_path(),
                p.member,
                p.deps.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("ide-app", Path::new("/repo"), true, vec!["splat-i18n", "serde"]),
            ("splat-i18n", Path::new("/splat/crates/splat-i18n"), false, vec![]),
        ]
    );
}
