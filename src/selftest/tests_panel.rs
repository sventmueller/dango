//! The `tests-panel` scenario (B27): a Go and Rust project's Tests panel shows its tests before any run, the Rust ones
//! read from the sources without building; a click on the gutter's symbol at a failing Go test runs it, the panel
//! shows it failed and the editor marks the failing line; the test at the caret runs from the Debug menu's command and
//! passes.

use std::path::PathBuf;
use std::time::Duration;

use objc2::rc::Retained;
use objc2_foundation::NSRange;

use super::helpers::{self, wait_within};
use super::{check, finish, give_up, snapshot};
use crate::editor::{self, EditorTextView};
use crate::window::{self, Panel, WindowController};

const SCENARIO: &str = "tests-panel";
const PROJECT: &str = "tests-project";
const FILE: &str = "sample_test.go";
const SOURCE: &str = "package sample\n\nimport \"testing\"\n\nfunc TestPass(t *testing.T) {\n\tif 1+2 != 3 {\n\t\tt.Fatal(\"wrong\")\n\t}\n}\n\nfunc TestFail(t *testing.T) {\n\tt.Errorf(\"got %d, want 4\", 1+2)\n}\n";
const RUST: &str = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn adds_up() {}\n}\n";
/// `go test` compiles first, which takes a while on a busy Mac.
const RUN: Duration = Duration::from_secs(120);

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn controller() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn editor_view() -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*controller()?, &root().join(FILE))
}

fn rows() -> Vec<(usize, String, ide_test::State)> {
    controller()
        .and_then(|c| crate::tests_panel::panel_of(&c))
        .map(|p| p.shown())
        .unwrap_or_default()
}

fn state(name: &str) -> Option<ide_test::State> {
    rows()
        .into_iter()
        .find(|(_, label, _)| label == name)
        .map(|(_, _, s)| s)
}

pub(crate) fn run() {
    helpers::open_project(
        SCENARIO,
        PROJECT,
        &[
            ("go.mod", "module example.com/sample\n\ngo 1.22\n"),
            (FILE, SOURCE),
            (
                "Cargo.toml",
                "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            ),
            ("src/lib.rs", RUST),
        ],
        listed,
    );
}

fn listed(controller: Retained<WindowController>) {
    controller.activate_panel(Panel::Tests);
    wait_within(
        RUN,
        || state("TestFail").is_some() && state("adds_up").is_some(),
        move |found| {
            check(
                "the Tests panel lists the Go package's tests",
                found && state("TestPass") == Some(ide_test::State::NotRun),
                format!("{:?}", rows()),
            );
            let shown = crate::tests_panel::panel_of(&controller).is_some_and(|p| p.tree_shown());
            check(
                "before any run the panel shows its tests as not run, the Rust ones from the sources",
                shown && state("adds_up") == Some(ide_test::State::NotRun),
                format!("tree shown {shown}, {:?}", rows()),
            );
            controller.open_file(&root().join(FILE));
            wait_within(Duration::from_secs(10), || editor_view().is_some(), |_| gutter());
        },
    );
}

fn gutter() {
    let Some(view) = editor_view() else {
        return give_up(SCENARIO, "the test file opens in an editor");
    };
    let fail_line = 10;
    let mark = crate::tests_panel::gutter_mark(&view, fail_line);
    check(
        "the gutter has a symbol at each test",
        mark == Some(ide_test::State::NotRun) && crate::tests_panel::gutter_mark(&view, 4).is_some(),
        format!("{mark:?}"),
    );
    let ran = crate::tests_panel::gutter_click(&view, fail_line, false);
    wait_within(
        RUN,
        || state("TestFail") == Some(ide_test::State::Failed),
        move |failed| {
            let marks = editor_view().map(|v| ide_ui_lsp::marks_of(&*v)).unwrap_or_default();
            let marked = editor_view().is_some_and(|v| {
                let start = v.line_start(11).unwrap_or(usize::MAX);
                marks
                    .iter()
                    .any(|(range, message)| range.start == start && message.contains("want 4"))
            });
            check(
                "a click on the symbol runs the test, the panel shows it failed and the failing line is marked",
                ran && failed && marked,
                format!("{:?} / {marks:?}", rows()),
            );
            if let Some(window) = controller().and_then(|c| c.window()) {
                snapshot(&window, "tests-panel");
            }
            at_cursor();
        },
    );
}

fn at_cursor() {
    let (Some(view), Some(controller)) = (editor_view(), controller()) else {
        return give_up(SCENARIO, "the test file opens in an editor in the folder's window");
    };
    if let Some(window) = view.window() {
        window.makeFirstResponder(Some(&view));
    }
    let inside = view.line_start(6).unwrap_or(0);
    view.setSelectedRange(NSRange::new(inside, 0));
    crate::tests_panel::run_at_cursor(&controller);
    wait_within(
        RUN,
        || state("TestPass") == Some(ide_test::State::Passed),
        |passed| {
            check(
                "the test at the caret runs from the Debug menu's command and passes",
                passed && state("TestFail") == Some(ide_test::State::Failed),
                format!("{:?}", rows()),
            );
            debugged();
        },
    );
}

/// Debugging the test at the caret starts a session named after it in the Debug panel, through Delve.
fn debugged() {
    let Some(window) = controller() else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    crate::tests_panel::debug_at_cursor(&window);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let polled = seen.clone();
    wait_within(
        RUN,
        move || {
            let sessions = controller().map(|c| crate::debug::activity(&c)).unwrap_or_default();
            let found = sessions.iter().any(|(_, name, _)| name.contains("TestPass"));
            *polled.borrow_mut() = sessions;
            found
        },
        move |started| {
            check(
                "debugging the test at the caret starts a session for it",
                started,
                format!("{:?}", seen.borrow()),
            );
            finish(SCENARIO);
        },
    );
}
