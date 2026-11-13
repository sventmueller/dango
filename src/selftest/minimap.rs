//! The `editor-minimap` scenario (E107): the minimap is off at first and comes from Editor ▸ Minimap; it follows the
//! scrolling of a file longer than it is high, a click scrolls the editor to the clicked line, folded lines leave it,
//! a narrow editor hides it, and pictures show it light, dark and with increased contrast.

use std::path::PathBuf;

use ide_core::settings::{Settings, keys};
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSRect, NSSize};

use super::helpers::{self, wait_until};
use super::{check, finish, snapshot};
use crate::editor::{self, EditorTextView};
use crate::window;

const SCENARIO: &str = "editor-minimap";
const PROJECT: &str = "minimap-project";
const FILE: &str = "src/lib.rs";
const FUNCTIONS: usize = 150;
/// A row in the middle of the minimap, whose rows are 2 pt high, and its top.
const CLICKED_ROW: usize = 150;
const CLICKED_ROW_Y: f64 = 300.0;

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn view() -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*window::for_project(&root())?, &root().join(FILE))
}

/// A file far longer than the minimap is high, its functions each four lines long.
fn source() -> String {
    (0..FUNCTIONS)
        .map(|n| format!("pub fn f{n}(x: u32) -> u32 {{\n    let y = x + {n}; // \"note\"\n    y * 2\n}}\n"))
        .collect()
}

fn picture(name: &str) {
    if let Some(window) = window::for_project(&root()).and_then(|c| c.window()) {
        window.displayIfNeeded();
        snapshot(&window, name);
    }
}

fn toggle(view: &EditorTextView) {
    let app = NSApplication::sharedApplication(view.mtm());
    // SAFETY: An action the editor answers, sent to it with no sender, as the menu item without a target sends it.
    unsafe { app.sendAction_to_from(objc2::sel!(ideToggleMinimap:), Some(view), None) };
    if let Some(window) = view.window() {
        window.layoutIfNeeded();
    }
}

pub fn run() {
    let source = source();
    if let Err(error) = helpers::make_project(PROJECT, &[(FILE, &source)]) {
        check("the scenario's folder is made", false, error.to_string());
        return finish(SCENARIO);
    }
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    crate::app::open_project(mtm, root());
    helpers::wait_for_window(&root(), |controller| {
        let Some(controller) = controller else {
            check("the project's window opens", false, "");
            return finish(SCENARIO);
        };
        controller.open_file(&root().join(FILE));
        wait_until(
            || view().is_some_and(|v| v.document().is_highlighted()),
            |_| shown_from_menu(),
        );
    });
}

fn shown_from_menu() {
    let Some(view) = view() else {
        check("the file opens in an editor", false, "");
        return finish(SCENARIO);
    };
    let before = editor::minimap(&view).map(|m| m.0);
    toggle(&view);
    let after = editor::minimap(&view);
    let saved = Settings::load_user().get(&keys::MINIMAP);
    check(
        "the minimap is off at first and Editor ▸ Minimap shows it, remembered",
        before == Some(0.0) && after.as_ref().is_some_and(|m| m.0 > 0.0) && saved,
        format!("{before:?} → {:?}, saved {saved}", after.as_ref().map(|m| m.0)),
    );
    let first = after.map(|m| m.2).unwrap_or_default();
    check(
        "at the top it starts with the first line",
        first.first() == Some(&0) && first.len() > 50,
        format!("{:?}", first.iter().take(3).collect::<Vec<_>>()),
    );
    picture("minimap-top");
    follows_scrolling(&view);
}

fn follows_scrolling(view: &EditorTextView) {
    let lines = view.document().text().line_count();
    let end = view.document().text().len_utf16();
    view.scrollRangeToVisible(objc2_foundation::NSRange::new(end, 0));
    let (_, visible, rows) = editor::minimap(view).unwrap_or_default();
    check(
        "at the end of a file longer than the minimap, it shows the last lines with the visible rows framed",
        rows.last().is_some_and(|&l| l + 2 >= lines) && visible > 0 && rows.first().is_some_and(|&l| l > 0),
        format!(
            "{lines} lines, visible from row {visible}, rows {:?}..{:?}",
            rows.first(),
            rows.last()
        ),
    );
    picture("minimap-end");
    let before = editor::minimap(view).map(|m| m.2).unwrap_or_default();
    let wanted = before.get(CLICKED_ROW).copied().unwrap_or(0);
    editor::click_minimap(view, CLICKED_ROW_Y);
    let shown = editor::visible_line_range(view);
    check(
        "a click scrolls the editor so that the clicked line shows",
        shown.contains(&wanted),
        format!("line {wanted}, editor shows {shown:?}"),
    );
    folded(view);
}

fn folded(view: &EditorTextView) {
    editor::scroll_to_byte(view, 0);
    editor::selftest::fold_all(view);
    let rows = editor::minimap(view).map(|m| m.2).unwrap_or_default();
    let hidden = rows.iter().any(|&l| l % 4 == 1 || l % 4 == 2);
    check(
        "folded lines leave the minimap",
        !hidden && rows.first() == Some(&0) && rows.get(1) == Some(&3),
        format!("{:?}", rows.iter().take(6).collect::<Vec<_>>()),
    );
    editor::selftest::unfold_all(view);
    appearances(view);
}

fn appearances(view: &EditorTextView) {
    use objc2_app_kit::{
        NSAppearanceNameAccessibilityHighContrastAqua, NSAppearanceNameAccessibilityHighContrastDarkAqua,
        NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    };
    let Some(controller) = window::for_project(&root()) else {
        return finish(SCENARIO);
    };
    let names = [
        (crate::util::constant!(NSAppearanceNameAqua), "light"),
        (crate::util::constant!(NSAppearanceNameDarkAqua), "dark"),
        (
            crate::util::constant!(NSAppearanceNameAccessibilityHighContrastAqua),
            "light-contrast",
        ),
        (
            crate::util::constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua),
            "dark-contrast",
        ),
    ];
    for (name, label) in names {
        super::agent::appearance(&controller, name);
        picture(&format!("minimap-{label}"));
    }
    if let Some(window) = controller.window() {
        objc2_app_kit::NSAppearanceCustomization::setAppearance(&*window, None);
    }
    narrow(view);
}

fn narrow(editor: &EditorTextView) {
    let Some(window) = editor.window() else {
        return finish(SCENARIO);
    };
    let frame = window.frame();
    window.setFrame_display(
        NSRect::new(
            frame.origin,
            NSSize::new(frame.size.width.min(560.0), frame.size.height),
        ),
        true,
    );
    wait_until(
        || view().and_then(|v| editor::minimap(&v)).is_some_and(|m| m.0 == 0.0),
        move |narrow| {
            if let Some(window) = view().and_then(|v| v.window()) {
                window.setFrame_display(frame, true);
            }
            wait_until(
                || view().and_then(|v| editor::minimap(&v)).is_some_and(|m| m.0 > 0.0),
                move |wide| {
                    check(
                        "a narrow editor hides the minimap, a wide one shows it again",
                        narrow && wide,
                        format!("{narrow} / {wide}"),
                    );
                    let Some(view) = view() else { return finish(SCENARIO) };
                    toggle(&view);
                    let off = editor::minimap(&view).map(|m| m.0);
                    check(
                        "Editor ▸ Minimap hides it again",
                        off == Some(0.0) && !Settings::load_user().get(&keys::MINIMAP),
                        format!("{off:?}"),
                    );
                    budget();
                },
            );
        },
    );
}

/// E25 with the minimap on: a keystroke in a file of 10 000 lines stays within `insets::KEYSTROKE`, edit, layout and
/// drawing of the minimap together.
fn budget() {
    let big: String = (0..10_000)
        .map(|n| format!("    let value_{n} = compute({n}, other_{n});\n"))
        .collect();
    let path = root().join("big.rs");
    if let Err(error) = std::fs::write(&path, &big) {
        check("the big file is written", false, error.to_string());
        return finish(SCENARIO);
    }
    let Some(controller) = window::for_project(&root()) else {
        return finish(SCENARIO);
    };
    controller.open_file(&path);
    let opened = path.clone();
    let big_view = move || window::for_project(&root()).and_then(|c| editor::editor_for(&c, &opened));
    let ready = big_view.clone();
    wait_until(
        move || ready().is_some_and(|v| v.line_start(9_999).is_some()),
        move |_| {
            let Some(view) = big_view() else {
                check("the big file opens", false, "");
                return finish(SCENARIO);
            };
            toggle(&view);
            let at = view.line_start(5_000).unwrap_or(0) + 20;
            view.scrollRangeToVisible(objc2_foundation::NSRange::new(at, 0));
            view.setSelectedRange(objc2_foundation::NSRange::new(at, 0));
            let width = editor::minimap(&view).map_or(0.0, |m| m.0);
            let (median, slowest) = super::insets::keystrokes(&view, 40);
            check(
                "E25 with the minimap: a keystroke in a file of 10 000 lines stays under 5 ms",
                width > 0.0 && median < super::insets::KEYSTROKE,
                format!("minimap {width} wide, median {median:?}, slowest {slowest:?}"),
            );
            toggle(&view);
            let _ = editor::save_all();
            finish(SCENARIO);
        },
    );
}
