//! Restoring 30 clean file tabs and measuring the first edit, the first unopened tab switch and memory.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ide_process::{Supervisor, system::physical_footprint};
use objc2_app_kit::NSAlertSecondButtonReturn;
use objc2_foundation::NSRange;

use super::{check, finish, give_up, helpers, snappy_input};
use crate::window::WindowController;

const NAME: &str = "snappy-restore";
const TABS: usize = 30;
const MIB: u64 = 1024 * 1024;

pub(crate) fn run() {
    let text = "an ordinary line in a restored document\n".repeat(10_000);
    let names: Vec<_> = (0..TABS).map(|n| format!("tab{n}.txt")).collect();
    let inputs: Vec<_> = names.iter().map(|name| (name.as_str(), text.as_str())).collect();
    helpers::open_project(NAME, NAME, &inputs, |controller| {
        let root = helpers::root(NAME);
        let paths: Vec<_> = (0..TABS).map(|n| root.join(format!("tab{n}.txt"))).collect();
        restore(&controller, &paths);
    });
}

fn restore(controller: &WindowController, paths: &[PathBuf]) {
    let before = memory();
    let started = Instant::now();
    controller.clear_tabs();
    controller.restore_editor_tabs(|| {
        for path in paths {
            controller.restore_editor(path, 13.0);
        }
    });
    let front = paths[TABS - 1].clone();
    let other = paths[0].clone();
    let window = controller.id();
    let queued = controller.when_restored_editor(&front, move |view| {
        helpers::type_at(view, NSRange::new(0, 0), "x");
        show(window);
        check(
            "the selected file takes the first edit while background files still have placeholders",
            crate::host::window(window)
                .is_some_and(|controller| crate::editor::editor_for(&controller, &other).is_none()),
            format!(
                "{} ms from restore to first displayed edit, before {before}, after {}",
                started.elapsed().as_millis(),
                memory()
            ),
        );
        helpers::after(Duration::ZERO, move || switch_to(window, other));
    });
    if queued.is_err() {
        give_up(NAME, "the selected tab waits for its editor");
    }
}

fn switch_to(window: ide_ui_common::window::WindowId, path: PathBuf) {
    let Some(controller) = crate::host::window(window) else {
        return give_up(NAME, "the restored window is open");
    };
    let started = Instant::now();
    controller.open_file(&path);
    let edit = path.clone();
    let queued = controller.when_restored_editor(&path, move |_| {
        show(window);
        check(
            "the first background tab opens on demand",
            true,
            format!("{} ms, {}", started.elapsed().as_millis(), memory()),
        );
        helpers::after(Duration::ZERO, move || sample_input(window, edit));
    });
    if queued.is_err() {
        give_up(NAME, "the inactive tab still needs to load");
    }
}

fn sample_input(window: ide_ui_common::window::WindowId, path: PathBuf) {
    let during = memory();
    snappy_input::steps(
        30,
        move |_| {
            if let Some(view) = crate::host::window(window).and_then(|c| crate::editor::editor_for(&c, &path)) {
                helpers::type_at(&view, NSRange::new(0, 0), "x");
            }
        },
        move |times| {
            check(
                "typing in a restored editor reports input-to-display timings",
                times.len() == 30,
                format!("{}, during {during}, after {}", snappy_input::spread(&times), memory()),
            );
            helpers::answer_alert_when_shown(None, NSAlertSecondButtonReturn);
            finish(NAME);
        },
    );
}

fn show(window: ide_ui_common::window::WindowId) {
    if let Some(window) = crate::host::window(window).and_then(|c| c.window()) {
        window.displayIfNeeded();
    }
}

fn memory() -> String {
    let app = physical_footprint(std::process::id())
        .map_or_else(|| "unavailable".to_string(), |bytes| format!("{} MiB", bytes / MIB));
    let workers: Vec<_> = Supervisor::global()
        .running()
        .into_iter()
        .filter_map(|process| physical_footprint(process.pid))
        .collect();
    format!(
        "app {app}, {} supervised workers {} MiB",
        workers.len(),
        workers.iter().sum::<u64>() / MIB
    )
}
