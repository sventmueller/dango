//! The `window-commands` scenario: a folder's window answers the commands of its menus and lists. ⇧⌘] and ⇧⌘[ walk
//! the tabs, the activity bar leaves a symbol out and brings it back, the search and symbol commands open their
//! panels, "+" asks for the menu of new tabs and the agent settings open on their tab. The folder tree's context menu
//! copies a path, opens a file beside, as preview or new, makes a folder, renames a file and says why a name is taken;
//! a click opens the row. The search panel takes typing, the arrow down and Return from its field, Return and Esc in
//! its results, follows its options, opens its filters and replace field, opens and replaces from a row's menu and a
//! click, folds a file and asks before replacing everything.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSApplication, NSBackingStoreType, NSButton, NSControl,
    NSControlTextDidChangeNotification, NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSMenuItem, NSOutlineView,
    NSPasteboard, NSPasteboardTypeString, NSResponder, NSTabViewController, NSTextField, NSTextView, NSView, NSWindow,
    NSWindowDidEnterFullScreenNotification, NSWindowStyleMask, NSWindowWillExitFullScreenNotification,
};
use objc2_foundation::{NSNotificationCenter, NSObject, NSPoint, NSRect, NSSize, NSString};

use super::helpers::{alert_texts, answer_alert, answer_alert_saying, in_any_mode_after, wait_until, window_of};
use super::settings_pages::all_in;
use super::{check, finish};
use crate::util::{constant, main_after, ns};
use crate::window::{Panel, WindowController, search_selftest as search};

const NAME: &str = "window-commands";
const STEP: Duration = Duration::from_millis(300);
const MAIN: &str = "fn main() {\n    let needle = 1;\n    println!(\"Needle {needle}\");\n}\n";
const LIB: &str = "pub fn needle() {}\n";
const SVG: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><rect width=\"8\" height=\"8\"/></svg>\n";
const RETURN: u16 = 36;
const ESCAPE: u16 = 53;

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = ide_core::paths::home().join("commands-project");
    for (path, text) in [
        ("src/main.rs", MAIN),
        ("src/lib.rs", LIB),
        ("src/other.rs", "pub fn other() {}\n"),
        ("notes.md", "# Notes\n"),
        ("pic.svg", SVG),
    ] {
        let file = root.join(path);
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&root));
        let _ = std::fs::write(file, text);
    }
    let root = root.canonicalize().unwrap_or(root);
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    main_after(Duration::from_millis(800), move || {
        let Some(controller) = window_of(&root) else {
            check("the folder opens in a window", false, "");
            return finish(NAME);
        };
        controller.open_file(&root.join("src/main.rs"));
        controller.open_file(&root.join("src/lib.rs"));
        main_after(STEP, move || tabs(root));
    });
}

fn controller(root: &Path) -> Option<Retained<WindowController>> {
    window_of(root)
}

/// Sends `action` to the window's controller as its menu item does, from `sender`.
fn send(controller: &WindowController, action: Sel, sender: Option<&AnyObject>) -> bool {
    let app = NSApplication::sharedApplication(MainThreadMarker::from(controller));
    let target: &AnyObject = controller.as_ref();
    // SAFETY: The controller answers the action, which takes a sender or none.
    unsafe { app.sendAction_to_from(action, Some(target), sender) }
}

fn tabs(root: PathBuf) {
    let Some(c) = controller(&root) else {
        return finish(NAME);
    };
    let front = c.front_title();
    send(&c, sel!(idePreviousTab:), None);
    let previous = c.front_title();
    send(&c, sel!(ideNextTab:), None);
    let next = c.front_title();
    check(
        "the previous and the next tab come to the front with their commands",
        front.as_deref() == Some("lib.rs")
            && previous.as_deref() == Some("main.rs")
            && next.as_deref() == Some("lib.rs"),
        format!("{front:?} {previous:?} {next:?}"),
    );
    let mtm = MainThreadMarker::from(&*c);
    let item = NSMenuItem::new(mtm);
    item.setTag(Panel::Git.tag());
    let hidden = || {
        ide_core::settings::Settings::load_user()
            .get(&ide_core::settings::keys::HIDDEN_PANELS)
            .contains(&Panel::Git.id().to_string())
    };
    let before = hidden();
    send(&c, sel!(ideToggleActivitySymbol:), Some(&item));
    let left_out = hidden();
    send(&c, sel!(ideToggleActivitySymbol:), Some(&item));
    check(
        "the activity bar leaves a symbol out and brings it back",
        !before && left_out && !hidden(),
        "",
    );
    c.record_new_tab_menu();
    send(&c, sel!(newWindowForTab:), None);
    check(
        "the window tab bar's \"+\" asks for the menu of new tabs",
        c.new_tab_menu_recorded().is_some(),
        "",
    );
    send(&c, sel!(ideOpenAgentSettings:), None);
    let tab = crate::settings::window()
        .and_then(|w| w.contentViewController())
        .and_then(|v| v.downcast::<NSTabViewController>().ok())
        .map(|t| t.selectedTabViewItemIndex());
    check(
        "the agent's settings open on their tab",
        tab == Some(3),
        format!("{tab:?}"),
    );
    if let Some(window) = crate::settings::window() {
        window.orderOut(None);
    }
    main_after(STEP, move || symbols(root));
}

/// Whether a panel shows with `query` typed into its field.
fn panel_with_query(query: &str) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    NSApplication::sharedApplication(mtm).windows().iter().any(|window| {
        window.isVisible()
            && window.downcast_ref::<objc2_app_kit::NSPanel>().is_some()
            && all_in::<NSTextField>(&window)
                .iter()
                .any(|f| f.isEditable() && f.stringValue().to_string() == query)
    })
}

fn symbols(root: PathBuf) {
    let Some(c) = controller(&root) else {
        return finish(NAME);
    };
    send(&c, sel!(ideProjectSymbols:), None);
    let shown = panel_with_query("#");
    c.close_quick_open();
    check(
        "the command for the folder's symbols opens Open Quickly on its symbols",
        shown && !panel_with_query("#"),
        "",
    );
    main_after(STEP, move || files_menu(root));
}

/// The row of `path` in `outline`, whose items are the paths.
fn row_of(outline: &NSOutlineView, path: &Path) -> Option<isize> {
    let wanted = path.to_string_lossy();
    (0..outline.numberOfRows()).find(|row| {
        outline
            .itemAtRow(*row)
            .and_then(|item| item.downcast::<NSString>().ok())
            .is_some_and(|item| item.to_string() == wanted)
    })
}

/// The context menu of `row` as a right click on it opens it, filled by its delegate.
fn menu_at(outline: &NSOutlineView, row: isize) -> Option<Retained<NSMenu>> {
    let window = outline.window()?;
    let rect = outline.rectOfRow(row);
    let point = outline.convertPoint_toView(
        NSPoint::new(
            rect.origin.x + rect.size.width / 2.0,
            rect.origin.y + rect.size.height / 2.0,
        ),
        None,
    );
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::RightMouseDown,
        point,
        NSEventModifierFlags::empty(),
        0.0,
        window.windowNumber(),
        None,
        0,
        1,
        1.0,
    )?;
    let menu = outline.menuForEvent(&event)?;
    let delegate = menu.delegate()?;
    // SAFETY: The menu's own delegate, which answers `menuNeedsUpdate:` as AppKit asks before showing it.
    let () = unsafe { msg_send![&*delegate, menuNeedsUpdate: &*menu] };
    Some(menu)
}

/// Chooses the item titled `title` of `menu`; whether there was one that could be chosen.
fn pick(menu: &NSMenu, title: &str) -> bool {
    let index = menu.indexOfItemWithTitle(&ns(title));
    if index < 0 || !menu.itemAtIndex(index).is_some_and(|item| item.isEnabled()) {
        return false;
    }
    menu.performActionForItemAtIndex(index);
    true
}

fn titles(menu: &NSMenu) -> Vec<String> {
    menu.itemArray().iter().map(|item| item.title().to_string()).collect()
}

/// Types `name` into the field of the alert titled `title` once it runs and confirms it.
fn name_in_alert(title: &'static str, name: &'static str) {
    in_any_mode_after(Duration::from_millis(500), move || {
        if !alert_texts().iter().any(|text| text == title) {
            return;
        }
        let mtm = MainThreadMarker::new();
        if let Some(alert) = mtm.and_then(|mtm| NSApplication::sharedApplication(mtm).modalWindow()) {
            for field in all_in::<NSTextField>(&alert).iter().filter(|f| f.isEditable()) {
                field.setStringValue(&ns(name));
            }
        }
        answer_alert(NSAlertFirstButtonReturn);
    });
}

fn pasteboard() -> String {
    NSPasteboard::generalPasteboard()
        .stringForType(constant!(NSPasteboardTypeString))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Opens the context menu of `path`'s row in the folder tree and chooses `title`.
fn tree_command(c: &WindowController, path: &Path, title: &str) -> Option<Vec<String>> {
    let outline = c.file_tree_outline()?;
    if let Some(parent) = path.parent()
        && let Some(row) = row_of(&outline, parent)
        && let Some(item) = outline.itemAtRow(row)
    {
        // SAFETY: The item is one the outline's data source handed it, shown in `row`.
        unsafe { outline.expandItem(Some(&item)) };
    }
    let row = row_of(&outline, path)?;
    let menu = menu_at(&outline, row)?;
    let shown = titles(&menu);
    pick(&menu, title).then_some(shown)
}

fn files_menu(root: PathBuf) {
    let Some(c) = controller(&root) else {
        return finish(NAME);
    };
    let lib = root.join("src/lib.rs");
    let menu = tree_command(&c, &lib, "Pfad kopieren");
    let absolute = pasteboard();
    tree_command(&c, &lib, "Relativen Pfad kopieren");
    check(
        "the folder tree's menu copies a file's path and its path in the folder",
        menu.as_ref().is_some_and(|m| m.iter().any(|t| t == "Daneben öffnen"))
            && absolute == lib.to_string_lossy()
            && pasteboard() == "src/lib.rs",
        format!("{menu:?} {absolute}"),
    );
    c.open_file(&root.join("src/main.rs"));
    let beside = tree_command(&c, &lib, "Daneben öffnen").is_some();
    check(
        "\"Daneben öffnen\" brings the file to the front",
        beside && c.front_title().as_deref() == Some("lib.rs"),
        format!("{:?}", c.tab_titles()),
    );
    let svg = root.join("pic.svg");
    let preview = tree_command(&c, &svg, "Als Vorschau öffnen");
    main_after(STEP, move || {
        let Some(c) = controller(&root) else {
            return finish(NAME);
        };
        check(
            "an SVG's menu offers to open it as preview, which opens it",
            preview.is_some() && c.tab_titles().iter().any(|t| t.contains("pic.svg")),
            format!("{:?}", c.tab_titles()),
        );
        name_in_alert("Umbenennen", "other2.rs");
        let renamed = tree_command(&c, &root.join("src/other.rs"), "Umbenennen …").is_some();
        main_after(STEP, move || rename_taken(root, renamed));
    });
}

fn rename_taken(root: PathBuf, renamed: bool) {
    let Some(c) = controller(&root) else {
        return finish(NAME);
    };
    let renamed_to = root.join("src/other2.rs");
    check(
        "\"Umbenennen …\" asks for the name and renames the file",
        renamed && renamed_to.is_file() && !root.join("src/other.rs").exists(),
        "",
    );
    name_in_alert("Umbenennen", "main.rs");
    let said = answer_alert_saying(
        Duration::from_millis(1200),
        "„main.rs“ gibt es schon.".to_string(),
        NSAlertFirstButtonReturn,
    );
    let asked = tree_command(&c, &renamed_to, "Umbenennen …").is_some();
    check(
        "a name that is taken is refused and the alert says so",
        asked && said.get() && renamed_to.is_file(),
        "",
    );
    let src = root.join("src");
    name_in_alert("Neuer Ordner", "neu");
    let folder = tree_command(&c, &src, "Neuer Ordner …").is_some();
    name_in_alert("Neue Datei", "a.txt");
    let file = tree_command(&c, &src, "Neue Datei …").is_some();
    main_after(STEP, move || {
        let Some(c) = controller(&root) else {
            return finish(NAME);
        };
        check(
            "the menu of a folder makes a folder and a file in it, and the file opens",
            folder
                && file
                && src.join("neu").is_dir()
                && src.join("a.txt").is_file()
                && c.front_title().as_deref() == Some("a.txt"),
            format!("{:?}", c.front_title()),
        );
        tree_click(root);
    });
}

/// A click on a row of the folder tree opens it, a double click too.
fn tree_click(root: PathBuf) {
    let Some(c) = controller(&root) else {
        return finish(NAME);
    };
    let notes = root.join("notes.md");
    let opened = c.file_tree_outline().is_some_and(|outline| {
        let Some(row) = row_of(&outline, &notes) else {
            return false;
        };
        menu_at(&outline, row);
        let clicked = outline.action().is_some_and(|action| {
            let app = NSApplication::sharedApplication(MainThreadMarker::from(&*c));
            // SAFETY: The outline's own action and target, with the outline as the sender, as its click sends.
            unsafe { app.sendAction_to_from(action, outline.target().as_deref(), Some(&outline)) }
        });
        let doubled = outline.doubleAction().is_some_and(|action| {
            let app = NSApplication::sharedApplication(MainThreadMarker::from(&*c));
            // SAFETY: The outline's own double action and target, with the outline as the sender.
            unsafe { app.sendAction_to_from(action, outline.target().as_deref(), Some(&outline)) }
        });
        clicked && doubled
    });
    main_after(STEP, move || {
        let Some(c) = controller(&root) else {
            return finish(NAME);
        };
        check(
            "a click and a double click on a file's row open it",
            opened && c.front_title().is_some_and(|t| t.contains("notes")),
            format!("{:?}", c.front_title()),
        );
        searching(root);
    });
}

fn panel(root: &Path) -> Option<Retained<crate::window::SearchPanel>> {
    controller(root)?.search_panel()
}

fn settled(root: &Path) -> bool {
    panel(root).is_some_and(|p| {
        let seen = search::seen(&p);
        seen.settled && !seen.files.is_empty()
    })
}

fn matches(root: &Path) -> usize {
    panel(root).map_or(0, |p| search::seen(&p).files.iter().map(|f| f.1).sum())
}

/// The field the window's keyboard focus is in, and its field editor.
fn focused_field(window: &NSWindow) -> Option<(Retained<NSTextField>, Retained<NSTextView>)> {
    let editor = window.firstResponder()?.downcast::<NSTextView>().ok()?;
    let delegate = editor.delegate()?;
    let object: Retained<AnyObject> = delegate.into();
    Some((object.downcast::<NSTextField>().ok()?, editor))
}

/// Types `text` into `field` as the keyboard does, which tells the field's delegate.
fn type_into(field: &NSTextField, text: &str) {
    field.setStringValue(&ns(text));
    let name = constant!(NSControlTextDidChangeNotification);
    // SAFETY: The notification a field posts while it is edited, with the field as its object.
    unsafe { NSNotificationCenter::defaultCenter().postNotificationName_object(name, Some(field)) };
}

fn key(view: &NSView, code: u16, characters: &str) {
    let Some(window) = view.window() else { return };
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags::empty(),
        0.0,
        window.windowNumber(),
        None,
        &ns(characters),
        &ns(characters),
        false,
        code,
    );
    if let Some(event) = event {
        view.keyDown(&event);
    }
}

fn outline_of(root: &Path) -> Option<Retained<NSOutlineView>> {
    let window = controller(root)?.window()?;
    all_in::<NSControl>(&window)
        .into_iter()
        .find(|c| c.action() == Some(sel!(ideSearchRowClicked:)))
        .and_then(|c| c.downcast::<NSOutlineView>().ok())
}

fn searching(root: PathBuf) {
    let Some(c) = controller(&root) else {
        return finish(NAME);
    };
    send(&c, sel!(ideSearchProject:), None);
    let window = c.window();
    let field = window.as_deref().and_then(focused_field);
    check(
        "the search command opens the search panel with the focus in its field",
        c.inspect().panel == Some(Panel::Search) && field.is_some(),
        "",
    );
    let Some((field, _)) = field else { return finish(NAME) };
    type_into(&field, "needle");
    let ready = root.clone();
    wait_until(
        move || settled(&ready),
        move |found| {
            let total = matches(&root);
            check(
                "typing in the field searches the folder",
                found && total >= 3,
                format!("{total}"),
            );
            field_keys(root, total);
        },
    );
}

/// The arrow down moves from the field into the results, Return there opens the match, Esc empties the field.
fn field_keys(root: PathBuf, total: usize) {
    let Some(c) = controller(&root) else {
        return finish(NAME);
    };
    let (Some(window), Some(panel), Some(outline)) = (c.window(), panel(&root), outline_of(&root)) else {
        return finish(NAME);
    };
    let Some((field, editor)) = focused_field(&window) else {
        check("the search field keeps the focus", false, "");
        return finish(NAME);
    };
    let control: &NSControl = &field;
    // SAFETY: The delegate method the field editor calls for a key's command, with the field and its editor.
    let moved: bool =
        unsafe { msg_send![&*panel, control: control, textView: &*editor, doCommandBySelector: sel!(moveDown:)] };
    let in_results = window
        .firstResponder()
        .is_some_and(|r| std::ptr::eq(Retained::as_ptr(&r).cast::<()>(), Retained::as_ptr(&outline).cast()));
    check(
        "the arrow down in the field moves into the results",
        moved && in_results && outline.selectedRow() >= 0,
        "",
    );
    outline.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(1), false);
    key(&outline, RETURN, "\r");
    let opened = c.front_title();
    key(&outline, ESCAPE, "\u{1b}");
    let seen = search::seen(&panel);
    check(
        "Return on a match opens its file and Esc in the results empties the search",
        opened.as_deref().is_some_and(|t| t.ends_with(".rs")) && !seen.results_shown,
        format!("{opened:?} {total}"),
    );
    search::type_query(&panel, "needle");
    let ready = root.clone();
    wait_until(move || settled(&ready), move |_| options(root, total));
}

fn visible_fields(root: &Path) -> usize {
    let Some(window) = controller(root).and_then(|c| c.window()) else {
        return 0;
    };
    all_in::<NSTextField>(&window)
        .iter()
        .filter(|f| f.isEditable() && !f.isHiddenOrHasHiddenAncestor())
        .count()
}

fn button(root: &Path, action: Sel) -> Option<Retained<NSButton>> {
    let window = controller(root)?.window()?;
    all_in::<NSButton>(&window)
        .into_iter()
        .find(|b| b.action() == Some(action))
}

fn click(root: &Path, action: Sel) -> bool {
    button(root, action).is_some_and(|b| {
        // SAFETY: A click with no sender, which the panel's actions take.
        unsafe { b.performClick(None) };
        true
    })
}

/// The case option narrows the matches, the filters and the replace field open with their buttons.
fn options(root: PathBuf, total: usize) {
    let case = click(&root, sel!(ideSearchOptionChanged:));
    let ready = root.clone();
    wait_until(
        move || settled(&ready) && matches(&ready) < total,
        move |narrowed| {
            check(
                "the case option narrows the matches to the written case",
                case && narrowed,
                format!("{} of {total}", matches(&root)),
            );
            click(&root, sel!(ideSearchOptionChanged:));
            let fields = visible_fields(&root);
            let disclosed = click(&root, sel!(ideSearchDisclose:));
            let filters = visible_fields(&root);
            let toggled = click(&root, sel!(ideSearchToggleReplace:));
            let with_replace = visible_fields(&root);
            check(
                "the filters' chevron shows the two filters and the replace chevron the replace field",
                disclosed && toggled && filters == fields + 2 && with_replace == filters + 1,
                format!("{fields} {filters} {with_replace}"),
            );
            let window = controller(&root).and_then(|c| c.window());
            if let Some((replace, _)) = window.as_deref().and_then(focused_field) {
                type_into(&replace, "pin");
            }
            let ready = root.clone();
            wait_until(move || settled(&ready), move |_| row_menu(root));
        },
    );
}

/// A match's menu replaces it and opens it; a click on a match opens it too.
fn row_menu(root: PathBuf) {
    let Some(outline) = outline_of(&root) else {
        return finish(NAME);
    };
    let before = matches(&root);
    let menu = menu_at(&outline, 1);
    let shown = menu.as_ref().map(|m| titles(m)).unwrap_or_default();
    let replaced = menu.is_some_and(|m| pick(&m, "Ersetzen"));
    let ready = root.clone();
    wait_until(
        move || settled(&ready) && matches(&ready) < before,
        move |gone| {
            check(
                "with the replace field open a match's menu replaces it, in its open editor too",
                replaced && gone && panel(&root).is_some_and(|p| search::seen(&p).can_replace_all),
                format!("{shown:?} {before} → {}", matches(&root)),
            );
            undo_replaced(&root);
            let Some(outline) = outline_of(&root) else {
                return finish(NAME);
            };
            let opened = menu_at(&outline, 1).is_some_and(|m| pick(&m, "Öffnen"));
            let front = controller(&root).and_then(|c| c.front_title());
            let clicked = outline.action().is_some_and(|action| {
                let app = NSApplication::sharedApplication(MainThreadMarker::from(&*outline));
                // SAFETY: The outline's own action and target, with the outline as the sender, as its click sends.
                unsafe { app.sendAction_to_from(action, outline.target().as_deref(), Some(&outline)) }
            });
            let doubled = outline.doubleAction().is_some_and(|action| {
                let app = NSApplication::sharedApplication(MainThreadMarker::from(&*outline));
                // SAFETY: The outline's own double action and target, with the outline as the sender.
                unsafe { app.sendAction_to_from(action, outline.target().as_deref(), Some(&outline)) }
            });
            check(
                "a match's menu, a click and a double click open its file",
                opened && clicked && doubled && front.is_some_and(|t| t.ends_with(".rs")),
                "",
            );
            fold_and_replace_all(root);
        },
    );
}

/// Takes the replacement back in the editor it went into, which leaves no unsaved change to ask about on quitting.
fn undo_replaced(root: &Path) {
    let Some(c) = controller(root) else { return };
    for title in c.tab_titles() {
        if let Some(editor) = super::helpers::editor(&c, &title)
            && super::helpers::text_of(&editor).contains("pin")
            && let Some(undo) = editor.undoManager()
        {
            undo.undo();
        }
    }
}

fn fold_and_replace_all(root: PathBuf) {
    let Some(outline) = outline_of(&root) else {
        return finish(NAME);
    };
    let rows = outline.numberOfRows();
    if let Some(item) = outline.itemAtRow(0) {
        // SAFETY: The item is one the outline's data source handed it, shown in row 0.
        unsafe { outline.collapseItem(Some(&item)) };
    }
    let folded = outline.numberOfRows() < rows;
    check(
        "a file's matches fold away",
        folded,
        format!("{rows} → {}", outline.numberOfRows()),
    );
    let pressed = click(&root, sel!(ideSearchReplaceAll:));
    let ready = root.clone();
    wait_until(
        move || controller(&ready).is_some_and(|c| c.has_sheet()),
        move |asked| {
            let before = std::fs::read_to_string(root.join("src/main.rs")).unwrap_or_default();
            if let Some(window) = controller(&root).and_then(|c| c.window())
                && let Some(sheet) = window.attachedSheet()
            {
                window.endSheet_returnCode(&sheet, NSAlertSecondButtonReturn);
            }
            main_after(STEP, move || {
                let after = std::fs::read_to_string(root.join("src/main.rs")).unwrap_or_default();
                check(
                    "\"Alle ersetzen\" asks first, and Abbrechen replaces nothing",
                    pressed && asked && before == after,
                    "",
                );
                full_screen(root);
            });
        },
    );
}

define_class!(
    // SAFETY: NSWindow has no subclassing requirements and the class adds nothing to it.
    #[unsafe(super(NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESelftestNSToolbarFullScreenWindow"]
    struct StandInToolbarWindow;
);

define_class!(
    // SAFETY: NSView has no subclassing requirements and the class adds nothing to it.
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESelftestNSTitlebarBackgroundView"]
    struct StandInBackground;
);

/// In full screen AppKit shows the toolbar in a window of its own whose background covers the sidebar; this stands in
/// for that window, which the locked screen of a self-test never gets, with the background over the sidebar's column.
fn stand_in_toolbar(over: &NSWindow, sidebar_width: f64) -> Option<(Retained<NSWindow>, Retained<NSView>)> {
    let mtm = MainThreadMarker::from(over);
    let frame = over.frame();
    // SAFETY: NSWindow's designated initializer, called once on the newly allocated object.
    let window: Retained<StandInToolbarWindow> = unsafe {
        msg_send![
            StandInToolbarWindow::alloc(mtm),
            initWithContentRect: frame,
            styleMask: NSWindowStyleMask::Titled,
            backing: NSBackingStoreType::Buffered,
            defer: true
        ]
    };
    let window: Retained<NSWindow> = window.into_super();
    // SAFETY: The window was made just above and is not shown, so nothing else releases it.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setFrame_display(frame, false);
    let column = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(sidebar_width, 40.0));
    // SAFETY: NSView's designated initializer, called once on the newly allocated object.
    let background: Retained<StandInBackground> =
        unsafe { msg_send![StandInBackground::alloc(mtm), initWithFrame: column] };
    let background: Retained<NSView> = background.into_super();
    // SAFETY: Asking for the superview only reads; the frame view holds the content view.
    let frame_view = window.contentView().and_then(|c| unsafe { c.superview() })?;
    frame_view.addSubview(&background);
    Some((window, background))
}

fn post(name: &NSString, window: &NSWindow) {
    // SAFETY: A notification the window posts itself, with the window as its object.
    unsafe { NSNotificationCenter::defaultCenter().postNotificationName_object(name, Some(window)) };
}

/// Entering full screen clears the toolbar's background above the sidebar, which leaving brings back; outside full
/// screen nothing is cleared.
fn full_screen(root: PathBuf) {
    let Some(c) = controller(&root) else {
        return finish(NAME);
    };
    let Some(window) = c.window() else { return finish(NAME) };
    let Some((toolbar, background)) = stand_in_toolbar(&window, c.inspect().sidebar_width) else {
        check("a stand-in for the toolbar's window can be made", false, "");
        return finish(NAME);
    };
    post(constant!(NSWindowDidEnterFullScreenNotification), &window);
    let kept = background.alphaValue();
    background.setAlphaValue(0.0);
    post(constant!(NSWindowWillExitFullScreenNotification), &window);
    let back = background.alphaValue();
    check(
        "outside full screen the toolbar's background stays, and leaving full screen brings back the one above the \
         sidebar",
        kept == 1.0 && back == 1.0,
        format!("{kept} {back}"),
    );
    toolbar.close();
    finish(NAME);
}
