//! The `file-icons` scenario: a folder with files of many kinds, shown with their logos and symbols in the Files panel
//! and in Open Quickly and pictured in light, dark and both high-contrast appearances, for a look at whether they
//! read at a glance.

use std::path::PathBuf;
use std::time::Duration;

use objc2_app_kit::{
    NSAppearanceNameAccessibilityHighContrastAqua, NSAppearanceNameAccessibilityHighContrastDarkAqua,
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
};

use super::agent::{appearance, controller_of, picture};
use super::{check, finish, helpers, snapshot};
use crate::util::constant;
use crate::window::Panel;

const NAME: &str = "file-icons";
const PROJECT: &str = "icons-project";
/// How long the tree may take to list the folder.
const LISTING: Duration = Duration::from_secs(10);
/// A turn for the window to redraw in a new appearance before its picture, which no state tells of.
const APPEARANCE_REDRAW: Duration = Duration::from_millis(400);

/// A file of each kind, as the Files panel sorts them after the one folder.
const FILES: &[&str] = &[
    ".editorconfig",
    ".env",
    ".gitignore",
    "api.proto",
    "app.rb",
    "app.ts",
    "app.tsx",
    "App.vue",
    "build.sh",
    "Cargo.lock",
    "Cargo.toml",
    "CHANGELOG.md",
    "chime.wav",
    "data.csv",
    "Dockerfile",
    "fix.patch",
    "index.html",
    "index.js",
    "index.php",
    "Info.plist",
    "Inter.ttf",
    "intro.mov",
    "lexer.c",
    "lexer.h",
    "LICENSE-MIT",
    "logo.svg",
    "main.dart",
    "main.go",
    "Main.hs",
    "Main.java",
    "Main.kt",
    "main.rs",
    "Makefile",
    "manual.pdf",
    "notes.md",
    "notes.txt",
    "package.json",
    "parser_test.go",
    "photo.png",
    "Program.cs",
    "README.md",
    "release.zip",
    "rustfmt.toml",
    "schema.prisma",
    "schema.sql",
    "site.css",
    "TODO.md",
    "tool.py",
    "View.swift",
    "workflow.yml",
];

pub fn run() {
    let _ = std::fs::create_dir_all(helpers::folder(PROJECT).join("src"));
    let files: Vec<(&str, &str)> = FILES.iter().map(|file| (*file, "x\n")).collect();
    helpers::open_project(NAME, PROJECT, &files, |controller| {
        if controller.inspect().panel != Some(Panel::Files) {
            controller.activate_panel(Panel::Files);
        }
        let root = helpers::root(PROJECT);
        let probe = root.clone();
        helpers::wait_within(
            LISTING,
            move || controller_of(&probe).is_some_and(|c| c.tree_rows() == expected_rows()),
            move |_| listed(root),
        );
    });
}

/// The folder and every file.
fn expected_rows() -> isize {
    isize::try_from(FILES.len() + 1).unwrap_or(isize::MAX)
}

/// Pictures the tree once it lists the folder.
fn listed(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let rows = controller.tree_rows();
    let expected = expected_rows();
    check(
        "the Files panel lists a file of every kind",
        rows == expected,
        format!("{rows} rows"),
    );
    let kinds: std::collections::HashSet<_> = FILES.iter().map(|f| crate::file_icon::icon_of(f)).collect();
    check(
        "the files show many different icons",
        kinds.len() * 2 > FILES.len(),
        format!("{} icons for {} files", kinds.len(), FILES.len()),
    );
    let missing: Vec<_> = kinds
        .iter()
        .filter(|icon| crate::file_icon::template(icon).is_none())
        .map(|icon| icon.id)
        .collect();
    check(
        "every icon has an image, each symbol is one macOS knows",
        missing.is_empty(),
        missing.join(", "),
    );
    picture(&controller, "file-icons");
    appearance(&controller, constant!(NSAppearanceNameDarkAqua));
    helpers::after(APPEARANCE_REDRAW, move || dark(root));
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
    helpers::after(APPEARANCE_REDRAW, move || contrast_dark(root));
}

fn contrast_dark(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    picture(&controller, "file-icons-contrast-dark");
    appearance(&controller, constant!(NSAppearanceNameAccessibilityHighContrastAqua));
    helpers::after(APPEARANCE_REDRAW, move || contrast(root));
}

fn contrast(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    picture(&controller, "file-icons-contrast");
    appearance(&controller, constant!(NSAppearanceNameAqua));
    controller.quick_open_with("a");
    let probe = root.clone();
    helpers::wait_until(
        move || controller_of(&probe).is_some_and(|c| c.quick_open_hits().len() > 5),
        move |_| quick_open(root),
    );
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
