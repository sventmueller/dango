//! The `companions` scenario (E140, E132, E109): beside TypeScript's server, ESLint checks a JavaScript file of a
//! project with ESLint 9 and an `eslint.config.ts`, its error comes with a quick fix that removes it, and Emmet offers
//! an abbreviation in an HTML file that completion takes over. It installs ESLint into the project and both servers
//! into the app's tools folder from the network, so it is not offline.

use std::path::PathBuf;
use std::time::Duration;

use objc2::rc::Retained;
use objc2_foundation::NSRange;

use super::helpers::{self, wait_within};
use super::{check, finish};
use crate::editor::{self, EditorTextView};
use crate::window::{self, WindowController};

/// The name of the scenario that runs, which its report carries.
fn scenario() -> &'static str {
    if LATEST.load(std::sync::atomic::Ordering::Relaxed) {
        "companions-eslint-latest"
    } else {
        "companions"
    }
}
const PROJECT: &str = "companions-project";
const SCRIPT: &str = "src/a.js";
const PAGE: &str = "index.html";
/// Long enough for `npm install` and building the ESLint server on a slow line.
const INSTALL: Duration = Duration::from_secs(420);
const ANSWER: Duration = Duration::from_secs(60);

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn view(file: &str) -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*window::for_project(&root())?, &root().join(file))
}

/// Whether the scenario runs against ESLint's current major version in place of 9 (A11), without Emmet.
static LATEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The `companions-eslint-latest` scenario: ESLint's part of `companions` against its current major version (A11).
pub fn run_latest() {
    LATEST.store(true, std::sync::atomic::Ordering::Relaxed);
    run();
}

pub fn run() {
    let eslint = if LATEST.load(std::sync::atomic::Ordering::Relaxed) {
        "latest"
    } else {
        "^9"
    };
    let package = format!(
        r#"{{"name":"companions","private":true,"type":"module","devDependencies":{{"eslint":"{eslint}","jiti":"^2"}}}}"#
    );
    helpers::open_project(
        scenario(),
        PROJECT,
        &[
            ("package.json", &package),
            (
                "eslint.config.ts",
                "export default [{ rules: { \"no-debugger\": \"error\" } }];\n",
            ),
            (SCRIPT, "export function f() {\n  debugger;\n  return 1;\n}\n"),
            (PAGE, "<!doctype html>\n<body>\n\n</body>\n"),
        ],
        npm_install,
    );
}

/// Installs ESLint into the project, as its developer would, before the file opens.
fn npm_install(controller: Retained<WindowController>) {
    let folder = root();
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let done = std::process::Command::new("/bin/zsh")
            .args(["-lc", "npm install --no-fund --no-audit"])
            .current_dir(&folder)
            .output()
            .map(|o| o.status.success());
        let _ = sent.send(done);
    });
    let seen = std::rc::Rc::new(std::cell::RefCell::new(None));
    let polled = seen.clone();
    wait_within(
        INSTALL,
        move || {
            if let Ok(done) = received.try_recv() {
                *polled.borrow_mut() = Some(done);
            }
            polled.borrow().is_some()
        },
        move |_| {
            let installed = matches!(*seen.borrow(), Some(Ok(true)));
            check(
                "the project gets ESLint with npm",
                installed,
                format!("{:?}", seen.borrow()),
            );
            if !installed {
                return finish(scenario());
            }
            controller.open_file(&root().join(SCRIPT));
            servers_ready(SCRIPT, "ESLint", eslint_error);
        },
    );
}

/// Installs the second server `name` of `file` and calls `then` once it runs.
fn servers_ready(file: &'static str, name: &'static str, then: fn()) {
    wait_within(
        ANSWER,
        move || {
            view(file).is_some_and(|v| {
                crate::lsp::companion_running(&v, name).is_err_and(|e| e == "missing")
                    || crate::lsp::companion_running(&v, name).is_ok()
            })
        },
        move |_| {
            if let Some(view) = view(file) {
                crate::lsp::install_companions(&view);
            }
            wait_within(
                INSTALL,
                move || view(file).is_some_and(|v| crate::lsp::companion_running(&v, name) == Ok(true)),
                move |running| {
                    let state = view(file).map(|v| crate::lsp::companion_running(&v, name));
                    check(
                        if name == "ESLint" {
                            "ESLint is installed and runs beside the file's own server"
                        } else {
                            "Emmet is installed and runs beside the file's own server"
                        },
                        running,
                        format!("{state:?}"),
                    );
                    if running { then() } else { finish(scenario()) }
                },
            );
        },
    );
}

fn eslint_error() {
    wait_within(
        ANSWER,
        || view(SCRIPT).is_some_and(|v| crate::lsp::marks_of(&v).iter().any(|(_, m)| m.contains("no-debugger"))),
        |found| {
            let marks = view(SCRIPT).map(|v| crate::lsp::marks_of(&v)).unwrap_or_default();
            check(
                "ESLint's error shows among the diagnostics, from a flat eslint.config.ts",
                found,
                format!("{marks:?}"),
            );
            if let Some(view) = view(SCRIPT) {
                let at = view.line_start(1).unwrap_or(0) + 3;
                view.setSelectedRange(NSRange::new(at, 0));
                crate::lsp::code_action_menu::forget();
                crate::lsp::code_actions(&view, crate::lsp::ActionKind::Fixes);
            }
            wait_within(
                ANSWER,
                || {
                    view(SCRIPT)
                        .and_then(|v| crate::lsp::code_action_menu::shown(&v))
                        .is_some_and(|titles| titles.iter().any(|t| t.contains("no-debugger")))
                },
                |_| quick_fix(),
            );
        },
    );
}

fn quick_fix() {
    let titles = view(SCRIPT)
        .and_then(|v| crate::lsp::code_action_menu::shown(&v))
        .unwrap_or_default();
    let wanted = titles
        .iter()
        .find(|t| t.contains("no-debugger") && t.starts_with("Disable"))
        .cloned();
    let chosen = wanted.as_deref().is_some_and(crate::lsp::code_action_menu::choose);
    wait_within(
        ANSWER,
        || view(SCRIPT).is_some_and(|v| v.document().text().to_string().contains("eslint-disable")),
        move |fixed| {
            check(
                "the quick fix of ESLint's error is offered beside the main server's and applies",
                chosen && fixed,
                titles.join(" | "),
            );
            if LATEST.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = editor::save_all();
                return finish(scenario());
            }
            if let Some(controller) = window::for_project(&root()) {
                controller.open_file(&root().join(PAGE));
            }
            servers_ready(PAGE, "Emmet", emmet);
        },
    );
}

fn emmet() {
    let Some(view) = view(PAGE) else {
        return finish(scenario());
    };
    let at = view.line_start(2).unwrap_or(0);
    view.setSelectedRange(NSRange::new(at, 0));
    editor::type_text(&view, "ul>li");
    crate::lsp::complete(&view);
    wait_within(
        ANSWER,
        || view_page().is_some_and(|v| crate::lsp::completion::shown(&v).iter().any(|l| l == "ul>li")),
        |offered| {
            let accepted = view_page().is_some_and(|v| crate::lsp::completion::accept(&v, "ul>li"));
            let text = view_page().map(|v| v.document().text().to_string()).unwrap_or_default();
            check(
                "Emmet offers the abbreviation in completion and taking it writes the elements",
                offered && accepted && text.contains("<ul>") && text.contains("<li>"),
                text,
            );
            if let Some(view) = view_page() {
                let _ = editor::save_all();
                drop(view);
            }
            finish(scenario());
        },
    );
}

fn view_page() -> Option<Retained<EditorTextView>> {
    view(PAGE)
}
