//! The `file-icons` scenario: a folder with files of many kinds, shown with their Seti icons in the Files panel and
//! in Open Quickly and pictured in light, dark and both high-contrast appearances, for a look at whether they match
//! VS Code's.

use std::path::PathBuf;
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAppearanceNameAccessibilityHighContrastAqua, NSAppearanceNameAccessibilityHighContrastDarkAqua,
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
};

use super::agent::{appearance, controller_of, picture};
use super::{check, finish, snapshot};
use crate::util::{constant, main_after};
use crate::window::Panel;

const NAME: &str = "file-icons";

/// A file of each kind, as the Files panel sorts them after the one folder.
const FILES: &[&str] = &[
    ".editorconfig",
    ".env",
    ".gitignore",
    "api.proto",
    "app.rb",
    "app.tsx",
    "build.sh",
    "Cargo.lock",
    "Cargo.toml",
    "CHANGELOG.md",
    "data.csv",
    "Dockerfile",
    "index.html",
    "index.js",
    "index.php",
    "Info.plist",
    "lexer.c",
    "lexer.h",
    "LICENSE-MIT",
    "logo.svg",
    "main.go",
    "main.rs",
    "Main.java",
    "Main.kt",
    "Makefile",
    "manual.pdf",
    "notes.md",
    "package.json",
    "parser_test.go",
    "photo.png",
    "README.md",
    "release.zip",
    "rustfmt.toml",
    "schema.sql",
    "site.css",
    "tool.py",
    "View.swift",
    "workflow.yml",
];

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = ide_core::paths::home().join("icons-project");
    let _ = std::fs::create_dir_all(root.join("src"));
    for file in FILES {
        let _ = std::fs::write(root.join(file), "x\n");
    }
    let root = root.canonicalize().unwrap_or(root);
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    if let Some(controller) = controller_of(&root).filter(|c| c.inspect().panel != Some(Panel::Files)) {
        controller.activate_panel(Panel::Files);
    }
    main_after(Duration::from_millis(800), move || listed(root, 0));
}

/// Waits up to ten seconds for the tree to list the folder, then pictures it.
fn listed(root: PathBuf, waited: u32) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let rows = controller.tree_rows();
    let expected = isize::try_from(FILES.len() + 1).unwrap_or(isize::MAX);
    if rows < expected && waited < 50 {
        main_after(Duration::from_millis(200), move || listed(root, waited + 1));
        return;
    }
    check(
        "the Files panel lists a file of every kind",
        rows == expected,
        format!("{rows} rows"),
    );
    let kinds: std::collections::HashSet<_> = FILES.iter().map(|f| crate::file_icon::icon_of(f)).collect();
    check(
        "the files show many different icons, as VS Code with Seti does",
        kinds.len() * 2 > FILES.len(),
        format!("{} icons for {} files", kinds.len(), FILES.len()),
    );
    picture(&controller, "file-icons");
    appearance(&controller, constant!(NSAppearanceNameDarkAqua));
    main_after(Duration::from_millis(400), move || dark(root));
}

fn dark(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    picture(&controller, "file-icons-dark");
    appearance(
        &controller,
        constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua),
    );
    main_after(Duration::from_millis(400), move || contrast_dark(root));
}

fn contrast_dark(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    picture(&controller, "file-icons-contrast-dark");
    appearance(&controller, constant!(NSAppearanceNameAccessibilityHighContrastAqua));
    main_after(Duration::from_millis(400), move || contrast(root));
}

fn contrast(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    picture(&controller, "file-icons-contrast");
    appearance(&controller, constant!(NSAppearanceNameAqua));
    controller.quick_open_with("a");
    main_after(Duration::from_secs(1), move || quick_open(root));
}

fn quick_open(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let hits = controller.quick_open_hits();
    check(
        "Open Quickly lists files of several kinds",
        hits.len() > 5,
        hits.join(", "),
    );
    let panel = controller
        .window()
        .and_then(|window| window.childWindows())
        .and_then(|children| children.firstObject());
    if let Some(panel) = panel {
        panel.displayIfNeeded();
        snapshot(&panel, "file-icons-quick-open");
    }
    finish(NAME);
}
