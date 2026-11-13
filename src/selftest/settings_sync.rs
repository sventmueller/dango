//! The `settings-sync` scenario (B28, E112–E114) in a folder the scenario picks in place of iCloud Drive's: turning the
//! sync on writes this Mac's file, another Mac's newer change arrives through the file presenter and reaches the
//! editors, a shortcut syncs into `keybindings.json`, a local change goes out with a clock above everybody's, and a
//! file of a newer app version pauses the sync with a note.

use std::time::Duration;

use ide_core::settings::{Settings, keys};
use ide_core::settings_sync::{Clock, DeviceFile, Entry};
use serde_json::json;

use super::helpers::wait_within;
use super::{check, finish};
use crate::settings_sync::{self, Status};

const SCENARIO: &str = "settings-sync";

fn other(device: &str, schema: u64, entries: &[(&str, serde_json::Value, u64)]) -> DeviceFile {
    let mut file = DeviceFile::empty(device, schema);
    for (name, value, counter) in entries {
        file.entries.insert(
            (*name).to_string(),
            Entry {
                value: Some(value.clone()),
                clock: Clock {
                    counter: *counter,
                    wall: 1,
                },
            },
        );
    }
    file
}

fn own() -> Option<DeviceFile> {
    let (folder, device) = settings_sync::where_and_who();
    let bytes = std::fs::read(folder.join(format!("{device}.json"))).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn run() {
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let folder = ide_core::paths::data_dir().join("sync-folder");
    if let Err(error) = std::fs::create_dir_all(&folder) {
        check("the sync folder is made", false, error.to_string());
        return finish(SCENARIO);
    }
    let mut settings = Settings::load_user();
    let _ = settings.set(&keys::FONT_SIZE, &13.0);
    settings_sync::use_folder(mtm, &folder);
    settings_sync::set_enabled(mtm, true);
    wait_within(
        Duration::from_secs(15),
        || own().is_some_and(|f| f.entries.contains_key("fontSize")),
        move |written| {
            let own = own();
            check(
                "turning it on writes this Mac's file with its shared settings at clock 0, not its per-Mac ones",
                written
                    && own.as_ref().is_some_and(|f| {
                        f.entries["fontSize"].clock == Clock::default() && !f.entries.contains_key("settingsSync")
                    }),
                format!("{own:?}"),
            );
            arrives(folder);
        },
    );
}

fn arrives(folder: std::path::PathBuf) {
    let file = other(
        "zz-other-mac",
        Settings::schema_version(),
        &[
            ("minimap", json!(true), 3),
            ("keybinding:view.wrap", json!("ctrl+alt+cmd+w"), 3),
        ],
    );
    let bytes = serde_json::to_vec(&file).unwrap_or_default();
    if let Err(error) = std::fs::write(folder.join("zz-other-mac.json"), bytes) {
        check("another Mac's file is written", false, error.to_string());
        return finish(SCENARIO);
    }
    wait_within(
        Duration::from_secs(15),
        || Settings::load_user().get(&keys::MINIMAP),
        move |arrived| {
            let shortcuts = std::fs::read_to_string(ide_core::keymap::Keymap::user_path()).unwrap_or_default();
            let shown = objc2::MainThreadMarker::new().is_some_and(|mtm| {
                crate::context::AppContext::get(mtm).keymap.borrow().key("view.wrap") == Some("ctrl+alt+cmd+w")
            });
            check(
                "another Mac's newer change arrives on its own, the shortcut too, and the menus take it",
                arrives_ok(arrived, &shortcuts, shown),
                format!("minimap {arrived}, keybindings {shortcuts:?}, menu {shown}"),
            );
            goes_out(folder);
        },
    );
}

fn arrives_ok(arrived: bool, shortcuts: &str, shown: bool) -> bool {
    arrived && shortcuts.contains("ctrl+alt+cmd+w") && shown
}

fn goes_out(folder: std::path::PathBuf) {
    let mut settings = Settings::load_user();
    let _ = settings.set(&keys::SHOW_WHITESPACE, &true);
    wait_within(
        Duration::from_secs(15),
        || {
            own().is_some_and(|f| {
                f.entries
                    .get("showWhitespace")
                    .is_some_and(|e| e.value == Some(json!(true)))
            })
        },
        move |sent| {
            let counter = own().and_then(|f| f.entries.get("showWhitespace").map(|e| e.clock.counter));
            check(
                "a local change goes out with a clock above every one seen",
                sent && counter.is_some_and(|c| c > 3),
                format!("{counter:?}"),
            );
            paused(folder);
        },
    );
}

fn paused(folder: std::path::PathBuf) {
    let file = other(
        "zz-newer-mac",
        Settings::schema_version() + 1,
        &[("fontSize", json!(30), 99)],
    );
    let _ = std::fs::write(
        folder.join("zz-newer-mac.json"),
        serde_json::to_vec(&file).unwrap_or_default(),
    );
    wait_within(
        Duration::from_secs(15),
        || objc2::MainThreadMarker::new().is_some_and(|mtm| settings_sync::status(mtm) == Status::Paused),
        |is_paused| {
            let size = Settings::load_user().get(&keys::FONT_SIZE);
            check(
                "a file of a newer version is left out and pauses the sync",
                is_paused && (size - 13.0).abs() < f64::EPSILON,
                format!("paused {is_paused}, font size {size}"),
            );
            if let Some(mtm) = objc2::MainThreadMarker::new() {
                settings_sync::set_enabled(mtm, false);
            }
            finish(SCENARIO);
        },
    );
}
