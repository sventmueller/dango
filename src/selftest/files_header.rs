//! The header of the files panel, part of the `search` scenario: its folder name, a new file and a new folder in the
//! selected folder, each named in the question the context menu's "Neu" asks too, and folding every folder.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::time::Duration;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::runtime::Sel;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSApplication, NSTextField, NSView};
use objc2_foundation::{NSRunLoop, NSRunLoopCommonModes, NSTimer};

use super::helpers::wait_until;
use super::{check, snapshot};
use crate::util::{main_on_run_loop, ns};
use crate::window::{self, WindowController};

/// When the question for the name is answered, a moment after it opened.
const ANSWER_AFTER: Duration = Duration::from_millis(500);

fn controller(root: &Path) -> Option<objc2::rc::Retained<WindowController>> {
    window::for_project(root)
}

/// Types `name` into the question in front and confirms it, from a timer of the run loop, which runs while it shows.
fn answer_name(name: &'static str) {
    let once = Cell::new(true);
    let block = RcBlock::new(move |_timer: NonNull<NSTimer>| {
        let Some(mtm) = MainThreadMarker::new().filter(|_| once.replace(false)) else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        let Some(content) = app.modalWindow().and_then(|w| w.contentView()) else {
            return;
        };
        if let Some(field) = editable_field(&content) {
            field.setStringValue(&ns(name));
        }
        app.stopModalWithCode(NSAlertFirstButtonReturn);
    });
    // SAFETY: The block holds only a flag and a name, which may go to any thread; the main run loop calls it.
    let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(ANSWER_AFTER.as_secs_f64(), false, &block) };
    // SAFETY: The main run loop takes the timer in its common modes, which include the one modal alerts run in.
    unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
}

fn editable_field(view: &NSView) -> Option<objc2::rc::Retained<NSTextField>> {
    if let Some(field) = view.downcast_ref::<NSTextField>()
        && field.isEditable()
    {
        return Some(objc2::Message::retain(field));
    }
    view.subviews().iter().find_map(|child| editable_field(&child))
}

/// Presses the header button with `action`, from the run loop, where its question can run; whether it is there.
fn press(root: &Path, action: Sel) -> bool {
    let there = controller(root).and_then(|c| c.files_header_button(action)).is_some();
    let root = root.to_path_buf();
    main_on_run_loop(move || {
        if let Some(button) = controller(&root).and_then(|c| c.files_header_button(action)) {
            // SAFETY: A click with no sender, which the tree's action takes.
            unsafe { button.performClick(None) };
        }
    });
    there
}

/// Once the tree shows the folder, opens `src`.
pub fn run(root: PathBuf, then: fn(PathBuf)) {
    let probe = root.clone();
    wait_until(
        move || controller(&probe).is_some_and(|c| c.tree_rows() > 0),
        move |_| {
            title_row(&root, false);
            let rows = controller(&root).map_or(0, |c| c.tree_rows());
            if let Some(controller) = controller(&root) {
                controller.click_in_tree(&root.join("src"), true);
            }
            let probe = root.clone();
            wait_until(
                move || controller(&probe).is_some_and(|c| c.tree_rows() > rows),
                move |_| new_file(root, then),
            );
        },
    );
}

/// "Dateien: <folder>" with the buttons on its line, the tree right below; checked again once a file opened, which
/// moves the sidebar into the file's window tab.
fn title_row(root: &Path, after_open: bool) {
    let folder = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let row = controller(root).and_then(|c| c.panel_title_row());
    let Some((title, row)) = row else {
        return check(
            "the files panel's title names the folder, its buttons on the same line",
            false,
            "no panel",
        );
    };
    let gap = (row.title_middle - row.page_top).abs();
    if !after_open {
        check(
            "the files panel's title names the folder, its buttons on the same line",
            title == splat_i18n::tf("Dateien: {}", &[&folder])
                && (row.title_middle - row.buttons_middle).abs() < 3.0
                && gap < 20.0,
            format!(
                "{title:?}, title at {:.1}, buttons at {:.1}, tree {gap:.1} pt below",
                row.title_middle, row.buttons_middle
            ),
        );
    }
    let name = if after_open {
        "with a file open, the files panel's buttons still sit at the row's right end"
    } else {
        "the files panel's buttons sit at the row's right end, apart from the title"
    };
    check(
        name,
        row.width - row.buttons_end < 10.0 && row.buttons_start - row.text_end > 20.0,
        format!(
            "title ends at {:.1}, buttons from {:.1} to {:.1} of {:.1}",
            row.text_end, row.buttons_start, row.buttons_end, row.width
        ),
    );
}

/// A new file goes into the selected folder.
fn new_file(root: PathBuf, then: fn(PathBuf)) {
    let Some(controller) = controller(&root) else {
        return then(root);
    };
    controller.select_in_tree(&root.join("src"));
    let pressed = press(&root, objc2::sel!(ideTreeNewFile:));
    answer_name("added.rs");
    let file = root.join("src/added.rs");
    wait_until(
        move || file.is_file(),
        move |ok| {
            check(
                "the header's new file goes into the selected folder and opens",
                pressed && ok && opened(&root, "src/added.rs"),
                "",
            );
            title_row(&root, true);
            if let Some(window) = controller_window(&root) {
                snapshot(&window, "files-header");
            }
            folder(root, then);
        },
    );
}

fn controller_window(root: &Path) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    controller(root).and_then(|c| c.window())
}

fn opened(root: &Path, relative: &str) -> bool {
    controller(root).is_some_and(|c| crate::editor::editor_of(&c, &root.join(relative)).is_some())
}

/// Without a selection, a new folder goes into the root.
fn folder(root: PathBuf, then: fn(PathBuf)) {
    if let Some(view) = controller(&root).and_then(|c| c.file_tree_outline()) {
        // SAFETY: Deselecting takes no sender.
        unsafe { view.deselectAll(None) };
    }
    let pressed = press(&root, objc2::sel!(ideTreeNewFolder:));
    answer_name("made");
    let made = root.join("made");
    wait_until(
        move || made.is_dir(),
        move |ok| {
            check(
                "the header's new folder goes into the root without a selection",
                pressed && ok,
                "",
            );
            collapse(root, then);
        },
    );
}

/// "Alle einklappen" leaves only the root's entries.
fn collapse(root: PathBuf, then: fn(PathBuf)) {
    let Some(controller) = controller(&root) else {
        return then(root);
    };
    let before = controller.tree_rows();
    let pressed = controller
        .files_header_button(objc2::sel!(ideTreeCollapseAll:))
        .map(|button| {
            // SAFETY: A click with no sender, which the tree's action takes.
            unsafe { button.performClick(None) }
        })
        .is_some();
    let after = controller.tree_rows();
    let top = std::fs::read_dir(&root).map_or(0, |entries| entries.count());
    check(
        "the header folds every folder",
        pressed && after < before && usize::try_from(after).is_ok_and(|a| a <= top),
        format!("{before} rows, then {after}; {top} entries at the top"),
    );
    then(root);
}
