//! The `lsp` scenario: a tiny cargo project with a type error, served by the real rust-analyzer. Diagnostics,
//! hover, completion, definition, references, rename across two files and formatting, each through the code its
//! command runs. Without rust-analyzer on this Mac it reports itself skipped. Every step waits for the server to
//! settle and then for its answer, each within a generous bound, and reports how long it waited. Its commands follow
//! in `lsp_commands`.

use ide_core::LockIgnoringPoison;
use std::path::PathBuf;
use std::time::Duration;

use objc2::rc::Retained;
use objc2_foundation::NSRange;

use super::{check, finish, give_up, give_up_saying, helpers, skip, snapshot};
use crate::editor::{self, EditorTextView};
use crate::window::{self, WindowController};

pub(super) const SCENARIO: &str = "lsp";
/// The longest the server may take to start, to index the project or to settle before a request.
pub(super) const WAIT: Duration = Duration::from_secs(180);
/// The longest an answer may take once the server settled.
pub(super) const ANSWER: Duration = Duration::from_secs(60);
/// How long the server must report no work to count as settled.
const SETTLE: Duration = Duration::from_secs(3);
/// How long the server may stay settled without the expected diagnostics before the wait for them ends.
const SILENCE: Duration = Duration::from_secs(10);
/// The time the whole scenario may take, below the runner's limit for it in `xtask/src/selftest.rs`, so that it still
/// writes its report when every wait runs to its bound.
const BUDGET: Duration = Duration::from_secs(540);

static END: helpers::Deadline = helpers::Deadline::new();

const MAIN: &str = "src/main.rs";
const UTIL: &str = "src/util.rs";
const MAIN_TEXT: &str = "mod util;\n\nfn main() {\n    let total: u32 = util::double(21);\n    let wrong: u32 = \"text\";\n    let _ = (total, wrong);\n}\n";
const UTIL_TEXT: &str = "pub fn double(n: u32) -> u32 {\n    n * 2\n}\n";

const PROJECT: &str = "lsp-project";
/// A turn of the run loop between the edits before formatting and the formatting, so that each is an undo step of
/// its own; there is no state to wait for.
const UNDO_TURN: Duration = Duration::from_millis(300);

pub(super) fn root() -> PathBuf {
    helpers::root(PROJECT)
}

pub(super) fn project_window() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn view(file: &str) -> Option<Retained<EditorTextView>> {
    let controller = project_window()?;
    editor::editor_for(&controller, &root().join(file))
}

pub(super) fn main_view() -> Option<Retained<EditorTextView>> {
    view(MAIN)
}

/// The UTF-16 offset of the `nth` occurrence of `needle` in `view`'s text, plus `into` code units.
pub(super) fn offset_of(view: &EditorTextView, needle: &str, nth: usize, into: usize) -> Option<usize> {
    let text = view.document().text().to_string();
    let byte = text.match_indices(needle).nth(nth)?.0;
    Some(text[..byte].encode_utf16().count() + into)
}

pub(super) fn put_cursor(view: &EditorTextView, at: usize) {
    view.setSelectedRange(NSRange::new(at, 0));
}

/// How a wait ended: whether its condition held, how long the server took to settle before the request, and how
/// long the condition took after it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Waited {
    pub(super) ok: bool,
    settled: Option<Duration>,
    took: Duration,
}

impl std::fmt::Display for Waited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(settled) = self.settled {
            write!(f, "settled after {:.1} s, ", settled.as_secs_f64())?;
        }
        write!(f, "waited {:.1} s", self.took.as_secs_f64())
    }
}

/// Calls `then` with how the wait ended, as soon as `ready` holds or once `within` or the scenario's budget passed.
pub(super) fn poll(within: Duration, ready: impl Fn() -> bool + 'static, then: impl FnOnce(Waited) + 'static) {
    END.poll(within, ready, move |ok, took| {
        then(Waited {
            ok,
            settled: None,
            took,
        });
    });
}

/// Whether the server has reported no work for `quiet`.
fn quiet(quiet: Duration) -> bool {
    main_view()
        .and_then(|v| ide_ui_lsp::quiet_for(&*v))
        .is_some_and(|q| q >= quiet)
}

/// Waits for the server to settle, then makes `request` and waits for `ready`.
pub(super) fn ask(
    request: impl FnOnce() + 'static,
    ready: impl Fn() -> bool + 'static,
    then: impl FnOnce(Waited) + 'static,
) {
    poll(
        WAIT,
        || quiet(SETTLE),
        move |settling| {
            request();
            poll(ANSWER, ready, move |answer| {
                then(Waited {
                    settled: Some(settling.took),
                    ..answer
                });
            });
        },
    );
}

/// Puts the cursor of main.rs at `at` and runs `command` there.
fn at_main<R: 'static>(at: usize, command: fn(&dyn ide_ui_lsp::editor::Editor) -> R) -> impl FnOnce() + 'static {
    move || {
        if let Some(view) = main_view() {
            put_cursor(&view, at);
            let _ = command(&*view);
        }
    }
}

pub(super) fn has_mark(code: &'static str) -> impl Fn() -> bool + 'static {
    move || main_view().is_some_and(|v| ide_ui_lsp::marks_of(&*v).iter().any(|(_, m)| m.contains(code)))
}

/// The text of the editor before a step, to compare with after its undo.
static BEFORE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn remember(view: &EditorTextView) {
    *BEFORE.lock_ignoring_poison() = view.document().text().to_string();
}

fn remembered() -> String {
    BEFORE.lock_ignoring_poison().clone()
}

fn picture(name: &str) {
    if let Some(window) = project_window().and_then(|c| c.window()) {
        snapshot(&window, name);
    }
}

/// Ends the scenario with the open editors saved, so quitting asks nothing.
pub(super) fn done() {
    let saved = editor::save_all();
    check(
        "every editor is saved before quitting",
        saved,
        format!("{} unsaved", editor::dirty_count()),
    );
    finish(SCENARIO);
}

pub(crate) fn run() {
    END.start(BUDGET);
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"selftest\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (MAIN, MAIN_TEXT),
        (UTIL, UTIL_TEXT),
    ];
    helpers::open_project(SCENARIO, PROJECT, &files, opened);
}

fn opened(controller: Retained<WindowController>) {
    controller.open_file(&root().join(MAIN));
    poll(
        WAIT,
        || main_view().is_some_and(|v| ide_ui_lsp::server_running(&*v) != Ok(false)),
        started,
    );
}

fn started(waited: Waited) {
    let Some(view) = main_view() else {
        return give_up(SCENARIO, "main.rs opens in an editor");
    };
    match ide_ui_lsp::server_running(&*view) {
        Err(why) if why == "missing" => {
            skip(
                SCENARIO,
                "rust-analyzer is there",
                "no rust-analyzer through rustup, on PATH or in the tools",
            );
        }
        Ok(true) => {
            check(
                "rust-analyzer starts for the trusted folder",
                true,
                format!("{waited}: {}", ide_ui_lsp::status::shown(&*view).1),
            );
            let typed = has_mark("E0308");
            poll(WAIT, move || typed() || quiet(SILENCE), diagnosed);
        }
        other => {
            give_up_saying(
                SCENARIO,
                "rust-analyzer starts for the trusted folder",
                format!("{waited}: {other:?}"),
            );
        }
    }
}

fn diagnosed(waited: Waited) {
    let Some(view) = main_view() else { return };
    let marks = ide_ui_lsp::marks_of(&*view);
    let wrong = offset_of(&view, "\"text\"", 0, 0).unwrap_or(0);
    check(
        "a type error is underlined where it is",
        marks
            .iter()
            .any(|(range, m)| range.contains(&wrong) && m.contains("E0308")),
        format!("{waited}: {marks:?}"),
    );
    let lines = ide_ui_lsp::line_marks(&*view);
    check(
        "the line number column marks its line",
        lines.contains_key(&4),
        format!("{:?}", lines.keys().collect::<Vec<_>>()),
    );
    let (problems, server) = ide_ui_lsp::status::shown(&*view);
    check(
        "the status bar counts the problems",
        problems.contains(splat_i18n::t("1 Fehler")) || problems.contains("Fehler") || problems.contains("error"),
        format!("{problems} · {server}"),
    );
    let wrong_rect = ide_ui_lsp::rect_of(&*view, wrong..wrong + 1);
    let hit = wrong_rect.map(|r| {
        let center = objc2_foundation::NSPoint::new(r.origin.x + r.size.width / 2.0, r.origin.y + r.size.height / 2.0);
        view.characterIndexForInsertionAtPoint(center)
    });
    check(
        "a point on the text finds its character, as hover and ⌘-click do",
        hit.is_some_and(|i| i.abs_diff(wrong) <= 1),
        format!("{hit:?} for {wrong}"),
    );
    check(
        "the editor still uses TextKit 2",
        view.textLayoutManager().is_some(),
        "",
    );
    picture("lsp-diagnostics");
    let Some(at) = offset_of(&view, "double", 0, 2) else {
        return;
    };
    ask(
        at_main(at, ide_ui_lsp::hover_at_cursor),
        || main_view().is_some_and(|v| ide_ui_lsp::hover_text(&*v).is_some()),
        hovered,
    );
}

fn hovered(waited: Waited) {
    let Some(view) = main_view() else { return };
    let text = ide_ui_lsp::hover_text(&*view).unwrap_or_default();
    check(
        "hovering shows the function's signature",
        waited.ok && text.contains("fn double"),
        format!("{waited}: {}", text.lines().take(4).collect::<Vec<_>>().join(" / ")),
    );
    picture("lsp-hover");
    let tokens = ide_ui_lsp::token_count(&*view);
    check(
        "semantic tokens colour what the lexer cannot tell",
        tokens > 0,
        format!("{tokens} tokens"),
    );
    let Some(end) = offset_of(&view, "let _ = (total, wrong);", 0, 23) else {
        return;
    };
    put_cursor(&view, end);
    editor::type_newline(&view);
    for c in "let more = util::d".chars() {
        editor::type_text(&view, &c.to_string());
    }
    let cursor = view.selectedRange().location;
    ask(
        at_main(cursor, ide_ui_lsp::complete),
        || {
            main_view().is_some_and(|v| {
                ide_ui_lsp::completion::shown(&*v)
                    .iter()
                    .any(|l| l.starts_with("double"))
            })
        },
        completed,
    );
}

fn completed(waited: Waited) {
    let Some(view) = main_view() else { return };
    let shown = ide_ui_lsp::completion::shown(&*view);
    check(
        "completion lists the module's function",
        waited.ok,
        format!(
            "{waited}: {}",
            shown.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
        ),
    );
    if let Some(panel) = ide_ui_lsp::completion::panel() {
        snapshot(&panel, "lsp-completion-list");
    }
    picture("lsp-completion");
    let typed = view.document().text().to_string();
    let accepted = ide_ui_lsp::completion::accept(&*view, "double");
    let text = view.document().text().to_string();
    check(
        "accepting inserts the call",
        accepted && text.contains("let more = util::double("),
        text.lines().nth(6).unwrap_or_default().to_string(),
    );
    if let Some(undo) = view.undoManager() {
        undo.undo();
        let undone = view.document().text().to_string();
        check(
            "one undo takes the whole completion back",
            undone == typed,
            undone.lines().nth(6).unwrap_or_default().to_string(),
        );
        undo.redo();
    }
    let Some(at) = offset_of(&view, "double", 0, 2) else {
        return;
    };
    ask(
        at_main(at, ide_ui_lsp::definition),
        || {
            project_window().is_some_and(|c| {
                editor::editor_for(&c, &root().join(UTIL)).is_some_and(|v| v.selectedRange().location == 7)
            })
        },
        defined,
    );
}

fn defined(waited: Waited) {
    let selection = view(UTIL).map(|v| v.selectedRange().location);
    check(
        "go to definition opens util.rs at the function",
        waited.ok,
        format!("{waited}: {selection:?}"),
    );
    let Some(controller) = project_window() else { return };
    controller.open_file(&root().join(MAIN));
    let Some(view) = main_view() else { return };
    let Some(at) = offset_of(&view, "double", 0, 2) else {
        return;
    };
    ask(
        at_main(at, ide_ui_lsp::references),
        || !ide_ui_lsp::listed().is_empty(),
        referenced,
    );
}

fn referenced(waited: Waited) {
    let listed = ide_ui_lsp::listed();
    check(
        "find uses lists both files",
        waited.ok
            && listed.iter().any(|l| l.starts_with("src/util.rs:1"))
            && listed.iter().any(|l| l.starts_with("src/main.rs:4")),
        format!("{waited}: {}", listed.join(" | ")),
    );
    picture("lsp-references");
    ide_ui_lsp::close_list();
    let Some(controller) = project_window() else { return };
    if controller.select_tab(&root().join(UTIL).to_string_lossy())
        && let Some(id) = controller.selected_tab()
    {
        controller.close_tab(id);
    }
    controller.open_file(&root().join(MAIN));
    let Some(view) = main_view() else { return };
    let Some(at) = offset_of(&view, "double", 0, 2) else {
        return;
    };
    ask(
        at_main(at, ide_ui_lsp::rename),
        || project_window().is_some_and(|c| c.has_sheet()),
        rename_asked,
    );
}

fn rename_asked(waited: Waited) {
    check(
        "renaming asks for the new name in a sheet",
        waited.ok,
        waited.to_string(),
    );
    picture("lsp-rename");
    if let Some(window) = project_window().and_then(|c| c.window())
        && let Some(sheet) = window.attachedSheet()
    {
        window.endSheet_returnCode(&sheet, objc2_app_kit::NSAlertSecondButtonReturn);
    }
    let Some(view) = main_view() else { return };
    let Some(at) = offset_of(&view, "double", 0, 2) else {
        return;
    };
    remember(&view);
    ask(
        move || {
            if let Some(view) = main_view() {
                ide_ui_lsp::rename_to(&*view, at, "twice");
            }
        },
        || {
            let disk = std::fs::read_to_string(root().join(UTIL)).unwrap_or_default();
            disk.contains("pub fn twice")
                && main_view().is_some_and(|v| !v.document().text().to_string().contains("double"))
        },
        renamed,
    );
}

fn renamed(waited: Waited) {
    let Some(view) = main_view() else { return };
    let text = view.document().text().to_string();
    let disk = std::fs::read_to_string(root().join(UTIL)).unwrap_or_default();
    check(
        "rename changes the open file and the closed one",
        waited.ok && text.contains("util::twice(21)") && disk.contains("pub fn twice"),
        format!(
            "{waited}: {} | {}",
            text.lines().nth(3).unwrap_or_default().trim(),
            disk.lines().next().unwrap_or_default()
        ),
    );
    if let Some(undo) = view.undoManager() {
        undo.undo();
        let undone = view.document().text().to_string() == remembered();
        undo.redo();
        check("the open file's rename is one undo step", undone, "");
    }
    let end = view.document().text().len_utf16();
    let mut edits = vec![(end..end, "fn  ugly( )  { }\n".to_string())];
    if let Some(call) = offset_of(&view, "util::twice(n)", 0, 14) {
        edits.push((call..call, ";".into()));
    }
    ide_ui_lsp::apply_text_edits(&*view, edits);
    helpers::after(UNDO_TURN, format);
}

/// Formats in a turn of the run loop of its own, so its undo step stays apart from the edit before it.
fn format() {
    let Some(view) = main_view() else { return };
    remember(&view);
    ask(
        || {
            if let Some(view) = main_view() {
                ide_ui_lsp::format(&*view, None);
            }
        },
        || main_view().is_some_and(|v| v.document().text().to_string().contains("fn ugly() {}")),
        formatted,
    );
}

fn formatted(waited: Waited) {
    let Some(view) = main_view() else { return };
    let text = view.document().text().to_string();
    check(
        "format document tidies the file",
        waited.ok,
        format!("{waited}: {}", text.lines().last().unwrap_or_default()),
    );
    if let Some(undo) = view.undoManager() {
        undo.undo();
        let after = view.document().text().to_string();
        undo.redo();
        let detail = after.lines().rev().take(2).collect::<Vec<_>>().join(" / ");
        check("formatting is one undo step", after == remembered(), detail);
    }
    picture("lsp-formatted");
    if let Some(at) = offset_of(&view, "let total", 0, 4) {
        put_cursor(&view, at);
    }
    ask(
        || {},
        || {
            main_view().is_some_and(|v| {
                crate::editor::selftest::from_server(&v) && ide_ui_lsp::status::crumbs(&*v).contains("main")
            })
        },
        outlined,
    );
}

fn outlined(waited: Waited) {
    let Some(view) = main_view() else { return };
    let crumbs = ide_ui_lsp::status::crumbs(&*view);
    check(
        "the breadcrumbs name the function around the cursor",
        waited.ok,
        format!("{waited}: {crumbs}"),
    );
    crate::editor::selftest::fold_at_cursor(&view);
    let shown = crate::editor::selftest::laid_out_lines(&view);
    check(
        "the server's folding range folds the function's body",
        crate::editor::selftest::from_server(&view) && shown.contains(&2) && !shown.contains(&3),
        format!("{shown:?}"),
    );
    picture("lsp-folded");
    crate::editor::selftest::unfold_all(&view);
    ide_ui_lsp::apply_text_edits(&*view, vec![(0..0, "mod extra;\n".into())]);
    poll(WAIT, has_mark("E0583"), unresolved);
}

fn unresolved(waited: Waited) {
    check("a module without its file is reported", waited.ok, waited.to_string());
    if let Err(error) = std::fs::write(root().join("src/extra.rs"), "pub fn more() {}\n") {
        return give_up_saying(SCENARIO, "the module's file is written", error.to_string());
    }
    let unresolved = has_mark("E0583");
    poll(WAIT, move || !unresolved(), watched);
}

fn watched(waited: Waited) {
    let marks = main_view().map(|v| ide_ui_lsp::marks_of(&*v)).unwrap_or_default();
    check(
        "a file made on disk reaches the server through the folder's watcher",
        waited.ok,
        format!("{waited}: {marks:?}"),
    );
    super::lsp_commands::run();
}
