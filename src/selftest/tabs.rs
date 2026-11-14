//! The `tabs` scenario: preview tabs, which the user turns on (E62). A single click in the folder tree opens a file in
//! the preview tab, the next one replaces it, a double click, typing or a double click on the tab keeps it, and with
//! the setting off every file gets a tab of its own.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::settings::{Settings, keys};
use objc2::rc::Retained;
use splat_i18n::t;

use super::{check, finish, helpers, snapshot, type_into};
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
    if let Err(error) = Settings::load_user().set(&keys::PREVIEW_TABS, &true) {
        check("preview tabs can be turned on", false, error.to_string());
    }
    let texts: Vec<String> = FILES
        .iter()
        .map(|name| format!("// {name}\nfn main() {{}}\n"))
        .collect();
    let files: Vec<(&str, &str)> = FILES.iter().copied().zip(texts.iter().map(String::as_str)).collect();
    helpers::open_project(SCENARIO, "tabs-project", &files, |controller| {
        let root = helpers::root("tabs-project");
        let rows = isize::try_from(FILES.len()).unwrap_or(0);
        helpers::wait_until(move || controller.tree_rows() >= rows, move |_| first_preview(root));
    });
}

/// Long enough for the window to draw what changed before a picture, which has no state of its own to wait for.
const DRAWN: Duration = Duration::from_millis(500);

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
    helpers::after(DRAWN, move || replaced(root));
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
    turned_off(&controller, &root);
    helpers::after(DRAWN, move || {
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
    let listed: Vec<bool> = controller
        .tab_windows()
        .iter()
        .map(|window| !window.isExcludedFromWindowsMenu())
        .collect();
    check(
        "the Window menu and the Dock list the folder once, by the tab in front",
        listed.len() > 6
            && listed.iter().filter(|shown| **shown).count() == 1
            && controller
                .window()
                .is_some_and(|window| !window.isExcludedFromWindowsMenu()),
        format!("{listed:?}"),
    );
    plus(root);
}

/// The tab bar's "+" and ⌘T: a click opens the menu of what a tab can show below the "+", and no tab before the user
/// picks; the entry chosen opens in a new tab straight away.
fn plus(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    controller.record_new_tab_menu();
    let before = controller.tab_count();
    let clicked = controller.click_plus();
    let probe = root.clone();
    let recorded = move || helpers::window_of(&probe).and_then(|c| c.new_tab_menu_recorded());
    helpers::wait_for(recorded, move |shown| {
        let Some(controller) = window_of(&root) else { return };
        check(
            "a click on the tab bar's + opens the menu of new tabs below it, without a tab",
            clicked && shown == Some(true) && controller.tab_count() == before,
            format!("menu {shown:?}, {}", tabs_of(&controller)),
        );
        controller.open_new_tab_menu();
        let shown = controller.new_tab_menu_recorded();
        check(
            "⌘T opens the same menu below the +, without a tab",
            shown == Some(true) && controller.tab_count() == before,
            format!("menu {shown:?}, {}", tabs_of(&controller)),
        );
        let titles = controller.plus_menu_titles();
        let chosen = controller.choose_from_plus_menu(&[t("Terminal")]);
        check(
            "the menu offers what a tab can show, and its entry opens a new tab straight away",
            titles == controller.inspect().new_tab_items
                && chosen
                && controller.tab_count() == before + 1
                && controller
                    .tab_titles()
                    .last()
                    .is_some_and(|title| title.starts_with(t("Terminal"))),
            format!("{titles:?}, {}", tabs_of(&controller)),
        );
        every_plus_entry(&controller, &root);
        super::git::pictures(root, "tabs-icons", navigation);
    });
}

/// The UTF-16 offset of the start of line `line`, counted from 0, in the text of `lines` lines `line N`.
fn line_start(line: usize) -> usize {
    (0..line).map(|n| format!("line {n}\n").len()).sum()
}

/// Where the cursor of the editor in front is, by file name and offset.
fn front_place(controller: &WindowController) -> Option<(String, usize)> {
    let view = crate::editor::editor_of_tab(controller, controller.selected_tab()?)?;
    let name = view.document().path()?.file_name()?.to_string_lossy().into_owned();
    Some((name, view.selectedRange().location))
}

/// Back and forward go through the places left for another file or a far jump, from the toolbar and the Go menu.
fn navigation(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let long = root.join("long.txt");
    let other = root.join("other.txt");
    let text: String = (0..300).map(|n| format!("line {n}\n")).collect();
    let _ = std::fs::write(&long, &text);
    let _ = std::fs::write(&other, "other\n");
    let put = |path: &Path, offset: usize| {
        controller.open_file(path);
        if let Some(view) = crate::editor::editor_of(&controller, path) {
            if let Some(window) = view.window() {
                window.makeFirstResponder(Some(&view));
            }
            view.setSelectedRange(objc2_foundation::NSRange::new(offset, 0));
        }
    };
    put(&long, line_start(5));
    put(&long, line_start(6));
    put(&long, line_start(200));
    put(&other, 0);
    let group = controller.window().and_then(|window| {
        let toolbar = window.toolbar()?;
        toolbar
            .items()
            .iter()
            .find(|item| item.itemIdentifier().to_string() == crate::navigation::TOOLBAR_ID)
            .and_then(|item| item.downcast::<objc2_app_kit::NSToolbarItemGroup>().ok())
    });
    let enabled = |group: &Option<Retained<objc2_app_kit::NSToolbarItemGroup>>| -> Vec<bool> {
        group
            .as_ref()
            .map(|g| g.subitems().iter().map(|item| item.isEnabled()).collect())
            .unwrap_or_default()
    };
    let before = enabled(&group);
    let mut walked = Vec::new();
    for back in [true, true, false, false] {
        crate::navigation::go(&controller, back);
        walked.push(front_place(&controller));
    }
    let name = |n: &str, at: usize| Some((n.to_string(), at));
    check(
        "back and forward go through the places left for another file or a far jump, typing in between leaves none",
        walked
            == [
                name("long.txt", line_start(200)),
                name("long.txt", line_start(6)),
                name("long.txt", line_start(200)),
                name("other.txt", 0),
            ],
        format!("{walked:?}"),
    );
    check(
        "the toolbar's back is enabled once there is a place to go back to, forward only after going back",
        before == [true, false] && enabled(&group) == [true, false],
        format!("before {before:?}, after {:?}", enabled(&group)),
    );
    let ids: Vec<String> = controller
        .window()
        .and_then(|w| w.toolbar())
        .map(|t| t.items().iter().map(|i| i.itemIdentifier().to_string()).collect())
        .unwrap_or_default();
    check(
        "the toolbar holds back and forward, the branch and Open Quickly beside the activity",
        [
            crate::navigation::TOOLBAR_ID,
            ide_ui_git::TOOLBAR_ID,
            "ide.quickOpen",
            "ide.activity",
        ]
        .iter()
        .all(|id| ids.iter().any(|shown| shown == id)),
        format!("{ids:?}"),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "tabs-toolbar");
    }
    finish(SCENARIO);
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

/// Every entry of the "+"'s menu opens what it names, chosen in an untrusted folder, where no agent starts; the
/// agents listed are the installed ones, directly in the menu, the own agent last, a thread in a worktree below them.
fn every_plus_entry(controller: &WindowController, root: &Path) {
    crate::project::distrust(root);
    let entries = controller.plus_menu_entries();
    let mut results = Vec::new();
    for (path, action, enabled) in &entries {
        if !enabled {
            results.push((path.join(" ▸ "), "disabled".to_string(), true));
            continue;
        }
        let tabs = controller.tab_count();
        let titles: Vec<&str> = path.iter().map(String::as_str).collect();
        let chosen = controller.choose_from_plus_menu(&titles);
        let (outcome, ok) = match action.as_str() {
            "ideOpenWelcome:" => {
                let front = controller.front_title();
                (format!("front {front:?}"), front.as_deref() == Some(t("Willkommen")))
            }
            "ideOpenAgentsSettings:" | "ideOpenAgentSettings:" => {
                let shown = settings_shown();
                close_settings();
                (format!("settings shown {shown}"), shown)
            }
            _ => (
                format!("{tabs} → {} tabs", controller.tab_count()),
                controller.tab_count() == tabs + 1,
            ),
        };
        results.push((path.join(" ▸ "), outcome, chosen && ok));
    }
    crate::project::trust(root);
    let failed: Vec<&(String, String, bool)> = results.iter().filter(|(_, _, ok)| !ok).collect();
    check(
        "every entry of the +'s menu opens what it names in a new tab, the welcome page or the settings",
        !entries.is_empty() && failed.is_empty(),
        format!("{failed:?} of {results:?}"),
    );
    let is_agent = |(path, action, _): &(Vec<String>, String, bool)| {
        matches!(action.as_str(), "ideNewClaudeThread:" | "ideNewAcpThread:")
            || path.first().map(String::as_str) == Some(t("Eigener Agent"))
    };
    let agents: Vec<String> = entries
        .iter()
        .filter(|entry| is_agent(entry))
        .map(|(path, _, _)| path[0].clone())
        .fold(Vec::new(), |mut names, name| {
            if names.last() != Some(&name) {
                names.push(name);
            }
            names
        });
    let last_agent = entries.iter().rposition(is_agent);
    let worktree = entries
        .iter()
        .position(|(path, _, _)| path.first().map(String::as_str) == Some(t("In eigenem Worktree")));
    check(
        "the agents stand directly in the +'s menu, a thread in a worktree below them",
        entries
            .iter()
            .filter(|entry| is_agent(entry))
            .all(|(path, _, _)| path.len() == 1 || path[0] == t("Eigener Agent"))
            && last_agent
                .zip(worktree)
                .is_none_or(|(agent, worktree)| worktree > agent),
        format!("{entries:?}"),
    );
    let installed: Vec<String> = ide_ui_agent::installed(&ide_ui_agent::Agent::Claude)
        .then(|| "Claude".to_string())
        .into_iter()
        .chain(ide_ui_agent::installed_presets().into_iter().map(|(_, name)| name))
        .chain(ide_ui_agent::installed(&ide_ui_agent::Agent::Own).then(|| t("Eigener Agent").to_string()))
        .collect();
    check(
        "the menu lists only installed agents, the own agent last",
        agents == installed,
        format!("{agents:?}, installed {installed:?}"),
    );
}

fn settings_shown() -> bool {
    crate::settings::window().is_some_and(|window| window.isVisible())
}

fn close_settings() {
    if let Some(window) = crate::settings::window() {
        window.close();
    }
}
