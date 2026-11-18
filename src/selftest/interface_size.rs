//! The step of the `panels` scenario for the text sizes from the settings: an open window follows the interface size
//! at once, its folder tree, activity bar and tab title in proportion, and its editor the editor's size; the defaults
//! give it back as it was.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::settings::{Settings, keys};
use ide_ui_common::ui_size;

use super::{check, helpers};
use crate::window::{self, WindowController};

/// The interface size the step sets.
const LARGE: f64 = 18.0;
/// The editor's size the step sets.
const EDITOR: f64 = 20.0;
/// A pause for the window to lay itself out again, which has no state to wait for.
const PAUSE: Duration = Duration::from_millis(600);

/// What the step measures of the window.
#[derive(Clone, Copy, Debug)]
struct Seen {
    row: f64,
    sidebar: f64,
    title: f64,
    editor: f64,
}

fn seen(controller: &WindowController, file: &Path) -> Seen {
    Seen {
        row: controller
            .file_tree_outline()
            .map_or(0.0, |outline| outline.rowHeight()),
        sidebar: controller.inspect().sidebar_width,
        title: controller.tab_title_points(),
        editor: crate::editor::editor_for(controller, file).map_or(0.0, |view| view.font_size()),
    }
}

fn window_of(root: &Path) -> Option<objc2::rc::Retained<WindowController>> {
    window::open_windows()
        .into_iter()
        .find(|w| w.project().as_deref() == Some(root))
}

/// Sets both sizes as the settings window does, and has the app take them as a change of the settings file would.
fn store(interface: f64, editor: f64) {
    let stored = Settings::load_user()
        .set(&keys::INTERFACE_FONT_SIZE, &interface)
        .and_then(|()| Settings::load_user().set(&keys::FONT_SIZE, &editor));
    if let Err(error) = stored {
        crate::diag::report("setting the text sizes for the self-test", error);
    }
    let names = [
        keys::INTERFACE_FONT_SIZE.declared().name.to_string(),
        keys::FONT_SIZE.declared().name.to_string(),
    ];
    crate::editor::settings_changed(&names);
    ui_size::settings_changed(&names);
}

/// Runs the step in the window of `root` with `file` open in front, then `then`.
pub(super) fn run(root: PathBuf, file: PathBuf, then: impl FnOnce(PathBuf) + 'static) {
    let Some(controller) = window_of(&root) else {
        return then(root);
    };
    let before = seen(&controller, &file);
    let editor_before = Settings::load_user().get(&keys::FONT_SIZE);
    store(LARGE, EDITOR);
    helpers::after(PAUSE, move || larger(root, file, before, editor_before, then));
}

fn larger(root: PathBuf, file: PathBuf, before: Seen, editor_before: f64, then: impl FnOnce(PathBuf) + 'static) {
    let Some(controller) = window_of(&root) else {
        return then(root);
    };
    let large = seen(&controller, &file);
    if let Some(front) = controller.window() {
        super::snapshot(&front, "panels-large-text");
    }
    let factor = LARGE / ui_size::DEFAULT;
    let wider = crate::window::ACTIVITY_WIDTH * (factor - 1.0);
    check(
        "a larger interface size reaches the open window's folder tree, activity bar and tab title at once",
        (large.row - before.row * factor).abs() < 0.5
            && (large.title - before.title * factor).abs() < 0.5
            && (large.sidebar - before.sidebar - wider).abs() < 2.0,
        format!("{before:?} → {large:?}"),
    );
    check(
        "a larger editor size reaches the open editor at once",
        (large.editor - EDITOR).abs() < f64::EPSILON,
        format!("{} → {}", before.editor, large.editor),
    );
    store(ui_size::DEFAULT, editor_before);
    helpers::after(PAUSE, move || back(root, file, before, then));
}

fn back(root: PathBuf, file: PathBuf, before: Seen, then: impl FnOnce(PathBuf) + 'static) {
    if let Some(controller) = window_of(&root) {
        let after = seen(&controller, &file);
        check(
            "the default sizes give the window back as it was",
            (after.row - before.row).abs() < 0.01
                && (after.title - before.title).abs() < 0.01
                && (after.sidebar - before.sidebar).abs() < 1.0
                && (after.editor - before.editor).abs() < f64::EPSILON,
            format!("{before:?} → {after:?}"),
        );
    }
    then(root);
}
