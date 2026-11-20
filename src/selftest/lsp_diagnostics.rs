//! The `lsp-diagnostics` scenario: an error in a Python, a JSON and a Swift file shows among the editor's diagnostics,
//! from servers that offer pulled diagnostics only by registering them after the start (basedpyright, sourcekit-lsp)
//! or check nothing without being told to (the JSON server). It installs missing servers from the network, so it is
//! not offline.

use std::path::PathBuf;
use std::time::Duration;

use objc2::rc::Retained;

use super::helpers::{self, wait_within};
use super::{check, finish, give_up};
use crate::editor::{self, EditorTextView};
use crate::window::{self, WindowController};

const SCENARIO: &str = "lsp-diagnostics";
const PROJECT: &str = "lsp-diagnostics-project";
/// Long enough for npm to install a server on a slow line.
const INSTALL: Duration = Duration::from_secs(420);
const ANSWER: Duration = Duration::from_secs(90);

/// A file, its text with one error, a part of the message the error must show with, and the check's name.
struct Case {
    file: &'static str,
    text: &'static str,
    message: &'static str,
    name: &'static str,
}

const CASES: &[Case] = &[
    Case {
        file: "a.py",
        text: "def twice(value: int) -> int:\n    return value * 2\n\nprint(twice(\"21\"))\n",
        message: "Literal['21']",
        name: "a type error in Python shows, pulled from basedpyright after it registered pulling",
    },
    Case {
        file: "data.json",
        text: "{\"a\": }\n",
        message: "Value expected",
        name: "a syntax error in JSON shows",
    },
    Case {
        file: "a.swift",
        text: "let x: Int = \"a\"\n",
        message: "String",
        name: "a type error in Swift shows, pulled from sourcekit-lsp after it registered pulling",
    },
];

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn view(file: &str) -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*window::for_project(&root())?, &root().join(file))
}

pub(crate) fn run() {
    let files: Vec<(&str, &str)> = CASES.iter().map(|c| (c.file, c.text)).collect();
    helpers::open_project(SCENARIO, PROJECT, &files, |controller| open_case(controller, 0));
}

/// Opens case `index` and waits for its server, installing it when it is missing.
fn open_case(controller: Retained<WindowController>, index: usize) {
    if let Some(case) = CASES.get(index) {
        return wait_for_server(controller, index, case);
    }
    finish(SCENARIO);
}

fn wait_for_server(controller: Retained<WindowController>, index: usize, case: &'static Case) {
    controller.open_file(&root().join(case.file));
    let file = case.file;
    wait_within(
        ANSWER,
        move || view(file).is_some_and(|v| ide_ui_lsp::server_running(&*v).is_ok_and(|r| r) || missing(file)),
        move |_| {
            if missing(file)
                && let Some(view) = view(file)
            {
                ide_ui_lsp::status_action(&*view);
            }
            wait_within(
                INSTALL,
                move || view(file).is_some_and(|v| ide_ui_lsp::server_running(&*v) == Ok(true)),
                move |running| {
                    if !running {
                        let state = view(file).map(|v| ide_ui_lsp::server_running(&*v));
                        check(CASES[index].name, false, format!("the server does not run: {state:?}"));
                        return give_up(SCENARIO, "the file's language server runs");
                    }
                    diagnosed(controller, index);
                },
            );
        },
    );
}

fn missing(file: &str) -> bool {
    view(file).is_some_and(|v| ide_ui_lsp::server_running(&*v).is_err_and(|e| e == "missing"))
}

/// Waits for the error of case `index` among the diagnostics, then goes on with the next case.
fn diagnosed(controller: Retained<WindowController>, index: usize) {
    let case = &CASES[index];
    let (file, message) = (case.file, case.message);
    wait_within(
        ANSWER,
        move || view(file).is_some_and(|v| ide_ui_lsp::marks_of(&*v).iter().any(|(_, m)| m.contains(message))),
        move |found| {
            let marks = view(file).map(|v| ide_ui_lsp::marks_of(&*v)).unwrap_or_default();
            check(CASES[index].name, found, format!("{marks:?}"));
            open_case(controller, index + 1);
        },
    );
}
