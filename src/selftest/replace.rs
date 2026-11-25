//! The `replace-folder` scenario (E56): a folder opened in a window that shows another one takes its place. With
//! unsaved text the window asks first and offers a new window, closing everything, or cancelling; a window that waits
//! for the answer to a sheet keeps its folder, and so does one whose terminal runs a program until the user agrees to
//! end it. The question counts every tab of the folder's tab group, and windows of different folders never merge.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_process::Supervisor;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn, NSButton, NSModalResponse,
    NSTextField, NSView, NSWindow,
};
use objc2_foundation::NSRange;
use splat_i18n::t;

use super::helpers::{self, editor, type_at, wait_until, window_of};
use super::{check, finish, give_up, give_up_saying, snapshot};
use crate::terminal::inspect;
use crate::window::{self, WindowController};

const SCENARIO: &str = "replace-folder";
/// How long a sheet takes to slide into its window before a picture shows it.
const SHEET_SETTLE: Duration = Duration::from_millis(800);
const EDIT: &str = "// unsaved\n";
/// The folders of the run, each with one file of its own.
#[derive(Clone)]
struct Folders {
    a: PathBuf,
    b: PathBuf,
    c: PathBuf,
    untrusted: PathBuf,
}

/// The folder `name` with the one file `file`, trusted or not.
fn folder(name: &str, file: &str, trusted: bool) -> PathBuf {
    let text = format!("// {file}\nfn main() {{}}\n");
    let root = helpers::make_project(name, &[(file, &text)]).unwrap_or_else(|_| helpers::root(name));
    if trusted {
        crate::project::trust(&root);
    }
    root
}

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folders = Folders {
        a: folder("replace-a", "a.rs", true),
        b: folder("replace-b", "b.rs", true),
        c: folder("replace-c", "c.rs", true),
        untrusted: folder("replace-untrusted", "d.rs", false),
    };
    let _ = std::fs::write(folders.b.join("more.rs"), "// more.rs\n");
    crate::app::open_project(&crate::context::app(), mtm, folders.a.clone());
    if let Some(controller) = window_of(&folders.a) {
        controller.open_file(&folders.a.join("a.rs"));
    }
    let a = folders.a.clone();
    wait_until(
        move || window_of(&a).is_some_and(|c| editor(&c, "a.rs").is_some()),
        move |_| replace_clean(folders),
    );
}

fn projects() -> String {
    format!("{:?}", window::open_projects(&crate::context::app()))
}

fn frame_of(controller: &WindowController) -> String {
    controller.window().map_or_else(String::new, |w| {
        let frame = w.frame();
        format!(
            "{}×{} at {},{}",
            frame.size.width, frame.size.height, frame.origin.x, frame.origin.y
        )
    })
}

fn replace_clean(f: Folders) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(controller) = window_of(&f.a) else {
        return give_up_saying(SCENARIO, "the first folder gets its window", projects());
    };
    let windows = window::open_windows().len();
    let before = frame_of(&controller);
    controller.quick_open_with("a.rs");
    crate::app::open_project(&crate::context::app(), mtm, f.b.clone());
    let same = window_of(&f.b).is_some_and(|w| std::ptr::eq(&*w, &*controller));
    let titles = controller.tab_titles();
    check(
        "a folder opened in a window with nothing to lose takes the place of its folder at once",
        same && window_of(&f.a).is_none()
            && window::open_windows().len() == windows
            && !titles.iter().any(|title| title == "a.rs"),
        format!(
            "{}, tabs {titles:?}, frame {before} → {}",
            projects(),
            frame_of(&controller)
        ),
    );
    controller.quick_open_with("b.rs");
    let b = f.b.clone();
    wait_until(
        move || window_of(&b).is_some_and(|c| c.quick_open_hits().iter().any(|hit| hit == "b.rs")),
        move |found| {
            let hits = window_of(&f.b).map(|c| c.quick_open_hits()).unwrap_or_default();
            check(
                "Open Quickly searches the folder that took the window",
                found,
                format!("{hits:?}"),
            );
            edit_b(f);
        },
    );
}

fn edit_b(f: Folders) {
    let Some(controller) = window_of(&f.b) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    controller.close_quick_open();
    controller.open_file(&f.b.join("b.rs"));
    if let Some(view) = editor(&controller, "b.rs") {
        type_at(&view, NSRange::new(0, 0), EDIT);
    }
    let b = f.b.clone();
    wait_until(
        move || window_of(&b).is_some_and(|c| crate::editor::unsaved_in(&c) == ["b.rs"]),
        move |_| {
            if let Some(window) = window_of(&f.b).and_then(|c| c.window()) {
                snapshot(&window, "replace-folder-clean");
            }
            ask(f, sheet_shown);
        },
    );
}

/// Opens folder C as the user does while the window in front shows B with unsaved text, and goes on with `then` once
/// the sheet has laid out its buttons.
fn ask(f: Folders, then: fn(Folders, bool)) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(&crate::context::app(), mtm, f.c.clone());
    let b = f.b.clone();
    wait_until(move || sheet_buttons(&b).len() == 3, move |ok| then(f, ok));
}

fn sheet_of(root: &Path) -> Option<Retained<NSWindow>> {
    window_of(root)?.window()?.attachedSheet()
}

fn sheet_views(root: &Path) -> Vec<Retained<NSView>> {
    fn collect(view: &NSView, into: &mut Vec<Retained<NSView>>) {
        for subview in view.subviews().iter() {
            collect(&subview, into);
            into.push(subview);
        }
    }
    let mut views = vec![];
    if let Some(content) = sheet_of(root).and_then(|sheet| sheet.contentView()) {
        collect(&content, &mut views);
    }
    views
}

/// The buttons the sheet on the window of `root` shows: title, key equivalent and whether it is destructive.
fn sheet_buttons(root: &Path) -> Vec<(String, String, bool)> {
    sheet_views(root)
        .iter()
        .filter_map(|view| view.downcast_ref::<NSButton>())
        .filter(|button| !button.isHidden() && button.title().length() > 0)
        .map(|button| {
            (
                button.title().to_string(),
                button.keyEquivalent().to_string(),
                button.hasDestructiveAction(),
            )
        })
        .collect()
}

/// The title of the button Return presses; an alert begun while the app is not active keeps its default in the
/// sheet's default button cell and clears the button's key equivalent.
fn default_button(root: &Path) -> Option<String> {
    if let Some(cell) = sheet_of(root).and_then(|sheet| sheet.defaultButtonCell()) {
        return Some(cell.title().to_string());
    }
    sheet_buttons(root)
        .into_iter()
        .find_map(|(title, key, _)| (key == "\r").then_some(title))
}

fn sheet_text(root: &Path) -> String {
    sheet_views(root)
        .iter()
        .filter_map(|view| {
            view.downcast_ref::<NSTextField>()
                .map(|field| field.stringValue().to_string())
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Ends the sheet on the window of `root` as a click on the button that gives `response` would.
fn answer(root: &Path, response: NSModalResponse) {
    if let (Some(window), Some(sheet)) = (window_of(root).and_then(|c| c.window()), sheet_of(root)) {
        window.endSheet_returnCode(&sheet, response);
    }
}

fn sheet_shown(f: Folders, ok: bool) {
    let buttons = sheet_buttons(&f.b);
    let text = sheet_text(&f.b);
    check(
        "with unsaved text, opening a folder asks first and names the file",
        ok && text.contains("b.rs"),
        text,
    );
    let titled = |title: &str| buttons.iter().find(|(shown, _, _)| shown == title).cloned();
    let close = titled(t("Alles schließen"));
    let cancel = titled(t("Abbrechen"));
    let default = default_button(&f.b);
    check(
        "the sheet offers a new window by default, closing everything as destructive, and Esc cancels",
        default.as_deref() == Some(t("In neuem Fenster öffnen"))
            && close.is_some_and(|(_, _, destructive)| destructive)
            && cancel.is_some_and(|(_, key, _)| key == "\u{1b}"),
        format!("default {default:?}, {buttons:?}"),
    );
    helpers::after(SHEET_SETTLE, move || {
        if let Some(window) = window_of(&f.b).and_then(|c| c.window()) {
            snapshot(&window, "replace-folder-sheet");
        }
        answer(&f.b, NSAlertThirdButtonReturn);
        let b = f.b.clone();
        wait_until(move || sheet_of(&b).is_none(), move |_| cancelled(f));
    });
}

fn cancelled(f: Folders) {
    let unsaved = window_of(&f.b)
        .map(|c| crate::editor::unsaved_in(&c))
        .unwrap_or_default();
    check(
        "„Abbrechen“ keeps the window's folder and its unsaved text",
        window_of(&f.c).is_none() && unsaved == ["b.rs"] && sheet_of(&f.b).is_none(),
        format!("{}, unsaved {unsaved:?}", projects()),
    );
    ask(f, new_window);
}

fn new_window(f: Folders, ok: bool) {
    let windows = window::open_windows().len();
    answer(&f.b, NSAlertFirstButtonReturn);
    let c = f.c.clone();
    wait_until(move || window_of(&c).is_some(), move |_| opened_beside(f, ok, windows));
}

fn opened_beside(f: Folders, asked: bool, windows: usize) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let unsaved = window_of(&f.b)
        .map(|c| crate::editor::unsaved_in(&c))
        .unwrap_or_default();
    check(
        "„In neuem Fenster öffnen“ opens the folder beside and leaves the window as it was",
        asked && window_of(&f.c).is_some() && unsaved == ["b.rs"] && window::open_windows().len() == windows + 1,
        format!("{}, unsaved {unsaved:?}", projects()),
    );
    let count = window::open_windows().len();
    crate::app::open_project(&crate::context::app(), mtm, f.b.clone());
    check(
        "a folder open in another window comes to the front there instead",
        window_of(&f.c).is_some() && window_of(&f.b).is_some() && window::open_windows().len() == count,
        projects(),
    );
    folders_apart(&f);
    if let Some(window) = window_of(&f.c).and_then(|c| c.window()) {
        window.performClose(None);
    }
    let c = f.c.clone();
    wait_until(
        move || window_of(&c).is_none(),
        move |closed| {
            check("the window beside closes", closed, projects());
            tab_group(f);
        },
    );
}

/// "Fenster zusammenführen" on the window of B leaves the window of C alone.
fn folders_apart(f: &Folders) {
    let (Some(b), Some(c)) = (
        window_of(&f.b).and_then(|c| c.window()),
        window_of(&f.c).and_then(|c| c.window()),
    ) else {
        return check("both folders have windows", false, projects());
    };
    b.mergeAllWindows(None);
    let merged = b
        .tabbedWindows()
        .is_some_and(|tabs| tabs.iter().any(|tab| std::ptr::eq(&*tab, &*c)));
    check(
        "windows of different folders never merge into one tab group",
        b.tabbingIdentifier() != c.tabbingIdentifier() && !merged,
        format!("{} / {}", b.tabbingIdentifier(), c.tabbingIdentifier()),
    );
}

/// Opens a second file of B in a tab of its own, with unsaved text there too, then opens C in the window of B.
fn tab_group(f: Folders) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(controller) = window_of(&f.b) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    controller.open_file(&f.b.join("more.rs"));
    if let Some(view) = editor(&controller, "more.rs") {
        type_at(&view, NSRange::new(0, 0), EDIT);
    }
    let tabs = controller.tab_windows();
    check(
        "the folder's files are tabs of one window",
        tabs.len() >= 3
            && tabs
                .first()
                .and_then(|window| window.tabbedWindows())
                .is_some_and(|tabbed| tabbed.count() == tabs.len()),
        format!("{:?}", controller.window_tabs()),
    );
    crate::app::open_project_from(&crate::context::app(), mtm, f.c.clone(), Some(controller));
    let b = f.b.clone();
    wait_until(move || sheet_buttons(&b).len() == 3, move |ok| close_all(f, ok));
}

fn close_all(f: Folders, asked: bool) {
    let text = sheet_text(&f.b);
    check(
        "the question counts the unsaved text of every tab of the folder",
        asked && text.contains("b.rs") && text.contains("more.rs"),
        text,
    );
    let before = window_of(&f.b).map(|c| frame_of(&c)).unwrap_or_default();
    answer(&f.b, NSAlertSecondButtonReturn);
    let c = f.c.clone();
    wait_until(move || window_of(&c).is_some(), move |_| closed_all(f, asked, before));
}

fn closed_all(f: Folders, asked: bool, before: String) {
    let Some(controller) = window_of(&f.c) else {
        return give_up_saying(SCENARIO, "„Alles schließen“ opens the folder in the window", projects());
    };
    let on_disk = std::fs::read_to_string(f.b.join("b.rs")).unwrap_or_default()
        + &std::fs::read_to_string(f.b.join("more.rs")).unwrap_or_default();
    let titles = controller.tab_titles();
    check(
        "„Alles schließen“ gives up the unsaved text, closes the folder's other tabs and opens the folder in the window",
        asked
            && window_of(&f.b).is_none()
            && window::open_windows().len() == 1
            && controller.tab_windows().len() == 1
            && crate::editor::unsaved_in(&controller).is_empty()
            && !on_disk.contains(EDIT)
            && !titles.iter().any(|title| title == "b.rs"),
        format!(
            "{}, tabs {titles:?}, frame {before} → {}",
            projects(),
            frame_of(&controller)
        ),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "replace-folder-closed");
    }
    waiting_window(f);
}

/// A window whose trust sheet waits for an answer keeps its folder; the next folder gets a window of its own.
fn waiting_window(f: Folders) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(&crate::context::app(), mtm, f.untrusted.clone());
    let asks = sheet_of(&f.untrusted).is_some();
    crate::app::open_project(&crate::context::app(), mtm, f.a.clone());
    let kept = window_of(&f.untrusted).is_some_and(|c| c.window().is_some_and(|w| w.attachedSheet().is_some()));
    let apart = match (window_of(&f.untrusted), window_of(&f.a)) {
        (Some(waiting), Some(opened)) => !std::ptr::eq(&*waiting, &*opened),
        _ => false,
    };
    check(
        "a window that waits for a sheet keeps its folder, and the next one opens beside",
        asks && kept && apart,
        format!("trust sheet {asks}, {}", projects()),
    );
    // A sheet orders its window in front once it has slid in, and without an active app no window is the main one.
    answer(&f.untrusted, NSAlertSecondButtonReturn);
    if let Some(window) = window_of(&f.untrusted).and_then(|c| c.window()) {
        window.performClose(None);
    }
    let untrusted = f.untrusted.clone();
    wait_until(move || window_of(&untrusted).is_none(), move |_| running_terminal(f));
}

/// Starts `sleep` in a terminal of folder A's window, which is in front now.
fn running_terminal(f: Folders) {
    let Some(controller) = window_of(&f.a) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    crate::terminal::open_tab(&controller, None);
    let Some((id, _)) = inspect::latest() else {
        return give_up(SCENARIO, "a terminal opens");
    };
    wait_until(
        move || inspect::view(id).is_some_and(|view| view.prompts() > 0),
        move |started| {
            check("the terminal's shell starts", started, "");
            if let Some(view) = inspect::view(id) {
                view.type_text("sleep 60\r");
            }
            wait_until(
                move || {
                    inspect::view(id)
                        .is_some_and(|view| view.foreground() == ide_terminal::Foreground::Other("sleep".into()))
                },
                move |sleeping| ask_running(f, id, sleeping),
            );
        },
    );
}

fn ask_running(f: Folders, id: u64, sleeping: bool) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let shell = inspect::view(id).and_then(|view| view.shell()).unwrap_or(0);
    crate::app::open_project(&crate::context::app(), mtm, f.c.clone());
    let a = f.a.clone();
    wait_until(
        move || sheet_buttons(&a).len() == 3,
        move |asked| {
            let text = sheet_text(&f.a);
            check(
                "a program running in a terminal makes opening a folder ask first and is named",
                sleeping && asked && text.contains("sleep"),
                text,
            );
            answer(&f.a, NSAlertSecondButtonReturn);
            let stopped = move || {
                Supervisor::global()
                    .running()
                    .iter()
                    .all(|process| process.pid != shell)
            };
            wait_until(stopped, move |stopped| {
                check(
                    "„Alles schließen“ ends the terminal's programs and opens the folder in the window",
                    stopped && shell != 0 && window_of(&f.c).is_some() && window_of(&f.a).is_none(),
                    format!("shell {shell}, {}", projects()),
                );
                finish(SCENARIO);
            });
        },
    );
}
