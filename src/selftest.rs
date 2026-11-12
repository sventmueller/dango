//! The app's side of `cargo xtask selftest`: with `IDE_SELFTEST` set it drives its own windows, writes what it saw to
//! `IDE_SELFTEST_REPORT` and quits. See docs/development.md.

mod a11y;
mod acp;
mod agent;
mod agent_background;
mod agent_context;
mod agent_outside;
mod agent_perf;
mod agent_plan;
pub mod application;
mod blame;
mod browser_dev;
mod markdown;
mod markdown_editor;
mod panels;
mod paste_cli;
mod preview;
mod tabs;
mod terminal;

use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSApplication;
use splat_i18n::t;

use crate::util::{main_after, main_soon};
use crate::window::{self, Panel};

mod ci;
mod conflict;
#[cfg(feature = "dictation")]
mod dictation;
mod editor_multi;
mod editor_undo;
mod extras;
mod file_icons;
mod files_header;
mod git;
mod helpers;
mod inlay;
mod insets;
mod lsp;
mod lsp_actions;
mod lsp_navigation;
mod own_agent;
mod recovery;
mod replace;
mod restore;
mod search;
mod search_replace;
mod servers;
mod split;
mod worktree;

struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
}

thread_local! {
    static CHECKS: RefCell<Vec<Check>> = const { RefCell::new(Vec::new()) };
}

/// Opens an AI tab that plays the start of the session `session`, where the app would resume it with `claude`.
pub fn replay_agent(controller: &window::WindowController, session: &str) -> bool {
    panels::replay_agent(controller, session)
}

/// Moves the app's data and its defaults under `IDE_SELFTEST_HOME`, before anything reads them.
pub fn prepare() {
    let Some(home) = std::env::var_os("IDE_SELFTEST_HOME").map(PathBuf::from) else {
        return;
    };
    ide_core::paths::set_home(home.clone());
    if std::env::var_os("IDE_SELFTEST").is_some() && own_defaults(&home).is_none() {
        check(
            "the run keeps its own defaults",
            false,
            "Foundation made no defaults with -[NSUserDefaults _initWithSuiteName:container:]",
        );
    }
}

/// Makes the standard defaults, where AppKit keeps window frames and divider positions, a domain of this run alone in
/// `home/Library/Preferences`: the app's own domain is shared by every selftest app on this Mac, including those other
/// runs start at the same time.
fn own_defaults(home: &Path) -> Option<()> {
    use objc2::{AnyThread, ClassType, msg_send, sel};
    use objc2_foundation::{NSBundle, NSURL, NSUserDefaults};

    let class = NSUserDefaults::class();
    class.instance_method(sel!(_initWithSuiteName:container:))?;
    class.class_method(sel!(setStandardUserDefaults:))?;
    // Foundation refuses a suite named like the app itself.
    let suite = crate::util::ns(&format!("{}.run", NSBundle::mainBundle().bundleIdentifier()?));
    let container = NSURL::fileURLWithPath_isDirectory(&crate::util::ns(&home.to_string_lossy()), true);
    // SAFETY: The private initializer Foundation uses for the defaults of an app's container takes the domain's name
    // and the folder whose `Library/Preferences` keeps it, and returns a defaults object or nil.
    let defaults: Option<Retained<NSUserDefaults>> =
        unsafe { msg_send![NSUserDefaults::alloc(), _initWithSuiteName: &*suite, container: &*container] };
    let defaults = defaults?;
    // SAFETY: Replaces the object `+standardUserDefaults` returns from now on, before AppKit starts and asks for it.
    let () = unsafe { msg_send![class, setStandardUserDefaults: &*defaults] };
    Some(())
}

pub fn after_launch(_mtm: MainThreadMarker) {
    let Ok(scenario) = std::env::var("IDE_SELFTEST") else {
        return;
    };
    main_after(Duration::from_millis(400), move || match scenario.as_str() {
        "probe" => probe(),
        "a11y" | "a11y-rtl" => a11y::run(),
        "acp" => acp::run(),
        "own-agent" => own_agent::run(),
        "agent" => agent::run(),
        "agent-perf" => agent_perf::run(),
        "agent-background" => agent_background::run(),
        "file-icons" => file_icons::run(),
        "agent-live" => agent::live(),
        "browser" => browser(),
        "browser-dev" => browser_dev::run(),
        "editor-extras" => extras::run(),
        "tabs" => tabs::run(),
        "replace-folder" => replace::run(),
        "preview" => preview::run(),
        "markdown" => markdown::run(),
        "panels" => panels::run(),
        "git" => git::run(),
        "git-stash" => git::stash::run(),
        "git-message" => git::message::run(),
        "ci" => ci::run(),
        "ci-live" => ci::live(),
        "ci-login" => ci::login(),
        "ci-settings" => ci::hosts(),
        "worktree" => worktree::run(),
        "lsp" => lsp::run(),
        "lsp-navigation" => lsp_navigation::run(),
        "lsp-actions" => lsp_actions::run_rust(),
        "lsp-actions-go" => lsp_actions::run_go(),
        "language-servers" => servers::run(),
        "editor-multi" => editor_multi::run(),
        "editor-undo" => editor_undo::run(),
        "editor-insets" => insets::run(),
        "inlay-hints" => inlay::run(),
        "editor-blame" => blame::run(),
        "search" => search::run(),
        "terminal" => terminal::run(),
        "paste-cli" => paste_cli::run(),
        "recovery" => recovery::run(),
        "restore" => restore::run(),
        "conflict" => conflict::run(),
        #[cfg(feature = "dictation")]
        "dictation" => dictation::run(),
        "show" => show(),
        other => {
            check("the scenario exists", false, format!("unknown scenario {other}"));
            finish(&scenario);
        }
    });
}

/// Opens a folder with a file and stays open, naming its window in the report, for a real screen capture by the
/// runner or by hand: `screencapture -l <number> picture.png`.
fn show() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = ide_core::paths::home().join("show-project");
    let _ = std::fs::create_dir_all(folder.join("src"));
    let _ = std::fs::write(
        folder.join("src/main.rs"),
        "fn main() {\n    let greeting = \"hello\"; // a comment\n    println!(\"{greeting}\");\n}\n",
    );
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder.clone());
    let canonical = folder.canonicalize().unwrap_or(folder);
    if let Some(controller) = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&canonical))
    {
        if std::env::var_os("IDE_SHOW_FILE").is_some() {
            controller.open_file(&canonical.join("src/main.rs"));
        }
        let number = controller.window().map_or(0, |w| w.windowNumber());
        check("the window shows", number > 0, number.to_string());
    }
    if let Some(path) = std::env::var_os("IDE_SELFTEST_REPORT") {
        let checks = CHECKS.with(|c| {
            c.borrow()
                .iter()
                .map(|c| c.detail.clone())
                .collect::<Vec<_>>()
                .join(",")
        });
        let _ = std::fs::write(path, checks);
    }
}

pub(crate) fn check(name: &'static str, ok: bool, detail: impl Into<String>) {
    CHECKS.with(|checks| {
        checks.borrow_mut().push(Check {
            name,
            ok,
            detail: detail.into(),
        })
    });
}

fn probe() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let windows = window::open_windows();
    check(
        "a window opens",
        !windows.is_empty(),
        format!("{} windows", windows.len()),
    );
    let Some(controller) = windows.first().cloned() else {
        finish("probe");
        return;
    };
    probe_window(&controller);
    probe_app(mtm);
}

/// The sidebar's first symbol and title start level with the tabs' top edge, which sits on its safe area's top.
fn check_sidebar_top(controller: &window::WindowController) {
    let (tabs, safe, sidebar) = controller.sidebar_top();
    check(
        "the sidebar's symbols and title start level with the tabs",
        (tabs - safe).abs() < 0.5 && (sidebar - (safe + window::SIDEBAR_RAISE)).abs() < 0.5,
        format!("tabs at {tabs:.1}, safe area at {safe:.1}, sidebar at {sidebar:.1}"),
    );
}

/// The panels, the sidebar and the tabs of the first window.
fn probe_window(controller: &window::WindowController) {
    let seen = controller.inspect();
    check(
        "the sidebar starts at its default width",
        (seen.sidebar_width - 304.0).abs() < 2.0,
        format!("{} pt", seen.sidebar_width),
    );
    check(
        "the activity bar has five panels and settings",
        seen.activity_buttons == 6,
        format!("{} buttons", seen.activity_buttons),
    );
    check(
        "the files panel shows first",
        seen.panel == Some(Panel::Files) && seen.panel_title == Panel::Files.title(),
        seen.panel_title.clone(),
    );
    check_sidebar_top(controller);
    controller.activate_panel(Panel::Agents);
    let seen = controller.inspect();
    check(
        "another panel replaces it",
        seen.panel == Some(Panel::Agents) && seen.panel_title == Panel::Agents.title(),
        seen.panel_title.clone(),
    );
    controller.activate_panel(Panel::Agents);
    let again = controller.inspect().panel;
    let width = controller.inspect().sidebar_width;
    check(
        "a click on the active symbol keeps its panel open",
        again == Some(Panel::Agents),
        format!("{again:?}"),
    );
    check(
        "the panel keeps its width",
        (width - 304.0).abs() < 2.0,
        format!("{width} pt"),
    );
    check("the welcome tab is open", seen.tabs == 1, format!("{} tabs", seen.tabs));
    probe_symbol_menu(controller);
    let expected = [
        t("Neue Datei"),
        t("Terminal"),
        t("Browser"),
        t("Agenten verwalten …"),
        t("Willkommen"),
    ];
    check(
        "the new tab menu offers every kind",
        expected
            .iter()
            .all(|title| seen.new_tab_items.iter().any(|item| item == title)),
        seen.new_tab_items.join(", "),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "probe-window");
    }
}

/// The ticked items of the context menu of `panel`'s symbol, by title, and the titles of those that are enabled.
fn symbol_menu_state(controller: &window::WindowController, panel: Panel) -> (Vec<String>, Vec<String>) {
    let Some(menu) = controller.activity_menu(panel) else {
        return (vec![], vec![]);
    };
    menu.update();
    let items: Vec<_> = menu.itemArray().iter().collect();
    let titles = |keep: &dyn Fn(&objc2_app_kit::NSMenuItem) -> bool| {
        items
            .iter()
            .filter(|item| keep(item))
            .map(|item| item.title().to_string())
            .collect::<Vec<_>>()
    };
    (
        titles(&|item| item.state() == objc2_app_kit::NSControlStateValueOn),
        titles(&|item| item.isEnabled()),
    )
}

/// The symbols' context menu hides a symbol for good and shows it again; the panel that showed gives way, and the
/// last symbol stays.
fn probe_symbol_menu(controller: &window::WindowController) {
    let titles: Vec<String> = Panel::ALL.iter().map(|p| p.title().to_string()).collect();
    let (ticked, _) = symbol_menu_state(controller, Panel::Git);
    check(
        "a right click on a symbol offers every panel, each ticked",
        ticked == titles,
        ticked.join(", "),
    );
    controller.activate_panel(Panel::Git);
    window::toggle_activity_symbol(Panel::Git);
    let stored = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::HIDDEN_PANELS);
    let after = controller.inspect().panel;
    check(
        "hiding the shown panel's symbol hides it, remembers that and shows the first panel left",
        !controller.shown_symbols().contains(&Panel::Git) && stored == ["git"] && after == Some(Panel::Files),
        format!("{:?}, stored {stored:?}, showing {after:?}", controller.shown_symbols()),
    );
    for panel in [Panel::Files, Panel::Search, Panel::Agents, Panel::Ci] {
        window::toggle_activity_symbol(panel);
    }
    let (ticked, enabled) = symbol_menu_state(controller, Panel::Ci);
    check(
        "the last symbol stays and its item cannot hide it",
        controller.shown_symbols() == [Panel::Ci] && ticked == [Panel::Ci.title()] && !enabled.contains(&ticked[0]),
        format!(
            "{:?}, ticked {ticked:?}, enabled {enabled:?}",
            controller.shown_symbols()
        ),
    );
    if let Some(window) = controller.window() {
        window.displayIfNeeded();
        snapshot(&window, "probe-symbols-hidden");
    }
    for panel in [Panel::Files, Panel::Search, Panel::Agents, Panel::Git] {
        window::toggle_activity_symbol(panel);
    }
    controller.activate_panel(Panel::Agents);
    check(
        "every symbol comes back",
        controller.shown_symbols() == Panel::ALL,
        format!("{:?}", controller.shown_symbols()),
    );
}

/// Settings, texts, opening a folder and the login shell's environment, which ends the run.
fn probe_app(mtm: MainThreadMarker) {
    crate::settings::show(mtm);
    let visible = crate::settings::window().is_some_and(|w| w.isVisible());
    check("the settings window opens", visible, "");
    if let Some(window) = crate::settings::window() {
        snapshot(&window, "probe-settings");
    }
    check(
        "the interface texts load from the bundle",
        !crate::app::localized_strings().is_empty(),
        "",
    );
    let folder = ide_core::paths::home().join("probe-project");
    for (path, text) in [
        ("src/main.rs", "// Grüße, ünïcödé\nfn main() {}\n"),
        ("README.md", "# Probe\n"),
        (".gitignore", "target/\n"),
        ("target/ignored.txt", "x\n"),
    ] {
        let file = folder.join(path);
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&folder));
        let _ = std::fs::write(file, text);
    }
    crate::app::open_project(mtm, folder.clone());
    let canonical = folder.canonicalize().unwrap_or(folder);
    check(
        "an opened folder gets its window",
        window::open_projects().contains(&canonical),
        format!("{:?}", window::open_projects()),
    );
    let Some(controller) = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&canonical))
    else {
        finish("probe");
        return;
    };
    check("an untrusted folder asks for trust", controller.has_sheet(), "");
    clicks_pass_through(mtm);
    let rows = controller.tree_rows();
    check(
        "the tree lists the folder's top without ignored entries",
        rows == 3,
        format!("{rows} rows"),
    );
    window::show_ignored_files(true);
    let shown = controller.tree_rows();
    window::show_ignored_files(false);
    check(
        "the setting for ignored files lists them in the tree and leaves them out again",
        shown == rows + 1 && controller.tree_rows() == rows,
        format!("{shown} rows with them, {} without", controller.tree_rows()),
    );
    let _ = std::fs::write(canonical.join("added.txt"), "new\n");
    controller.quick_open_with("main");
    main_after(Duration::from_secs(2), move || probe_project(canonical, rows));
}

/// Checks that no window of the app, the trust sheet and the settings window among them, catches a click, which would
/// make the app the active one (see `application`).
fn clicks_pass_through(mtm: MainThreadMarker) {
    let windows = NSApplication::sharedApplication(mtm).windows();
    let catching: Vec<String> = windows
        .iter()
        .filter(|w| w.isVisible() && !w.ignoresMouseEvents())
        .map(|w| w.class().name().to_string_lossy().into_owned())
        .collect();
    let visible = windows.iter().filter(|w| w.isVisible()).count();
    check(
        "every window lets clicks through",
        visible > 1 && catching.is_empty(),
        format!("{visible} visible, catching clicks: {}", catching.join(", ")),
    );
}

/// What the folder's window shows a moment later: the watcher's update, quick open's answer and an editor tab.
fn probe_project(root: PathBuf, rows_before: isize) {
    let Some(controller) = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&root))
    else {
        finish("probe");
        return;
    };
    let rows = controller.tree_rows();
    check(
        "a file created outside shows up in the tree",
        rows > rows_before,
        format!("{rows_before} then {rows}"),
    );
    let hits = controller.quick_open_hits();
    check(
        "open quickly finds a file by a few letters",
        hits.iter().any(|hit| hit == "src/main.rs"),
        hits.join(", "),
    );
    let tabs = controller.tab_count();
    controller.open_file(&root.join("src/main.rs"));
    controller.open_file(&root.join("src/main.rs"));
    check(
        "a file opens once in an editor tab",
        controller.tab_count() == tabs + 1,
        format!("{tabs} then {}", controller.tab_count()),
    );
    let windows = controller.tab_windows();
    let front = controller.window();
    let sidebar = controller.sidebar_window();
    check(
        "the file opens in a native tab of its own, which has the sidebar, and the tab bar shows",
        windows.len() == 2
            && windows
                .last()
                .zip(front.as_ref())
                .is_some_and(|(last, front)| std::ptr::eq(&**last, &**front))
            && sidebar
                .zip(front.as_ref())
                .is_some_and(|(sidebar, front)| std::ptr::eq(&*sidebar, &**front))
            && controller.tab_bar_visible()
            && windows
                .first()
                .and_then(|w| w.tabbedWindows())
                .is_some_and(|tabbed| tabbed.count() == 2),
        format!("{:?}", controller.window_tabs()),
    );
    let file = root.join("src/main.rs");
    if let Some(editor) = crate::editor::editor_of(&controller, &file) {
        type_into(&editor, "\nfn later() {");
        crate::editor::type_newline(&editor);
        type_into(&editor, "let x = 1;");
    }
    main_after(Duration::from_millis(500), move || probe_editor(root, file));
}

/// The editor a moment after typing: colours, indentation, the dirty mark and saving.
fn probe_editor(root: PathBuf, file: PathBuf) {
    let Some(controller) = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&root))
    else {
        finish("probe");
        return;
    };
    let Some(editor) = crate::editor::editor_of(&controller, &file) else {
        check("the editor of the opened file is found", false, "");
        finish("probe");
        return;
    };
    let text = editor.document().text().to_string();
    check(
        "a new line after a brace is indented and the brace closed",
        text.contains("fn later() {\n    let x = 1;\n}"),
        text.clone(),
    );
    check(
        "keywords get a colour of their own, also below a line with umlauts",
        crate::editor::keyword_coloured(&editor, &text),
        "",
    );
    let second = text
        .split_inclusive('\n')
        .next()
        .map(|line| line.encode_utf16().count());
    check(
        "a line starts where the view counts it, also below a line with umlauts",
        editor.line_start(1) == second,
        format!("{:?} for {second:?}", editor.line_start(1)),
    );
    check("typing marks the file unsaved", editor.document().is_dirty(), "");
    crate::editor::save_view(&editor);
    let disk = std::fs::read_to_string(&file).unwrap_or_default();
    check(
        "saving writes the text and clears the mark",
        disk == text && !editor.document().is_dirty(),
        disk,
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "probe-project");
    }
    probe_markdown();
}

/// The login shell's environment, the probe's last step.
fn probe_environment() {
    std::thread::spawn(|| {
        let path = crate::environment::wait(Duration::from_secs(15)).and_then(|env| {
            env.iter()
                .find(|(name, _)| name == "PATH")
                .map(|(_, value)| value.clone())
        });
        main_soon(move || {
            check(
                "the login shell's environment arrives",
                path.as_ref().is_some_and(|p| p.contains("/usr/bin")),
                path.unwrap_or_default(),
            );
            finish("probe");
        });
    });
}

const MARKDOWN: &str = "# Notes\n\nA paragraph with **bold** text, `code` and a [link](https://example.org), long \
                        enough to run past the right edge of the window if the editor did not wrap its lines for \
                        prose like this one does.\n\n- [ ] a task with *emphasis*\n\n> A quote\n\n| a | b |\n\
                        |---|---|\n| 1 | 2 |\n\n```rust\nfn main() { let s = \"text\"; }\n```\n";

/// A Markdown file in a trusted folder of its own, opened in splat's source colours.
fn probe_markdown() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = ide_core::paths::home().join("markdown-project");
    let _ = std::fs::create_dir_all(&root);
    let file = root.join("NOTES.md");
    let _ = std::fs::write(&file, MARKDOWN);
    let root = root.canonicalize().unwrap_or(root);
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    main_after(Duration::from_millis(500), move || {
        if let Some(controller) = window::open_windows()
            .into_iter()
            .find(|w| w.project().as_ref() == Some(&root))
        {
            controller.open_file(&root.join("NOTES.md"));
        }
        main_after(Duration::from_millis(600), move || probe_markdown_shown(&root));
    });
}

/// The opened Markdown file: its heading set off from its text, and its long lines wrapped.
fn probe_markdown_shown(root: &std::path::Path) {
    let controller = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_deref() == Some(root));
    let editor = controller
        .as_ref()
        .and_then(|c| crate::editor::editor_of(c, &root.join("NOTES.md")));
    let (Some(controller), Some(editor)) = (controller, editor) else {
        check("a Markdown file opens in an editor", false, "");
        return probe_environment();
    };
    let at = |needle: &str| {
        MARKDOWN
            .find(needle)
            .map(|byte| MARKDOWN[..byte].encode_utf16().count())
    };
    let heading = at("Notes").and_then(|i| crate::editor::styled_at(&editor, i));
    let body = at("paragraph").and_then(|i| crate::editor::styled_at(&editor, i));
    check(
        "a Markdown heading is bold and coloured apart from the text",
        matches!((heading, body), (Some((h, true)), Some((b, false))) if h != b),
        format!("{heading:?} / {body:?}"),
    );
    check("Markdown wraps its long lines", crate::editor::wraps(&editor), "");
    if let Some(window) = controller.window() {
        snapshot(&window, "probe-markdown");
    }
    probe_environment();
}

/// Types `text` into `editor` the way the keyboard does, character by character.
fn type_into(editor: &crate::editor::EditorTextView, text: &str) {
    for c in text.chars() {
        if c == '\n' {
            crate::editor::type_newline(editor);
        } else {
            crate::editor::type_text(editor, &c.to_string());
        }
    }
}

/// The port of the page server of the browser scenario.
static PAGE_PORT: AtomicU16 = AtomicU16::new(0);
/// How many times the page server sent its page.
static PAGES_SENT: AtomicUsize = AtomicUsize::new(0);
const PAGE_TITLE: &str = "Browser Selftest";

/// Serves one page on 127.0.0.1 from a thread, for as long as the app runs.
fn serve_page() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = answer(stream);
        }
    });
    Ok(port)
}

fn answer(mut stream: TcpStream) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") && request.len() < 16 * 1024 {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    let line = String::from_utf8_lossy(&request);
    let path = line.split_whitespace().nth(1).unwrap_or("/");
    let (status, body) = if path == "/favicon.ico" {
        ("404 Not Found", String::new())
    } else {
        PAGES_SENT.fetch_add(1, Ordering::SeqCst);
        let body = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{PAGE_TITLE}</title>\
             <style>body{{font:15px -apple-system;margin:40px;color:#1d1d1f}}\
             h1{{color:#0a66d8}}</style></head><body><h1>{PAGE_TITLE}</h1>\
             <p>Served from 127.0.0.1 for the browser tab.</p>\
             <form><input name=user autocomplete=username> <input name=secret type=password></form>\
             <script>addEventListener('input', () => {{ const f = document.forms[0]; \
             document.title = `{FILLED} ${{f.user.value}} ${{f.secret.value.length}}`; }})</script></body></html>"
        );
        ("200 OK", body)
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Calls `then` with whether `ready` held, as soon as it does or once `deadline` passed, asking every 200 ms.
fn poll(deadline: Instant, ready: fn() -> bool, then: fn(bool)) {
    main_after(Duration::from_millis(200), move || {
        let ok = ready();
        if ok || Instant::now() > deadline {
            then(ok);
        } else {
            poll(deadline, ready, then);
        }
    });
}

fn first_window() -> Option<Retained<window::WindowController>> {
    window::open_windows().first().cloned()
}

fn page_url(path: &str) -> String {
    format!("http://127.0.0.1:{}{path}", PAGE_PORT.load(Ordering::SeqCst))
}

/// A browser tab at a page of a local server: its title, one tab per server, the reload, the native error page.
fn browser() {
    let Some(controller) = first_window() else {
        check("a window opens", false, "");
        finish("browser");
        return;
    };
    match serve_page() {
        Ok(port) => PAGE_PORT.store(port, Ordering::SeqCst),
        Err(error) => {
            check("the page server starts", false, error.to_string());
            finish("browser");
            return;
        }
    }
    let before = controller.tab_count();
    crate::browser::open_tab(&controller, Some(&page_url("/")));
    check(
        "a browser tab opens",
        controller.tab_count() == before + 1,
        format!("{before} then {} tabs", controller.tab_count()),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    poll(
        deadline,
        || first_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == PAGE_TITLE)),
        browser_loaded,
    );
}

fn browser_loaded(ok: bool) {
    let Some(controller) = first_window() else { return };
    check("the tab takes the page's title", ok, controller.tab_titles().join(", "));
    let seen = crate::browser::inspect(&controller);
    check(
        "the Web Inspector is on",
        !seen.is_empty() && seen.iter().all(|seen| seen.inspector),
        format!("{} tabs", seen.len()),
    );
    let tabs = controller.tab_count();
    let sent = PAGES_SENT.load(Ordering::SeqCst);
    crate::browser::open_tab(&controller, Some(&page_url("")));
    check(
        "the same local server opens no second tab",
        controller.tab_count() == tabs,
        format!("{tabs} then {} tabs", controller.tab_count()),
    );
    BEFORE_RELOAD.store(sent, Ordering::SeqCst);
    poll(
        Instant::now() + Duration::from_secs(10),
        || PAGES_SENT.load(Ordering::SeqCst) > BEFORE_RELOAD.load(Ordering::SeqCst),
        browser_reloaded,
    );
}

static BEFORE_RELOAD: AtomicUsize = AtomicUsize::new(0);

fn browser_reloaded(ok: bool) {
    let Some(controller) = first_window() else { return };
    check(
        "it reloads the open tab instead",
        ok,
        format!("{} pages sent", PAGES_SENT.load(Ordering::SeqCst)),
    );
    let Some(window) = controller.window() else { return };
    snapshot(&window, "browser-page");
    press(&window, "l", 37);
    let page = || {
        crate::browser::inspect(&controller)
            .into_iter()
            .find(|seen| seen.url.as_deref() == Some(page_url("/").as_str()))
    };
    check(
        "⌘L focuses the address field",
        page().is_some_and(|seen| seen.editing),
        "",
    );
    if let Some(editor) = window
        .firstResponder()
        .and_then(|responder| responder.downcast::<objc2_app_kit::NSTextView>().ok())
    {
        editor.setString(&crate::util::ns("typed over"));
        // SAFETY: The field editor handles the command as Esc does, through the field's delegate.
        unsafe {
            objc2_app_kit::NSStandardKeyBindingResponding::doCommandBySelector(&*editor, objc2::sel!(cancelOperation:))
        };
    }
    let address = page().map(|seen| seen.address).unwrap_or_default();
    check(
        "Esc in the field restores the address",
        address == page_url("/"),
        address,
    );
    BEFORE_RELOAD.store(PAGES_SENT.load(Ordering::SeqCst), Ordering::SeqCst);
    press(&window, "r", 15);
    poll(
        Instant::now() + Duration::from_secs(10),
        || PAGES_SENT.load(Ordering::SeqCst) > BEFORE_RELOAD.load(Ordering::SeqCst),
        browser_shortcut_reloaded,
    );
}

/// Sends ⌘ and `key`, whose virtual key code is `code`, to `window` as a key equivalent.
fn press(window: &objc2_app_kit::NSWindow, key: &str, code: u16) {
    let event = objc2_app_kit::NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        objc2_app_kit::NSEventType::KeyDown,
        objc2_foundation::NSPoint::new(0.0, 0.0),
        objc2_app_kit::NSEventModifierFlags::Command,
        0.0,
        window.windowNumber(),
        None,
        &crate::util::ns(key),
        &crate::util::ns(key),
        false,
        code,
    );
    if let Some(event) = event {
        window.performKeyEquivalent(&event);
    }
}

fn browser_shortcut_reloaded(ok: bool) {
    let Some(controller) = first_window() else { return };
    check(
        "⌘R reloads the page",
        ok,
        format!("{} pages sent", PAGES_SENT.load(Ordering::SeqCst)),
    );
    check(
        "the menu bar offers reloading for the browser tab in front",
        crate::browser::validate(&controller, objc2::sel!(ideBrowserReload:)),
        "",
    );
    crate::browser::fill_login(&controller);
    check(
        "the key button asks for a login",
        crate::browser::answer_asking(&["selftest", "secret"]),
        "",
    );
    poll(
        Instant::now() + Duration::from_secs(10),
        || first_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == &format!("{FILLED} selftest 6"))),
        browser_filled,
    );
}

/// The title the self-test page takes once its login form was filled in.
const FILLED: &str = "Filled";

fn browser_filled(ok: bool) {
    let Some(controller) = first_window() else { return };
    check(
        "the login lands in the page's form as typing would",
        ok,
        controller.tab_titles().join(", "),
    );
    let closed = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .unwrap_or(9);
    crate::browser::open_tab(&controller, Some(&format!("http://127.0.0.1:{closed}/")));
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| crate::browser::inspect(&c).iter().any(|seen| seen.failure.is_some())),
        browser_failed,
    );
}

fn browser_failed(ok: bool) {
    let Some(controller) = first_window() else { return };
    let seen = crate::browser::inspect(&controller);
    let failure = seen.iter().find_map(|seen| seen.failure.clone()).unwrap_or_default();
    check(
        "a server that is not running shows the native error page",
        ok && failure.starts_with(&splat_i18n::tf("Keine Verbindung zu {}", &["127.0.0.1"])),
        failure,
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-error");
    }
    crate::browser::open_tab(&controller, None);
    let seen = crate::browser::inspect(&controller);
    let empty = seen.last();
    check(
        "an empty tab invites an address in its focused field",
        empty.is_some_and(|seen| seen.failure.as_deref() == Some(t("Browser")) && seen.url.is_none() && seen.editing),
        empty.and_then(|seen| seen.failure.clone()).unwrap_or_default(),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-empty");
    }
    crate::browser::open_tab(&controller, Some("file:///etc/hosts"));
    let refused = crate::browser::inspect(&controller)
        .last()
        .and_then(|seen| seen.failure.clone())
        .unwrap_or_default();
    check(
        "a local file outside the window's folder stays closed",
        refused == t("Datei außerhalb des Projekts"),
        refused,
    );
    browser_project_file();
}

const FILE_TITLE: &str = "Project File";

/// A page of the window's folder opens from its path, with the stylesheet beside it.
fn browser_project_file() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = ide_core::paths::home().join("browser-project");
    let page = folder.join("site/index.html");
    let _ = std::fs::create_dir_all(folder.join("site"));
    let _ = std::fs::write(
        &page,
        format!("<!doctype html><title>{FILE_TITLE}</title><link rel=stylesheet href=style.css><h1>{FILE_TITLE}</h1>"),
    );
    let _ = std::fs::write(
        folder.join("site/style.css"),
        "h1 { color: #b0451f; font: 600 28px -apple-system }",
    );
    crate::app::open_project(mtm, folder.clone());
    let root = folder.canonicalize().unwrap_or(folder);
    let Some(controller) = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&root))
    else {
        check("the folder gets its window", false, "");
        finish("browser");
        return;
    };
    crate::browser::open_tab(&controller, Some(&root.join("site/index.html").to_string_lossy()));
    poll(
        Instant::now() + Duration::from_secs(20),
        || project_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == FILE_TITLE)),
        browser_project_file_loaded,
    );
}

fn project_window() -> Option<Retained<window::WindowController>> {
    window::open_windows().into_iter().find(|w| w.project().is_some())
}

fn browser_project_file_loaded(ok: bool) {
    let Some(controller) = project_window() else { return };
    check(
        "a page of the window's folder opens",
        ok,
        controller.tab_titles().join(", "),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-file");
    }
    finish("browser");
}

/// Saves a picture of `window` as `<report folder>/<name>.png`: a real capture of just this window by
/// `screencapture`, else AppKit's own drawing, which leaves out layer content such as TextKit 2 text and glass.
pub(crate) fn snapshot(window: &objc2_app_kit::NSWindow, name: &str) {
    let Some(path) = picture_path(name) else {
        return;
    };
    window.displayIfNeeded();
    flush_to_screen();
    if !capture(&["-o", "-l", &window.windowNumber().to_string()], &path) {
        draw_window(window, &path);
    }
}

/// Saves AppKit's own drawing of `window` as `<report folder>/<name>.png`, without its glass and layer content.
pub(crate) fn drawn_snapshot(window: &objc2_app_kit::NSWindow, name: &str) {
    if let Some(path) = picture_path(name) {
        draw_window(window, &path);
    }
}

/// Where the picture `name` goes, beside the report; none when the run takes no pictures, with
/// `IDE_SELFTEST_NO_PICTURES` set.
pub(crate) fn picture_path(name: &str) -> Option<PathBuf> {
    if std::env::var_os("IDE_SELFTEST_NO_PICTURES").is_some() {
        return None;
    }
    let report = std::env::var_os("IDE_SELFTEST_REPORT")?;
    Some(PathBuf::from(report).with_file_name(format!("{name}.png")))
}

/// How long `screencapture` may take before the picture counts as failed.
const CAPTURE_LIMIT: Duration = Duration::from_secs(15);

/// Has `screencapture` save what `args` select to `path`, as the only selftest run on this Mac doing so: captures of
/// several runs at once queue up in the system's capture service, up to seconds each. False if it failed or took too
/// long.
pub(crate) fn capture(args: &[&str], path: &Path) -> bool {
    let _turn = capture_turn();
    let Ok(mut child) = std::process::Command::new("/usr/sbin/screencapture")
        .arg("-x")
        .args(args)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success() && path.exists(),
            Ok(None) if started.elapsed() < CAPTURE_LIMIT => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// The lock in the user's temporary folder that a run holds while it captures; without it, the run captures at once.
fn capture_turn() -> Option<std::fs::File> {
    let lock = std::env::temp_dir().join(format!("{}.selftest-capture.lock", ide_product::BUNDLE_ID));
    let file = std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock)
        .ok()?;
    file.lock().ok()?;
    Some(file)
}

/// Hands pending drawing to the window server now: `screencapture` reads what it shows, which otherwise lags until the
/// run loop commits Core Animation's transaction.
fn flush_to_screen() {
    if let Some(class) = objc2::runtime::AnyClass::get(c"CATransaction") {
        // SAFETY: `+[CATransaction flush]` takes no arguments and returns nothing.
        let _: () = unsafe { objc2::msg_send![class, flush] };
    }
    std::thread::sleep(Duration::from_millis(50));
}

fn draw_window(window: &objc2_app_kit::NSWindow, path: &std::path::Path) {
    let Some(content) = window.contentView() else { return };
    // SAFETY: The window's frame view, which draws the title bar and toolbar, outlives this call like the window.
    let view = unsafe { content.superview() }.unwrap_or(content);
    let bounds = view.bounds();
    let Some(bitmap) = view.bitmapImageRepForCachingDisplayInRect(bounds) else {
        return;
    };
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
    // SAFETY: An empty properties dictionary is valid for PNG.
    let data = unsafe {
        bitmap.representationUsingType_properties(
            objc2_app_kit::NSBitmapImageFileType::PNG,
            &objc2_foundation::NSDictionary::new(),
        )
    };
    if let Some(data) = data {
        let _ = std::fs::write(path, data.to_vec());
    }
}

pub(crate) fn finish(scenario: &str) {
    write_report(scenario);
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    for window in app.windows().iter() {
        if let Some(sheet) = window.attachedSheet() {
            window.endSheet(&sheet);
        }
    }
    // A quit that hangs fails the run with its own exit status instead of the runner's two-minute timeout.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(10));
        crate::diag::report("selftest", "the app did not quit within 10 s");
        std::process::exit(3);
    });
    // From a run loop timer, not this dispatch block: while a block of the main queue runs, the queue cannot run the
    // block with `replyToApplicationShouldTerminate:` that quitting waits for when processes are still running.
    let object: &objc2::runtime::NSObject = &app;
    // SAFETY: `terminate:` takes the sender, here none; the run loop retains the application until the call.
    unsafe {
        objc2_foundation::NSObjectNSDelayedPerforming::performSelector_withObject_afterDelay(
            object,
            objc2::sel!(terminate:),
            None,
            0.0,
        );
    }
}

/// Writes the checks so far to `IDE_SELFTEST_REPORT`, whole or not at all.
fn write_report(scenario: &str) {
    let checks = CHECKS.with(|checks| {
        checks
            .borrow()
            .iter()
            .map(|c| {
                format!(
                    r#"{{"name": {}, "ok": {}, "detail": {}}}"#,
                    quote(c.name),
                    c.ok,
                    quote(&c.detail)
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    });
    let report = format!(r#"{{"scenario": {}, "checks": [{checks}]}}"#, quote(scenario));
    if let Some(path) = std::env::var_os("IDE_SELFTEST_REPORT") {
        let path = PathBuf::from(path);
        let partial = path.with_extension("partial");
        if let Err(error) = std::fs::write(&partial, report).and_then(|()| std::fs::rename(&partial, &path)) {
            crate::diag::report("selftest report", error);
        }
    }
}

/// `s` as a JSON string.
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
