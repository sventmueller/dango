//! The `lsp-navigation` scenario (E85): this repository itself, served by the real rust-analyzer. A symbol found with
//! `#` in Open Quickly while no editor is open, which only the search's lease keeps the server running for; the
//! implementations of a trait in the list of places; and a call hierarchy opened three levels deep in the search
//! panel, with its direction switch and its menu of earlier searches. Without rust-analyzer it reports itself skipped.

use ide_core::LockIgnoringPoison;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_foundation::NSRange;

use super::{check, finish, helpers, snapshot};
use crate::editor::{self, EditorTextView};
use crate::window::{self, WindowController, search_selftest as search};

const SCENARIO: &str = "lsp-navigation";
/// The longest rust-analyzer may take to load this workspace and answer the first search.
const WAIT: Duration = Duration::from_secs(300);
/// The longest an answer may take once the server answers searches.
const ANSWER: Duration = Duration::from_secs(90);
/// Below the runner's limit in `xtask/src/selftest.rs`, so the report is written when every wait runs to its bound.
const BUDGET: Duration = Duration::from_secs(540);
/// How often a search or request that found nothing is made again while the server loads.
const AGAIN: Duration = Duration::from_secs(5);

const QUERY: &str = "#ViewState";
const TRAIT_FILE: &str = "crates/ide-lsp/src/convert.rs";
const CALLS_FILE: &str = "crates/ide-lsp/src/server.rs";

static END: helpers::Deadline = helpers::Deadline::new();
/// What Open Quickly said while it found nothing.
static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn root() -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    folder.canonicalize().unwrap_or(folder)
}

fn controller() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn view(file: &str) -> Option<Retained<EditorTextView>> {
    let controller = controller()?;
    editor::editor_for(&controller, &root().join(file))
}

fn picture(name: &str) {
    if let Some(window) = controller().and_then(|c| c.window()) {
        snapshot(&window, name);
    }
}

/// Calls `then` with whether `ready` held, as soon as it does or once `within` or the scenario's budget passed;
/// `again` runs every `AGAIN` meanwhile.
fn poll(
    within: Duration,
    ready: impl Fn() -> bool + 'static,
    again: impl Fn() + 'static,
    then: impl FnOnce(bool, Duration) + 'static,
) {
    let last = std::cell::Cell::new(Instant::now());
    let ready = move || {
        if ready() {
            return true;
        }
        if last.get().elapsed() >= AGAIN {
            again();
            last.set(Instant::now());
        }
        false
    };
    END.poll(within, ready, then);
}

fn done() {
    finish(SCENARIO);
}

/// What VoiceOver would miss in the visible windows titled `title`, or in the project's window for `None`.
fn audit(name: &'static str, title: Option<&str>) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let windows: Vec<Retained<objc2_app_kit::NSWindow>> = match title {
        None => controller().and_then(|c| c.window()).into_iter().collect(),
        Some(title) => objc2_app_kit::NSApplication::sharedApplication(mtm)
            .windows()
            .iter()
            .filter(|w| w.isVisible() && w.title().to_string() == title)
            .collect(),
    };
    let found: Vec<super::a11y::audit::Finding> = windows
        .iter()
        .flat_map(|window| super::a11y::audit::findings(&super::a11y::audit::elements(window)))
        .collect();
    check(
        name,
        !windows.is_empty() && found.is_empty(),
        format!(
            "{} windows: {:?}",
            windows.len(),
            found.iter().take(6).collect::<Vec<_>>()
        ),
    );
}

pub fn run() {
    END.start(BUDGET);
    let Some(mtm) = MainThreadMarker::new() else { return };
    helpers::open_trusted(&root());
    let Some(controller) = controller() else {
        check(
            "the repository opens in a window",
            false,
            format!("{:?}", window::open_projects()),
        );
        done();
        return;
    };
    menus(mtm);
    controller.quick_open_with(QUERY);
    poll(WAIT, found_symbol, search_again, symbol_found);
}

/// The Go menu offers every new command, the frequent ones with their shortcut.
fn menus(mtm: MainThreadMarker) {
    let main = objc2_app_kit::NSApplication::sharedApplication(mtm).mainMenu();
    let find = |action: objc2::runtime::Sel| {
        main.as_ref().and_then(|menu| {
            menu.itemArray().iter().find_map(|top| {
                top.submenu()?
                    .itemArray()
                    .iter()
                    .find(|item| item.action() == Some(action))
            })
        })
    };
    let mut seen = vec![];
    for action in [
        objc2::sel!(ideImplementation:),
        objc2::sel!(ideTypeDefinition:),
        objc2::sel!(ideDeclaration:),
        objc2::sel!(ideProjectSymbols:),
        objc2::sel!(ideCallHierarchy:),
        objc2::sel!(ideTypeHierarchy:),
    ] {
        seen.push(find(action).map(|item| (item.title().to_string(), item.keyEquivalent().to_string())));
    }
    let shortcuts = |at: usize| seen[at].as_ref().is_some_and(|(_, key)| !key.is_empty());
    check(
        "the Go menu has the new commands, symbol search and call hierarchy with a shortcut",
        seen.iter().all(Option::is_some) && shortcuts(3) && shortcuts(4),
        format!("{seen:?}"),
    );
}

fn hits() -> Vec<String> {
    controller().map(|c| c.quick_open_hits()).unwrap_or_default()
}

fn found_symbol() -> bool {
    if let Some(said) = controller().and_then(|c| c.quick_open_status()) {
        let mut seen = SAID.lock_ignoring_poison();
        if !seen.contains(&said) {
            seen.push(said);
        }
    }
    hits()
        .iter()
        .any(|h| h.starts_with("ViewState ") && h.ends_with("crates/ide-ui-lsp/src/lib.rs"))
}

/// Types the query again when the server answered with nothing while it was not yet loaded.
fn search_again() {
    if let Some(controller) = controller()
        && controller.quick_open_status().as_deref() == Some(splat_i18n::t("Nichts gefunden"))
    {
        controller.quick_open_with(QUERY);
    }
}

fn symbol_found(ok: bool, took: Duration) {
    let said = SAID.lock_ignoring_poison().clone();
    let held = ide_ui_lsp::symbol_servers(&root());
    if !ok && held.iter().any(|(language, running, _)| language == "Rust" && !running) {
        check(
            "skipped: rust-analyzer missing",
            true,
            format!("{held:?}, said {said:?}"),
        );
        done();
        return;
    }
    check(
        "`#ViewState` in Open Quickly finds the struct in crates/ide-ui-lsp/src/lib.rs",
        ok,
        format!(
            "after {:.1} s: {:?}",
            took.as_secs_f64(),
            hits().iter().take(5).collect::<Vec<_>>()
        ),
    );
    check(
        "while rust-analyzer indexes, the list says so instead of nothing found",
        said.iter().all(|s| s != splat_i18n::t("Nichts gefunden"))
            || said.iter().any(|s| s == splat_i18n::t("Wird indexiert …")),
        format!("{said:?}"),
    );
    let editors = controller().map_or(0, |c| c.tab_count());
    check(
        "the search's lease keeps rust-analyzer running without an editor",
        held.iter()
            .any(|(language, running, editors)| language == "Rust" && *running && *editors == 0),
        format!("{held:?}, {editors} tabs"),
    );
    if !ok {
        done();
        return;
    }
    audit(
        "VoiceOver reaches every symbol Open Quickly lists",
        Some(splat_i18n::t("Schnell öffnen")),
    );
    let row = hits()
        .iter()
        .position(|h| h.starts_with("ViewState ") && h.ends_with("crates/ide-ui-lsp/src/lib.rs"));
    if let (Some(controller), Some(row)) = (controller(), row) {
        controller.quick_open_row(row);
    }
    poll(ANSWER, symbol_opened, || {}, |_, _| opened_symbol());
}

fn selected(view: &EditorTextView) -> String {
    let range = view.selectedRange();
    let text = view.document().text().to_string();
    let utf16: Vec<u16> = text.encode_utf16().collect();
    utf16
        .get(range.location..range.location + range.length)
        .map(String::from_utf16_lossy)
        .unwrap_or_default()
}

fn symbol_opened() -> bool {
    view("crates/ide-ui-lsp/src/lib.rs").is_some_and(|v| selected(&v) == "ViewState")
}

fn opened_symbol() {
    let shown = view("crates/ide-ui-lsp/src/lib.rs").map(|v| selected(&v));
    check(
        "choosing the symbol opens its file with its name selected",
        shown.as_deref() == Some("ViewState"),
        format!("{shown:?}"),
    );
    let Some(controller) = controller() else { return done() };
    controller.open_file(&root().join(TRAIT_FILE));
    poll(ANSWER, implementations_listed, ask_implementations, implementations);
}

/// Asks for the implementations of the trait `Positions`, unless the list of them is open.
fn ask_implementations() {
    let Some(view) = view(TRAIT_FILE) else { return };
    let text = view.document().text().to_string();
    let Some(byte) = text.find("trait Positions") else {
        return;
    };
    let at = text[..byte].encode_utf16().count() + "trait Pos".len();
    view.setSelectedRange(NSRange::new(at, 0));
    if ide_ui_lsp::listed().is_empty() {
        ide_ui_lsp::go_to(&*view, ide_ui_lsp::Goto::Implementation);
    }
}

fn implementations_listed() -> bool {
    ide_ui_lsp::listed().len() >= 10
}

fn implementations(ok: bool, took: Duration) {
    let listed = ide_ui_lsp::listed();
    let in_files = listed
        .iter()
        .all(|line| line.starts_with(TRAIT_FILE) || line.starts_with("crates/ide-lsp/src/navigation.rs"));
    check(
        "the implementations of a trait are listed in the list of places",
        ok && in_files,
        format!(
            "after {:.1} s, {} places: {:?}",
            took.as_secs_f64(),
            listed.len(),
            listed.iter().take(4).collect::<Vec<_>>()
        ),
    );
    picture("lsp-navigation-implementations");
    ide_ui_lsp::close_list();
    let Some(controller) = controller() else { return done() };
    controller.open_file(&root().join(CALLS_FILE));
    poll(
        ANSWER,
        || level(0, "static_name") && level(1, "call"),
        ask_calls,
        first_level,
    );
}

fn ask_calls() {
    let Some(view) = view(CALLS_FILE) else { return };
    let text = view.document().text().to_string();
    let Some(byte) = text.find("fn static_name") else {
        return;
    };
    let at = text[..byte].encode_utf16().count() + "fn stat".len();
    view.setSelectedRange(NSRange::new(at, 0));
    if rows().is_empty() {
        ide_ui_lsp::call_hierarchy(&*view);
    }
}

fn rows() -> Vec<(isize, String)> {
    controller()
        .and_then(|c| c.search_panel())
        .map(|p| search::hierarchy_rows(&p))
        .unwrap_or_default()
}

fn level(depth: isize, name: &str) -> bool {
    rows().iter().any(|(l, n)| *l == depth && n == name)
}

/// Opens the row at `depth` named `name`, as its disclosure triangle does.
fn expand(depth: isize, name: &str) {
    let row = rows().iter().position(|(l, n)| *l == depth && n == name);
    if let (Some(panel), Some(row)) = (controller().and_then(|c| c.search_panel()), row) {
        search::hierarchy_expand(&panel, isize::try_from(row).unwrap_or(0));
    }
}

fn first_level(ok: bool, took: Duration) {
    check(
        "the call hierarchy shows in the search panel with the callers of its root",
        ok,
        format!("after {:.1} s: {:?}", took.as_secs_f64(), rows()),
    );
    if !ok {
        return done();
    }
    expand(1, "call");
    poll(ANSWER, || level(2, "ask"), || {}, second_level);
}

fn second_level(ok: bool, took: Duration) {
    check(
        "opening a caller asks for its callers",
        ok,
        format!("after {:.1} s: {:?}", took.as_secs_f64(), rows()),
    );
    if !ok {
        return done();
    }
    expand(2, "ask");
    poll(ANSWER, || level(3, "hover"), || {}, third_level);
}

fn third_level(ok: bool, took: Duration) {
    let deepest: Vec<String> = rows().into_iter().filter(|(l, _)| *l == 3).map(|(_, n)| n).collect();
    check(
        "the call hierarchy reaches three levels below its root",
        ok,
        format!("after {:.1} s: {deepest:?}", took.as_secs_f64()),
    );
    picture("lsp-navigation-calls");
    audit("VoiceOver reaches every control and row of the hierarchy", None);
    let Some(panel) = controller().and_then(|c| c.search_panel()) else {
        return done();
    };
    let row = rows().iter().position(|(l, n)| *l == 2 && n == "ask");
    if let Some(row) = row {
        search::hierarchy_click(&panel, isize::try_from(row).unwrap_or(0));
    }
    let clicked = view("crates/ide-lsp/src/features.rs").map(|v| selected(&v));
    check(
        "a click on a caller shows where it calls",
        clicked.as_deref() == Some("call"),
        format!("{clicked:?}"),
    );
    history(&panel);
}

/// The menu keeps the text search and the hierarchy, and the hierarchy comes back opened as it was left.
fn history(panel: &crate::window::SearchPanel) {
    let titles = search::hierarchy_history(panel);
    let title = splat_i18n::tf("Aufrufhierarchie von {}", &["static_name"]);
    check(
        "the menu keeps the text search and the hierarchy",
        titles.first().map(String::as_str) == Some(splat_i18n::t("Textsuche")) && titles.contains(&title),
        format!("{titles:?}"),
    );
    let left = rows();
    search::hierarchy_choose(panel, 0);
    check(
        "choosing the text search leaves the hierarchy",
        rows().is_empty(),
        format!("{:?}", rows()),
    );
    search::hierarchy_choose(panel, 1);
    check(
        "the hierarchy comes back opened as it was left",
        left.iter().any(|(l, _)| *l == 3) && rows() == left,
        format!("{} rows, {} before", rows().len(), left.len()),
    );
    poll(
        ANSWER,
        || level(0, "call"),
        ask_callees,
        |ok, took| {
            check(
                "a second hierarchy comes on top of the first",
                ok,
                format!(
                    "after {:.1} s: {:?}",
                    took.as_secs_f64(),
                    rows().iter().take(3).collect::<Vec<_>>()
                ),
            );
            if !ok {
                return done();
            }
            if let Some(panel) = controller().and_then(|c| c.search_panel()) {
                search::hierarchy_direction(&panel, 1);
            }
            poll(ANSWER, || level(1, "static_name"), || {}, callees);
        },
    );
}

/// Asks for the call hierarchy of `call`, which calls `static_name`.
fn ask_callees() {
    let Some(view) = view(CALLS_FILE) else { return };
    let text = view.document().text().to_string();
    let Some(byte) = text.find("pub(crate) fn call(") else {
        return;
    };
    let at = text[..byte].encode_utf16().count() + "pub(crate) fn ca".len();
    view.setSelectedRange(NSRange::new(at, 0));
    if !level(0, "call") {
        ide_ui_lsp::call_hierarchy(&*view);
    }
}

fn callees(ok: bool, took: Duration) {
    check(
        "the switch shows the callees in place of the callers",
        ok && !level(1, "ask"),
        format!("after {:.1} s: {:?}", took.as_secs_f64(), rows()),
    );
    if let Some(panel) = controller().and_then(|c| c.search_panel()) {
        let titles = search::hierarchy_history(&panel);
        check(
            "the menu lists both hierarchies, the newest first",
            titles.get(1) == Some(&splat_i18n::tf("Aufrufhierarchie von {}", &["call"])) && titles.len() == 3,
            format!("{titles:?}"),
        );
    }
    if let Some(view) = view(CALLS_FILE) {
        let types = ide_ui_lsp::validate(&*view, Some(objc2::sel!(ideTypeHierarchy:)));
        let declaration = ide_ui_lsp::validate(&*view, Some(objc2::sel!(ideDeclaration:)));
        check(
            "commands the server does not offer are greyed out, not hidden",
            types.is_some() && declaration.is_some(),
            format!("type hierarchy {types:?}, declaration {declaration:?}"),
        );
    }
    done();
}
