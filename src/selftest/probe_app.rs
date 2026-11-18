//! The probe's look at the app beyond its first window: the settings window and its picture, the interface texts, and
//! a folder opened with the login shell's environment, which ends the run.

use std::time::Duration;

use objc2::MainThreadMarker;

use super::*;

/// Settings, texts, opening a folder and the login shell's environment, which ends the run.
pub(super) fn run(mtm: MainThreadMarker) {
    crate::settings::show(mtm);
    let visible = crate::settings::window().is_some_and(|w| w.isVisible());
    check("the settings window opens", visible, "");
    // A window just shown still moves its toolbar's items into place, which a picture taken at once catches midway.
    helpers::after(Duration::from_millis(500), move || {
        if let Some(window) = crate::settings::window() {
            snapshot(&window, "probe-settings");
        }
        probe_texts_and_folder(mtm);
    });
}

/// The interface texts, then a folder opened with the login shell's environment, which ends the run.
fn probe_texts_and_folder(mtm: MainThreadMarker) {
    check(
        "the interface texts load from the bundle",
        !crate::app::localized_strings().is_empty(),
        "",
    );
    let folder = ide_core::paths::home().join("probe-project");
    for (path, text) in [
        ("src/main.rs", "// Grüße, ünïcödé\nfn main() {}\n"),
        ("README.md", "# Probe\n"),
        (".gitignore", "target/\n"),
        ("target/ignored.txt", "x\n"),
    ] {
        let file = folder.join(path);
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&folder));
        let _ = std::fs::write(file, text);
    }
    crate::app::open_project(mtm, folder.clone());
    let canonical = folder.canonicalize().unwrap_or(folder);
    check(
        "an opened folder gets its window",
        window::open_projects().contains(&canonical),
        format!("{:?}", window::open_projects()),
    );
    let Some(controller) = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&canonical))
    else {
        give_up("probe", "the opened folder has its window");
        return;
    };
    check("an untrusted folder asks for trust", controller.has_sheet(), "");
    clicks_pass_through(mtm);
    let probe = canonical.clone();
    helpers::wait_until(
        move || window::for_project(&probe).is_some_and(|c| c.tree_rows() > 0),
        move |_| probe_ignored(canonical),
    );
}
