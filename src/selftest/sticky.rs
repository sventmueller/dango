//! The `editor-sticky` scenario (E108): in a Rust file of nested blocks the headers of the blocks around the top line
//! stay at the top of the editor, a line scrolled into view never lands under them, a click on a header scrolls to it,
//! and Editor ▸ Sticky Scroll turns them off. The folder is not trusted, so the headers come from the indentation.

use std::path::PathBuf;

use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::NSApplication;
use objc2_foundation::NSRange;

use super::helpers::{self, wait_until};
use super::{check, finish, give_up, give_up_saying, snapshot};
use crate::editor::{self, EditorTextView};
use crate::window;

const SCENARIO: &str = "editor-sticky";
const PROJECT: &str = "sticky-project";
const FILE: &str = "src/lib.rs";
/// The line of the innermost header, `fn deep`, counted from 0.
const DEEP: usize = 2;
/// The first line of the long body of `fn deep`.
const BODY: usize = 3;

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn view() -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*window::for_project(&root())?, &root().join(FILE))
}

fn source() -> String {
    let body: String = (0..200).map(|n| format!("            let v{n} = {n};\n")).collect();
    format!("mod outer {{\n    impl Thing {{\n        fn deep() {{\n{body}        }}\n    }}\n}}\n")
}

fn picture(name: &str) {
    if let Some(window) = window::for_project(&root()).and_then(|c| c.window()) {
        window.displayIfNeeded();
        snapshot(&window, name);
    }
}

/// Scrolls `view` so that `line` is the first on screen and lets the headers follow.
fn scroll_to_line(view: &EditorTextView, line: usize) {
    let byte: usize = view
        .document()
        .text()
        .to_string()
        .lines()
        .take(line)
        .map(|l| l.len() + 1)
        .sum();
    editor::scroll_to_byte(view, byte);
    if let Some(window) = view.window() {
        window.layoutIfNeeded();
    }
}

pub(crate) fn run() {
    if let Err(error) = helpers::make_project(PROJECT, &[(FILE, &source())]) {
        return give_up_saying(SCENARIO, "the scenario's folder is made", error.to_string());
    }
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    crate::app::open_project(&crate::context::app(), mtm, root());
    helpers::wait_for_window(&root(), |controller| {
        let Some(controller) = controller else {
            return give_up(SCENARIO, "the project's window opens");
        };
        controller.open_file(&root().join(FILE));
        wait_until(
            || view().is_some_and(|v| v.document().is_highlighted() && v.line_start(150).is_some()),
            |_| headers(),
        );
    });
}

fn headers() {
    let Some(view) = view() else {
        return give_up(SCENARIO, "the file opens in an editor");
    };
    let at_top = editor::sticky_shown(&view);
    scroll_to_line(&view, 100);
    let shown = editor::sticky_shown(&view);
    check(
        "at the top no headers show; deep in the body the three blocks around it do, outermost first",
        at_top.is_empty() && shown == [0, 1, DEEP],
        format!("{at_top:?} / {shown:?}"),
    );
    picture("sticky-nested");
    clear_of_headers(&view);
}

fn clear_of_headers(view: &EditorTextView) {
    scroll_to_line(view, 120);
    let target = editor::visible_line_range(view).start + 1;
    if let Some(start) = view.line_start(target) {
        view.setSelectedRange(NSRange::new(start, 0));
        view.scrollRangeToVisible(NSRange::new(start, 0));
    }
    let covered = editor::sticky_height(view);
    let top = editor::line_rect(view, target).map_or(0.0, |r| r.origin.y);
    let visible = view.visibleRect().origin.y;
    check(
        "a line behind the headers scrolled into view moves out from under them",
        covered > 0.0 && top + 0.5 >= visible + covered && top < visible + covered + 20.0,
        format!("line at {top}, view from {visible}, headers {covered} high"),
    );
    let row = covered / 3.0;
    editor::click_sticky(view, row * 1.5);
    let line_top = editor::line_rect(view, 1).map_or(-1.0, |r| r.origin.y);
    let after = view.visibleRect().origin.y;
    check(
        "a click on the second header scrolls its line right under the first",
        (line_top - (after + row)).abs() < 1.0,
        format!("line 2 at {line_top}, view from {after}, row {row}"),
    );
    turned_off(view);
}

fn turned_off(view: &EditorTextView) {
    scroll_to_line(view, BODY + 60);
    let app = NSApplication::sharedApplication(view.mtm());
    // SAFETY: An action the editor answers, sent to it with no sender, as the menu item without a target sends it.
    unsafe { app.sendAction_to_from(objc2::sel!(ideToggleStickyScroll:), Some(view), None) };
    let off = editor::sticky_shown(view);
    // SAFETY: As above.
    unsafe { app.sendAction_to_from(objc2::sel!(ideToggleStickyScroll:), Some(view), None) };
    let on = editor::sticky_shown(view);
    check(
        "Editor ▸ Sticky Scroll turns the headers off and on again",
        off.is_empty() && on.len() == 3,
        format!("{off:?} / {on:?}"),
    );
    finish(SCENARIO);
}
