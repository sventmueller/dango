//! The `tabs` scenario: preview tabs, which the user turns on (E62). A single click in the folder tree opens a file in
//! the preview tab, the next one replaces it, a double click, typing or a double click on the tab keeps it, and with
//! the setting off every file gets a tab of its own.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::settings::{Settings, keys};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use splat_i18n::t;

use super::{check, finish, snapshot, type_into};
use crate::util::main_after;
use crate::window::{self, WindowController};

const SCENARIO: &str = "tabs";
const FILES: [&str; 5] = ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"];

/// Whether the tabs hold `name`, as the preview tab when `preview`.
fn has(controller: &WindowController, name: &str, preview: bool) -> bool {
    controller
        .tab_previews()
        .iter()
        .any(|(title, is_preview, _)| title == name && *is_preview == preview)
}

fn tabs_of(controller: &WindowController) -> String {
    controller
        .tab_previews()
        .iter()
        .map(|(title, preview, _)| if *preview { format!("[{title}]") } else { title.clone() })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = ide_core::paths::home().join("tabs-project");
    let _ = std::fs::create_dir_all(&folder);
    for name in FILES {
        let _ = std::fs::write(folder.join(name), format!("// {name}\nfn main() {{}}\n"));
    }
    crate::project::trust(&folder);
    if let Err(error) = Settings::load_user().set(&keys::PREVIEW_TABS, &true) {
        check("preview tabs can be turned on", false, error.to_string());
    }
    crate::app::open_project(mtm, folder.clone());
    let root = folder.canonicalize().unwrap_or(folder);
    main_after(PAUSE, move || first_preview(root));
}

/// A pause for the window to draw before a picture.
const PAUSE: Duration = Duration::from_millis(500);

fn window_of(root: &Path) -> Option<Retained<WindowController>> {
    let controller = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_deref() == Some(root));
    if controller.is_none() {
        check("the folder gets its window", false, root.display().to_string());
        finish(SCENARIO);
    }
    controller
}

fn first_preview(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let before = controller.tab_count();
    controller.click_in_tree(&root.join("a.rs"), false);
    let spoken = controller
        .tab_previews()
        .into_iter()
        .find(|(title, _, _)| title == "a.rs")
        .map(|(_, _, spoken)| spoken)
        .unwrap_or_default();
    check(
        "a single click opens a file in the preview tab",
        controller.tab_count() == before + 1 && has(&controller, "a.rs", true),
        tabs_of(&controller),
    );
    check(
        "the preview tab's tooltip, which VoiceOver reads as its help, says that it is a preview",
        spoken
            == t(
                "Vorschau: Die nächste Datei, die du mit einem Klick öffnest, ersetzt diesen Tab. Ein Doppelklick behält ihn.",
            ),
        spoken,
    );
    main_after(PAUSE, move || replaced(root));
}

fn replaced(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(window) = controller.window() {
        snapshot(&window, "tabs-preview");
    }
    let before = controller.tab_count();
    controller.click_in_tree(&root.join("b.rs"), false);
    check(
        "the next single click replaces the preview",
        controller.tab_count() == before && has(&controller, "b.rs", true) && !has(&controller, "a.rs", true),
        tabs_of(&controller),
    );
    controller.click_in_tree(&root.join("b.rs"), true);
    check(
        "a double click in the tree keeps the tab",
        controller.tab_count() == before && has(&controller, "b.rs", false),
        tabs_of(&controller),
    );
    keeping(&controller, &root);
    new_tab_page(&controller, &root);
    turned_off(&controller, &root);
    main_after(PAUSE, move || {
        if let Some(window) = window_of(&root).and_then(|c| c.window()) {
            snapshot(&window, "tabs-kept");
        }
        variety(root);
    });
}

/// Documents of every kind show an icon before their title: files the icon of their type, the others a symbol.
fn variety(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let _ = std::fs::write(root.join("site.css"), "h1 { color: red }\n");
    let _ = std::fs::write(root.join("notiz.md"), "# Notiz\n\nText.\n");
    controller.open_file(&root.join("site.css"));
    crate::browser::open_tab(&controller, None);
    let replayed = super::panels::replay_agent(&controller, "51e1f7e5-0000-4000-8000-0000000000cc");
    controller.open_file(&root.join("notiz.md"));
    let icons = controller.tab_icons();
    check(
        "every tab shows a small icon before its title: files, the browser, the AI thread, the preview",
        replayed && icons.len() > 6 && icons.iter().all(|shown| *shown),
        format!("{icons:?} for {:?}", controller.window_tabs()),
    );
    main_after(PAUSE, move || {
        super::git::pictures(root, "tabs-icons", |_| finish(SCENARIO));
    });
}

fn keeping(controller: &WindowController, root: &Path) {
    let before = controller.tab_count();
    let file: PathBuf = root.join("c.rs");
    controller.click_in_tree(&file, false);
    let editor = crate::editor::editor_of(controller, &file);
    if let Some(editor) = &editor {
        type_into(editor, "x");
    }
    check(
        "typing keeps the preview tab",
        controller.tab_count() == before + 1 && has(controller, "c.rs", false),
        tabs_of(controller),
    );
    let native = controller.native_tabs();
    check(
        "the native tab names its document and shows its unsaved changes",
        native.iter().any(|(title, _, edited)| title == "c.rs" && *edited),
        format!("{native:?}"),
    );

    // Quitting with unsaved changes would ask what to do with them.
    if let Some(editor) = &editor {
        crate::editor::save_view(editor);
    }
    controller.click_in_tree(&root.join("d.rs"), false);
    let previewed = has(controller, "d.rs", true);
    controller.click_in_tree(&root.join("d.rs"), true);
    check(
        "a double click in the tree keeps the previewed file's tab",
        previewed && controller.tab_count() == before + 2 && has(controller, "d.rs", false),
        tabs_of(controller),
    );
    controller.click_in_tree(&root.join("a.rs"), false);
    controller.open_file(&root.join("a.rs"));
    check(
        "opening the previewed file for good keeps its tab",
        controller.tab_count() == before + 3 && has(controller, "a.rs", false),
        tabs_of(controller),
    );
}

/// ⌘T opens the page of a new tab, which offers what a tab can show and gives way to a file opened from it.
fn new_tab_page(controller: &WindowController, root: &Path) {
    let before = controller.tab_count();
    controller.open_new_tab();
    let entries = controller.new_tab_page_entries().unwrap_or_default();
    let offered = controller.inspect().new_tab_items;
    check(
        "⌘T opens the page of a new tab with what a tab can show",
        controller.tab_count() == before + 1 && !entries.is_empty() && entries == offered,
        format!("{entries:?}"),
    );
    controller.open_file(&root.join("e.rs"));
    check(
        "a file opened from the page takes its place",
        controller.tab_count() == before + 1
            && has(controller, "e.rs", false)
            && controller.new_tab_page_entries().is_none(),
        tabs_of(controller),
    );
    controller.close_tab(
        controller
            .tab_with_key(&root.join("e.rs").to_string_lossy())
            .unwrap_or(-1),
    );
}

fn turned_off(controller: &WindowController, root: &Path) {
    if let Err(error) = Settings::load_user().set(&keys::PREVIEW_TABS, &false) {
        check("preview tabs can be turned off", false, error.to_string());
        return;
    }
    let before = controller.tab_count();
    controller.click_in_tree(&root.join("e.rs"), false);
    check(
        "with preview tabs off a single click opens an ordinary tab",
        controller.tab_count() == before + 1 && has(controller, "e.rs", false),
        tabs_of(controller),
    );
}
