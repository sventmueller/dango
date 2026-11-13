//! The Tests panel (B27, E110, E111): the tests of a window's folder in a tree, run through the supervisor and read by
//! the adapters of `ide_test`; a symbol in the gutter at each test, a mark at each failed assert, and the commands in
//! the Debug menu's Tests section.

mod panel;
mod run;

use std::path::Path;

use ide_test::{DebugTarget, Scope, State, TestId, Tool};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::NSMenuItem;
use splat_i18n::t;

use crate::editor::EditorTextView;
use crate::window::{Panel, WindowController};
pub use panel::{TestsPanel, state_symbol};

/// The source of the marks failed asserts leave, beside the language servers' own (E111).
pub const SOURCE: u64 = u64::MAX;

/// Runs `f` with the panel `id`, if its window is still open.
pub fn with_panel(id: u64, f: impl FnOnce(&TestsPanel)) {
    if let Some(mtm) = MainThreadMarker::new() {
        crate::context::AppContext::get(mtm).tests_panels.with(id, f);
    }
}

fn of(controller: &WindowController) -> Option<Retained<TestsPanel>> {
    controller.context().tests_panels.of(controller.handle())
}

/// The window shows a folder: its panel lists the tests that list without building.
pub fn attach(controller: &WindowController) {
    let Some(root) = controller.project_root() else { return };
    window_closed(controller);
    let panels = &controller.context().tests_panels;
    let panel = TestsPanel::new(controller.mtm(), panels.next_id(), controller.id(), root);
    let (page, header) = panel.build();
    controller.set_panel_page(Panel::Tests, &page);
    controller.set_panel_accessory(Panel::Tests, &header);
    panels.insert(panel.id(), panel.clone());
    panel.list(false);
}

/// The user trusts `root` now: its panels list their tests.
pub fn trust_changed(root: &Path) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    for panel in crate::context::AppContext::get(mtm).tests_panels.for_root(root) {
        panel.list(false);
    }
}

/// The window's folder closes: its panel goes.
pub fn window_closed(controller: &WindowController) {
    controller.context().tests_panels.detach(controller.handle());
}

/// The test at the caret of the focused editor of `controller`, with its tool.
fn at_cursor(controller: &WindowController) -> Option<(Retained<TestsPanel>, Tool, TestId)> {
    let panel = of(controller)?;
    let editor = crate::editor::focused_editor(controller)?;
    let path = editor.document().path()?.to_path_buf();
    let line = caret_line(&editor);
    locate(&panel, &editor);
    let found = panel
        .ivars()
        .board
        .borrow()
        .at(&path, line)
        .map(|e| (e.tool, e.test.id.clone()));
    let (tool, id) = found?;
    Some((panel, tool, id))
}

fn caret_line(editor: &EditorTextView) -> u32 {
    let at = editor.selectedRange().location;
    let document = editor.document();
    let text = document.text();
    let byte = text.utf16_to_byte(at.min(text.len_utf16())).unwrap_or(0);
    u32::try_from(text.line_of(byte) + 1).unwrap_or(u32::MAX)
}

/// Gives the panel's tests the lines `editor`'s text shows them on.
fn locate(panel: &TestsPanel, editor: &EditorTextView) {
    let Some(path) = editor.document().path().map(Path::to_path_buf) else {
        return;
    };
    let text = editor.document().text().to_string();
    let found = ide_test::board::in_source(&path, &text);
    if found.is_empty() {
        return;
    }
    let known = !panel.ivars().board.borrow().tests_in(&path).is_empty();
    panel.ivars().board.borrow_mut().locate(&path, &text);
    if !known {
        panel.show();
    }
}

/// Runs the test at the caret, or says there is none (⌃⌥⌘U).
pub fn run_at_cursor(controller: &WindowController) {
    match at_cursor(controller) {
        Some((panel, tool, id)) => panel.run(tool, Scope::Test(id)),
        None => crate::editor::show_alert(controller.mtm(), t("Kein Test an der Einfügemarke"), ""),
    }
}

/// Debugs the test at the caret.
pub fn debug_at_cursor(controller: &WindowController) {
    match at_cursor(controller) {
        Some((panel, tool, id)) => debug(&panel, tool, &id),
        None => crate::editor::show_alert(controller.mtm(), t("Kein Test an der Einfügemarke"), ""),
    }
}

/// Runs the last test run again (⌃⌥⌘G).
pub fn rerun(controller: &WindowController) {
    if let Some(panel) = of(controller) {
        panel.rerun();
    }
}

/// Runs every test of the folder.
pub fn run_all(controller: &WindowController) {
    if let Some(panel) = of(controller) {
        controller.activate_panel(Panel::Tests);
        panel.run_everything();
    }
}

/// Debugs the test `id` of `tool` in the panel's window, as the Debug panel does a program (C31, F27).
pub fn debug(panel: &TestsPanel, tool: Tool, id: &TestId) {
    let target = tool.debug(panel.root(), id);
    let program = match target {
        DebugTarget::Native { build, args } => {
            let target = id.package.clone();
            ide_dap::TestProgram::Native {
                build: build.args,
                cwd: build.cwd,
                target,
                args,
            }
        }
        DebugTarget::Go { package, args } => ide_dap::TestProgram::Go { package, args },
        DebugTarget::PythonModule { module, args, cwd } => ide_dap::TestProgram::PythonModule { module, args, cwd },
        DebugTarget::Node { program, args, cwd } => ide_dap::TestProgram::Node { program, args, cwd },
    };
    let name = id.name.rsplit("::").next().unwrap_or(&id.name).to_string();
    ide_ui_debug::debug_test(ide_ui_common::window::Window::new(panel.window()), name, program);
}

/// The panel's tests or failures changed: the editors of its folder show their marks again.
fn marks_changed(panel: &TestsPanel) {
    for editor in crate::editor::all_editors() {
        let Some(path) = editor.document().path().map(Path::to_path_buf) else {
            continue;
        };
        if !path.starts_with(panel.root()) {
            continue;
        }
        let failures = panel.ivars().board.borrow().failures_in(&path);
        crate::lsp::show_failures(&editor, SOURCE, &failures);
        crate::editor::redraw_gutter(&editor);
    }
}

/// The state of the test whose source starts at `line` of `editor`, from 0, for the gutter's symbol.
pub fn gutter_mark(editor: &EditorTextView, line: usize) -> Option<State> {
    let controller = editor.controller()?;
    let panel = of(&controller)?;
    let path = editor.document().path()?.to_path_buf();
    if panel.ivars().board.borrow().tests_in(&path).is_empty() {
        locate(&panel, editor);
    }
    let line = u32::try_from(line + 1).ok()?;
    let board = panel.ivars().board.borrow();
    board
        .tests_in(&path)
        .into_iter()
        .find(|(l, _)| *l == line)
        .map(|(_, e)| e.state)
}

/// A click on the gutter's test symbol at `line` of `editor`, from 0: runs the test, or debugs it with ⌥; whether
/// there was one.
pub fn gutter_click(editor: &EditorTextView, line: usize, debugging: bool) -> bool {
    let Some(controller) = editor.controller() else {
        return false;
    };
    let Some(panel) = of(&controller) else { return false };
    let Some(path) = editor.document().path().map(Path::to_path_buf) else {
        return false;
    };
    let line = u32::try_from(line + 1).unwrap_or(u32::MAX);
    let found = {
        let board = panel.ivars().board.borrow();
        board
            .tests_in(&path)
            .into_iter()
            .find(|(l, _)| *l == line)
            .map(|(_, e)| (e.tool, e.test.id.clone()))
    };
    let Some((tool, id)) = found else { return false };
    if debugging {
        debug(&panel, tool, &id);
    } else {
        panel.run(tool, Scope::Test(id));
    }
    true
}

/// The items of the gutter's context menu at a test's line: run and debug it.
pub fn gutter_items(editor: &EditorTextView, line: usize) -> Vec<Retained<NSMenuItem>> {
    if gutter_mark(editor, line).is_none() {
        return Vec::new();
    }
    let mtm = objc2::MainThreadOnly::mtm(editor);
    let run = crate::util::item(mtm, t("Test ausführen"), Some(objc2::sel!(ideTestRunAtLine:)), "");
    let debug = crate::util::item(mtm, t("Test debuggen"), Some(objc2::sel!(ideTestDebugAtLine:)), "");
    let tag = isize::try_from(line).unwrap_or(-1);
    run.setTag(tag);
    debug.setTag(tag);
    vec![run, debug]
}

/// The Debug menu with its Tests section (E111): run and debug the test at the caret, run the last run again and every
/// test; the shortcuts are Xcode's, ⌘U stays Underline (HIG).
pub fn with_menu_items(
    mtm: MainThreadMarker,
    debug_menu: Retained<NSMenuItem>,
    key: &dyn Fn(&str) -> String,
) -> Retained<NSMenuItem> {
    let Some(menu) = debug_menu.submenu() else {
        return debug_menu;
    };
    menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(mtm));
    for (title, action, command) in [
        (
            t("Test an der Einfügemarke ausführen"),
            objc2::sel!(ideTestRunAtCursor:),
            "test.runAtCursor",
        ),
        (
            t("Test an der Einfügemarke debuggen"),
            objc2::sel!(ideTestDebugAtCursor:),
            "",
        ),
        (
            t("Letzten Testlauf wiederholen"),
            objc2::sel!(ideTestRerun:),
            "test.rerun",
        ),
        (t("Alle Tests ausführen"), objc2::sel!(ideTestRunAll:), ""),
    ] {
        let shortcut = if command.is_empty() {
            String::new()
        } else {
            key(command)
        };
        menu.addItem(&crate::util::item(mtm, title, Some(action), &shortcut));
    }
    debug_menu
}

/// The panel of `controller`'s window, for the self-test.
#[cfg(feature = "selftest")]
pub fn panel_of(controller: &WindowController) -> Option<Retained<TestsPanel>> {
    of(controller)
}
