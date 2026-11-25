//! The Tests panel (B27, E110, E111): the tests of a window's folder in a tree, run through the supervisor and read by
//! the adapters of `ide_test`; a symbol in the gutter at each test, a mark at each failed assert, and the commands in
//! the Debug menu's Tests section.

mod outline;
mod panel;
mod run;

use std::path::Path;

use ide_test::{DebugTarget, Scope, State, TestId, Tool};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::NSMenuItem;
use splat_i18n::{t, tf};

use crate::editor::EditorTextView;
use crate::window::{Panel, WindowController};
pub(crate) use panel::{TestsPanel, state_symbol};

/// The source of the marks failed asserts leave, beside the language servers' own (E111).
pub(crate) const SOURCE: u64 = u64::MAX;

/// Runs `f` with the panel `id`, if its window is still open.
pub(crate) fn with_panel(id: u64, f: impl FnOnce(&TestsPanel)) {
    if let Some(mtm) = MainThreadMarker::new() {
        crate::context::AppContext::get(mtm).tests_panels().with(id, f);
    }
}

fn of(controller: &WindowController) -> Option<Retained<TestsPanel>> {
    controller.context().tests_panels().of(controller.handle())
}

/// The window shows a folder: its panel lists the tests, those of tools that list by building from the sources, once
/// it shows or an editor's gutter needs them.
pub(crate) fn attach(controller: &WindowController) {
    let Some(root) = controller.project_root() else { return };
    window_closed(controller);
    let panels = &controller.context().tests_panels();
    let panel = TestsPanel::new(controller.mtm(), panels.next_id(), controller.id(), root);
    let (page, header) = panel.build();
    controller.set_panel_page(Panel::Tests, &page);
    controller.set_panel_accessory(Panel::Tests, &header);
    panels.insert(panel.id(), panel.clone());
    let id = panel.id();
    ide_ui_common::panel::watch_view(&page, move || with_panel(id, TestsPanel::needed), None);
}

/// The user trusts `root` now: its panels list their tests, if they were needed already.
pub(crate) fn trust_changed(cx: &std::rc::Rc<crate::context::AppContext>, root: &Path) {
    for panel in cx.tests_panels().for_root(root) {
        panel.trusted();
    }
}

/// The window's folder closes: its panel goes.
pub(crate) fn window_closed(controller: &WindowController) {
    controller.context().tests_panels().detach(controller.handle());
}

/// The test at the caret of the focused editor of `controller`, with its tool.
fn at_cursor(controller: &WindowController) -> Option<(Retained<TestsPanel>, Tool, TestId)> {
    controller.attach_waiting_parts();
    let panel = of(controller)?;
    let editor = crate::editor::focused_editor(controller)?;
    let path = editor.document().path()?.to_path_buf();
    let line = caret_line(&editor);
    panel.needed();
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

/// Gives the panel's tests the lines `editor`'s text shows them on, the text read once per revision of it.
fn locate(panel: &TestsPanel, editor: &EditorTextView) {
    let Some((path, document)) = editor
        .document()
        .path()
        .map(|path| (path.to_path_buf(), editor.document().revision()))
    else {
        return;
    };
    let found = {
        let mut marks = panel.ivars().marks.borrow_mut();
        let marks = marks.entry(path.clone()).or_default();
        if marks.read != Some(document) {
            let text = editor.document().text().to_string();
            marks.found = ide_test::board::in_source(&path, &text);
            marks.read = Some(document);
        }
        marks.found.clone()
    };
    place(panel, &path, &found);
}

/// As [`locate`], with the text read on a lane once per revision; the editors of the file draw their gutter again
/// when it placed tests.
fn locate_soon(panel: &TestsPanel, editor: &EditorTextView) {
    let (path, revision, text) = {
        let document = editor.document();
        let Some(path) = document.path() else { return };
        (path.to_path_buf(), document.revision(), document.text().clone())
    };
    let found = {
        let mut marks = panel.ivars().marks.borrow_mut();
        let marks = marks.entry(path.clone()).or_default();
        if marks.read == Some(revision) {
            Some(marks.found.clone())
        } else if marks.reading == Some(revision) {
            return;
        } else {
            marks.reading = Some(revision);
            None
        }
    };
    if let Some(found) = found {
        place(panel, &path, &found);
        return;
    }
    let id = panel.id();
    let shown_in = dispatch2::MainThreadBound::new(objc2::rc::Weak::from(editor), editor.mtm());
    let asked = path.clone();
    let read = move || {
        let found = ide_test::board::in_source(&asked, &text.to_string());
        (asked, found)
    };
    let answer = move |answer: Option<(std::path::PathBuf, Vec<(String, u32)>)>| {
        crate::util::main_soon(move || {
            if let Some((path, found)) = answer {
                let editor = objc2::MainThreadMarker::new().and_then(|mtm| shown_in.get(mtm).load());
                with_panel(id, |panel| located(panel, path, revision, found, editor.as_deref()));
            }
        });
    };
    if ide_core::lane::WORK.ask(read, answer).is_err() {
        if let Some(marks) = panel.ivars().marks.borrow_mut().get_mut(&path) {
            marks.reading = None;
        }
        crate::diag::report("finding the tests in a file", "the work lane took no more work");
    }
}

/// The lane read the tests of `path` at `revision`, asked by `editor`: the panel keeps them, unless it read a later
/// text since, and the editor shows the tests it placed.
fn located(
    panel: &TestsPanel,
    path: std::path::PathBuf,
    revision: ide_document::Revision,
    found: Vec<(String, u32)>,
    editor: Option<&EditorTextView>,
) {
    {
        let mut marks = panel.ivars().marks.borrow_mut();
        let marks = marks.entry(path.clone()).or_default();
        if marks.reading == Some(revision) {
            marks.reading = None;
        }
        if marks.read.is_some_and(|read| read >= revision) {
            return;
        }
        marks.read = Some(revision);
        marks.found.clone_from(&found);
    }
    if !found.is_empty() {
        panel.needed();
    }
    if place(panel, &path, &found)
        && let Some(editor) = editor
    {
        crate::editor::redraw_gutter(editor);
    }
}

/// Puts the tests `found` in `path` on the panel's board, and shows the panel if it knew none there; whether any
/// test got its place.
fn place(panel: &TestsPanel, path: &Path, found: &[(String, u32)]) -> bool {
    if found.is_empty() {
        return false;
    }
    let known = !panel.ivars().board.borrow().tests_in(path).is_empty();
    let before = panel.ivars().board.borrow().revision();
    panel.ivars().board.borrow_mut().place(path, found);
    if !known {
        panel.show();
    }
    panel.ivars().board.borrow().revision() != before
}

/// Runs the test at the caret, or says there is none (⌃⌥⌘U).
pub(crate) fn run_at_cursor(controller: &WindowController) {
    match at_cursor(controller) {
        Some((panel, tool, id)) => panel.run(tool, Scope::Test(id)),
        None => ide_ui_common::alert::warn(controller.mtm(), t("Kein Test an der Einfügemarke"), ""),
    }
}

/// Debugs the test at the caret.
pub(crate) fn debug_at_cursor(controller: &WindowController) {
    match at_cursor(controller) {
        Some((panel, tool, id)) => debug(&panel, tool, &id),
        None => ide_ui_common::alert::warn(controller.mtm(), t("Kein Test an der Einfügemarke"), ""),
    }
}

/// Runs the last test run again (⌃⌥⌘G).
pub(crate) fn rerun(controller: &WindowController) {
    controller.attach_waiting_parts();
    if let Some(panel) = of(controller) {
        panel.rerun();
    }
}

/// Runs every test of the folder.
pub(crate) fn run_all(controller: &WindowController) {
    controller.attach_waiting_parts();
    if let Some(panel) = of(controller) {
        controller.activate_panel(Panel::Tests);
        panel.run_everything();
    }
}

/// Debugs the test `id` of `tool` in the panel's window, as the Debug panel does a program (C31, F27).
pub(crate) fn debug(panel: &TestsPanel, tool: Tool, id: &TestId) {
    let Some(target) = tool.debug(&panel.folder(tool), id) else {
        let text = tf("Tests von {} lassen sich nicht debuggen", &[tool.name()]);
        ide_ui_common::alert::warn(panel.mtm(), &text, "");
        return;
    };
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
        ide_ui_lsp::show_failures(&*editor, SOURCE, &failures);
        crate::editor::redraw_gutter(&editor);
    }
}

/// The state of the test whose source starts at `line` of `editor`, from 0, for the gutter's symbol.
pub(crate) fn gutter_mark(editor: &EditorTextView, line: usize) -> Option<State> {
    let controller = editor.controller()?;
    let panel = of(&controller)?;
    let (path, document) = {
        let document = editor.document();
        (document.path()?.to_path_buf(), document.revision())
    };
    let board = panel.ivars().board.borrow().revision();
    let fresh = panel
        .ivars()
        .marks
        .borrow()
        .get(&path)
        .is_some_and(|m| m.shown == Some((board, document)));
    if !fresh {
        if ide_test::named_as_test(&path) {
            panel.needed();
        }
        if panel.ivars().board.borrow().tests_in(&path).is_empty() {
            locate_soon(&panel, editor);
        }
        let board = panel.ivars().board.borrow();
        let mut by_line = std::collections::HashMap::new();
        for (line, entry) in board.tests_in(&path) {
            by_line.entry(line).or_insert(entry.state);
        }
        let mut marks = panel.ivars().marks.borrow_mut();
        let marks = marks.entry(path.clone()).or_default();
        marks.shown = Some((board.revision(), document));
        marks.by_line = by_line;
    }
    let line = u32::try_from(line + 1).ok()?;
    let marks = panel.ivars().marks.borrow();
    marks.get(&path)?.by_line.get(&line).copied()
}

/// One file's tests for the gutter, so it looks up each line it draws instead of going through every test.
#[derive(Default)]
pub(crate) struct Marks {
    /// The document's revision `found` was read at.
    read: Option<ide_document::Revision>,
    /// The revision whose text a lane reads now.
    reading: Option<ide_document::Revision>,
    /// The tests the text shows, by function name and line from 1.
    found: Vec<(String, u32)>,
    /// The revisions of the board and the document `by_line` is of.
    shown: Option<(u64, ide_document::Revision)>,
    /// The state of the test at each line from 1.
    by_line: std::collections::HashMap<u32, State>,
}

/// A click on the gutter's test symbol at `line` of `editor`, from 0: runs the test, or debugs it with ⌥; whether
/// there was one.
pub(crate) fn gutter_click(editor: &EditorTextView, line: usize, debugging: bool) -> bool {
    let Some(controller) = editor.controller() else {
        return false;
    };
    controller.attach_waiting_parts();
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
pub(crate) fn gutter_items(editor: &EditorTextView, line: usize) -> Vec<Retained<NSMenuItem>> {
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
pub(crate) fn with_menu_items(
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

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;
