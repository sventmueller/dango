//! The `lsp-actions` and `lsp-actions-go` scenarios (B14, E82, E84): a quick fix and signature help with the real
//! rust-analyzer and gopls. The cursor rests on a name that needs an import, the lightbulb shows on its line, the
//! actions open as a menu at the cursor and choosing the import applies it as one undo step; typing a call opens the
//! signature window with the first parameter set apart, a comma moves to the second, Esc closes it, and moving the
//! cursor out of the call closes it again. Without the server on this Mac the scenario reports itself skipped.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::sel;
use objc2_foundation::NSRange;

use super::{check, finish, helpers, snapshot};
use crate::editor::{self, EditorTextView};
use crate::window::{self, WindowController};

/// A turn of the run loop after the undo and redo of the quick fix, so that typing the call starts an undo step of
/// its own; there is no state to wait for.
const UNDO_TURN: Duration = Duration::from_millis(300);
/// The longest the server may take to start, to index the project or to settle before a request.
const WAIT: Duration = Duration::from_secs(180);
/// The longest an answer may take once the server settled.
const ANSWER: Duration = Duration::from_secs(60);
/// How long the server must report no work to count as settled.
const SETTLE: Duration = Duration::from_secs(3);
/// Below the runner's limit for the scenario in `xtask/src/selftest.rs`, so it still writes its report.
const BUDGET: Duration = Duration::from_secs(540);

/// What one language's run uses.
struct Case {
    scenario: &'static str,
    server: &'static str,
    folder: &'static str,
    files: &'static [(&'static str, &'static str)],
    main: &'static str,
    /// The name the quick fix is for; the cursor rests inside it.
    unresolved: &'static str,
    /// How the import's action title starts, and what the text holds once it is applied.
    fix_title: &'static [&'static str],
    fixed: &'static str,
    /// Where a new line with a call goes, after this text; what is typed there up to the call's parenthesis.
    call_after: &'static str,
    call: &'static str,
    /// The first and the second parameter as the window sets them apart.
    first: &'static str,
    second: &'static str,
}

const RUST: Case = Case {
    scenario: "lsp-actions",
    server: "rust-analyzer",
    folder: "actions-rust",
    files: &[
        (
            "Cargo.toml",
            "[package]\nname = \"actions\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "src/main.rs",
            "mod util;\n\nfn main() {\n    let map: HashMap<u32, u32> = HashMap::new();\n    let total = util::scale(2, 3);\n    let _ = (map, total);\n}\n",
        ),
        (
            "src/util.rs",
            "/// Multiplies.\npub fn scale(value: u32, factor: u32) -> u32 {\n    value * factor\n}\n",
        ),
    ],
    main: "src/main.rs",
    unresolved: "HashMap<u32",
    fix_title: &["Import `std::collections::HashMap`"],
    fixed: "use std::collections::HashMap;",
    call_after: "let _ = (map, total);",
    call: "let more = util::scale(",
    first: "value: u32",
    second: "factor: u32",
};

const GO: Case = Case {
    scenario: "lsp-actions-go",
    server: "gopls",
    folder: "actions-go",
    files: &[
        ("go.mod", "module example.com/actions\n\ngo 1.21\n"),
        (
            "main.go",
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(strings.ToUpper(\"a\"))\n\tfmt.Println(scale(2, 3))\n}\n\n// scale multiplies.\nfunc scale(value int, factor int) int {\n\treturn value * factor\n}\n",
        ),
    ],
    main: "main.go",
    unresolved: "strings.To",
    fix_title: &["Add import:"],
    fixed: "\"strings\"",
    call_after: "fmt.Println(scale(2, 3))",
    call: "x := scale(",
    first: "value int",
    second: "factor int",
};

static CASE: OnceLock<&'static Case> = OnceLock::new();
static END: helpers::Deadline = helpers::Deadline::new();

fn case() -> &'static Case {
    CASE.get().copied().unwrap_or(&RUST)
}

fn root() -> PathBuf {
    helpers::root(case().folder)
}

fn project_window() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn main_view() -> Option<Retained<EditorTextView>> {
    let controller = project_window()?;
    editor::editor_for(&controller, &root().join(case().main))
}

fn offset_of(view: &EditorTextView, needle: &str, into: usize) -> Option<usize> {
    let text = view.document().text().to_string();
    let byte = text.find(needle)?;
    Some(text[..byte].encode_utf16().count() + into)
}

fn put_cursor(view: &EditorTextView, at: usize) {
    view.setSelectedRange(NSRange::new(at, 0));
}

fn picture(name: &str) {
    if let Some(window) = project_window().and_then(|c| c.window()) {
        snapshot(&window, name);
    }
}

fn quiet() -> bool {
    main_view()
        .and_then(|v| crate::lsp::quiet_for(&v))
        .is_some_and(|q| q >= SETTLE)
}

/// Ends the scenario with the editors saved, so quitting asks nothing.
fn done() {
    let saved = editor::save_all();
    check(
        "every editor is saved before quitting",
        saved,
        editor::dirty_count().to_string(),
    );
    finish(case().scenario);
}

pub fn run_rust() {
    run(&RUST);
}

pub fn run_go() {
    run(&GO);
}

fn run(chosen: &'static Case) {
    let _ = CASE.set(chosen);
    END.start(BUDGET);
    helpers::open_project(chosen.scenario, chosen.folder, chosen.files, opened);
}

fn opened(controller: Retained<WindowController>) {
    let chosen = case();
    controller.open_file(&root().join(chosen.main));
    END.poll(
        WAIT,
        || main_view().is_some_and(|v| crate::lsp::server_running(&v) != Ok(false)),
        |_, _| started(),
    );
}

fn started() {
    let Some(view) = main_view() else {
        check("the file opens in an editor", false, "");
        finish(case().scenario);
        return;
    };
    match crate::lsp::server_running(&view) {
        Err(why) if why == "missing" => {
            check("skipped: the language server is missing", true, case().server);
            done();
        }
        Ok(true) => {
            check("the language server starts", true, case().server);
            END.poll(WAIT, quiet, |_, took| rest_on_name(took));
        }
        other => {
            check("the language server starts", false, format!("{other:?}"));
            done();
        }
    }
}

/// Puts the cursor into the unresolved name and waits for the bulb on its line.
fn rest_on_name(settled: Duration) {
    let Some(view) = main_view() else { return };
    let Some(at) = offset_of(&view, case().unresolved, 2) else {
        check("the unresolved name is in the file", false, "");
        done();
        return;
    };
    put_cursor(&view, at - 1);
    put_cursor(&view, at);
    let line = view.document().text().to_string()[..]
        .split('\n')
        .position(|l| l.contains(case().unresolved))
        .unwrap_or(0);
    END.poll(
        ANSWER,
        move || main_view().is_some_and(|v| crate::lsp::bulb_line(&v) == Some(line)),
        move |ok, took| {
            let Some(view) = main_view() else { return };
            let bulb = crate::lsp::code_action_menu::bulb(&view);
            check(
                "a lightbulb shows on the cursor's line once it rests",
                ok && bulb.is_some(),
                format!(
                    "settled after {:.1} s, waited {:.1} s: {bulb:?}",
                    settled.as_secs_f64(),
                    took.as_secs_f64()
                ),
            );
            check(
                "VoiceOver names the lightbulb",
                bulb.is_some_and(|(label, _)| label == splat_i18n::t("Aktionen für diese Stelle")),
                "",
            );
            picture(&format!("{}-bulb", case().scenario));
            typing_hides_the_bulb(&view);
        },
    );
}

/// A keystroke takes the bulb away until the cursor rests again.
fn typing_hides_the_bulb(view: &EditorTextView) {
    let before = view.document().text().to_string();
    let cursor = view.selectedRange().location;
    editor::type_text(view, "x");
    let gone = crate::lsp::bulb_line(view).is_none();
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    put_cursor(view, cursor);
    check(
        "a keystroke takes the bulb away at once",
        gone && view.document().text().to_string() == before,
        "",
    );
    crate::lsp::code_action_menu::forget();
    let wanted = case().fix_title;
    END.poll(
        ANSWER,
        || main_view().is_some_and(|v| crate::lsp::bulb_line(&v).is_some()),
        move |_, _| {
            if let Some(view) = main_view() {
                crate::lsp::code_actions(&view, crate::lsp::ActionKind::Fixes);
            }
            END.poll(
                ANSWER,
                move || {
                    main_view()
                        .and_then(|v| crate::lsp::code_action_menu::shown(&v))
                        .is_some_and(|titles| titles.iter().any(|t| wanted.iter().any(|w| t.starts_with(w))))
                },
                listed,
            );
        },
    );
}

fn listed(ok: bool, took: Duration) {
    let Some(view) = main_view() else { return };
    let titles = crate::lsp::code_action_menu::shown(&view).unwrap_or_default();
    check(
        "the quick fix menu at the cursor lists the import",
        ok,
        format!("waited {:.1} s: {}", took.as_secs_f64(), titles.join(" | ")),
    );
    let before = view.document().text().to_string();
    let chosen = case()
        .fix_title
        .iter()
        .find(|w| titles.iter().any(|t| t.starts_with(**w)))
        .is_some_and(|w| crate::lsp::code_action_menu::choose(w));
    END.poll(
        ANSWER,
        || main_view().is_some_and(|v| v.document().text().to_string().contains(case().fixed)),
        move |ok, took| {
            let Some(view) = main_view() else { return };
            let after = view.document().text().to_string();
            check(
                "choosing the quick fix adds the import",
                chosen && ok,
                format!(
                    "waited {:.1} s: {}",
                    took.as_secs_f64(),
                    after.lines().take(4).collect::<Vec<_>>().join(" / ")
                ),
            );
            if let Some(undo) = view.undoManager() {
                undo.undo();
                let undone = view.document().text().to_string() == before;
                undo.redo();
                check("one undo takes the quick fix back", undone, "");
            }
            picture(&format!("{}-fixed", case().scenario));
            helpers::after(UNDO_TURN, call);
        },
    );
}

/// Types a call on a new line and waits for the signature window.
fn call() {
    let Some(view) = main_view() else { return };
    let Some(end) = offset_of(&view, case().call_after, case().call_after.encode_utf16().count()) else {
        check("the line for the call is in the file", false, "");
        done();
        return;
    };
    put_cursor(&view, end);
    editor::type_newline(&view);
    for c in case().call.chars() {
        editor::type_text(&view, &c.to_string());
    }
    END.poll(
        ANSWER,
        || main_view().and_then(|v| crate::lsp::signatures::shown(&v)).is_some(),
        |ok, took| {
            let Some(view) = main_view() else { return };
            let shown = crate::lsp::signatures::shown(&view);
            check(
                "typing the call's parenthesis shows its signature",
                ok,
                format!("waited {:.1} s: {shown:?}", took.as_secs_f64()),
            );
            check(
                "the first parameter is set apart and read out",
                shown.as_ref().is_some_and(|(_, active)| active == case().first),
                format!("{shown:?}"),
            );
            if let Some(panel) = crate::lsp::signatures::panel() {
                snapshot(&panel, &format!("{}-signature-window", case().scenario));
            }
            picture(&format!("{}-signature", case().scenario));
            for c in "2, ".chars() {
                editor::type_text(&view, &c.to_string());
            }
            let second = case().second;
            END.poll(
                ANSWER,
                move || {
                    main_view()
                        .and_then(|v| crate::lsp::signatures::shown(&v))
                        .is_some_and(|(_, active)| active == second)
                },
                second_parameter,
            );
        },
    );
}

fn second_parameter(ok: bool, took: Duration) {
    let Some(view) = main_view() else { return };
    let shown = crate::lsp::signatures::shown(&view);
    check(
        "after a comma the second parameter is set apart",
        ok,
        format!("waited {:.1} s: {shown:?}", took.as_secs_f64()),
    );
    let took_esc = crate::lsp::command(&view, sel!(cancelOperation:));
    check(
        "Esc closes the signature window",
        took_esc && crate::lsp::signatures::shown(&view).is_none(),
        "",
    );
    crate::lsp::signature_help(&view);
    END.poll(
        ANSWER,
        || main_view().and_then(|v| crate::lsp::signatures::shown(&v)).is_some(),
        |ok, _| {
            check("the menu item shows the signature again", ok, "");
            let Some(view) = main_view() else { return };
            if let Some(start) = offset_of(&view, case().call, 0) {
                put_cursor(&view, start);
            }
            END.poll(
                ANSWER,
                || main_view().is_some_and(|v| crate::lsp::signatures::shown(&v).is_none()),
                |ok, took| {
                    check(
                        "leaving the call closes the signature window",
                        ok,
                        format!("waited {:.1} s", took.as_secs_f64()),
                    );
                    if let Some(view) = main_view() {
                        let end = view.document().text().len_utf16();
                        put_cursor(&view, end);
                        if let Some(undo) = view.undoManager() {
                            while view.document().text().to_string().contains(case().call) && undo.canUndo() {
                                undo.undo();
                            }
                        }
                    }
                    done();
                },
            );
        },
    );
}
