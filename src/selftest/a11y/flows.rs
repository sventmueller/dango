//! The main tasks with the keyboard alone, through key presses to the window: staging and committing from the Git
//! panel, Open Quickly, a new tab with ⌘T, the next panel, a prompt to an agent (the test agent of `ide_acp`),
//! comments on a plan (`plan`), and opening a folder with ⌘O and from the welcome tab. On the way, the sheet the
//! secrets check shows before a commit.

use std::cell::Cell;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use block2::RcBlock;
use ide_process::Supervisor;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSSavePanel, NSView, NSWindow};
use objc2_foundation::{NSRunLoop, NSRunLoopCommonModes, NSTimer};

use super::super::git::sheet_ready;
use super::super::helpers::wait_until;
use super::{end, keys, walk, window_of};
use crate::git_panel::selftest as git;
use crate::selftest::check;
use crate::util::main_after;
use crate::window::{Panel, WindowController};
use splat_i18n::t;

pub const CREDENTIALS: &str = "credentials.txt";
/// An access key in the shape gitleaks' `aws-access-token` rule looks for; it opens nothing.
pub const FAKE_KEY: &str = "aws_access_key_id = AKIAQYLPMN5HHHFPZAM2\n";
/// A folder the welcome tab lists among the recent ones.
pub const RECENT: &str = "a11y-recent";
const MESSAGE: &str = "Commit by keyboard";
const REGISTRY: &str = include_str!("../../../crates/ide-acp/tests/fixtures/registry.json");
/// The place of Gemini in the "+" menu, played by the test agent.
const GEMINI: usize = 1;
const PROMPT: &str = "hello";

const NONE: NSEventModifierFlags = NSEventModifierFlags::empty();
const COMMAND: NSEventModifierFlags = NSEventModifierFlags::Command;
const RETURN: (&str, u16) = ("\r", 36);
const ESCAPE: (&str, u16) = ("\u{1b}", 53);
const TAB: (&str, u16) = ("\t", 48);
const SPACE: (&str, u16) = (" ", 49);
const DOWN: (&str, u16) = ("\u{f701}", 125);

/// The panel window in front of `controller`'s window, such as Open Quickly.
pub fn child_panel(controller: &WindowController) -> Option<Retained<NSWindow>> {
    let window = controller.window()?;
    let children = window.childWindows()?;
    children.iter().find(|child| child.isVisible())
}

fn class_of_focus(window: &NSWindow) -> String {
    keys::focused(window).map_or_else(|| "nothing".to_string(), |view| class_name(&view))
}

/// The class of `view`, without the subclass key-value observing puts in its place.
fn class_name(view: &NSView) -> String {
    let name = view.class().name().to_string_lossy().into_owned();
    name.strip_prefix("NSKVONotifying_")
        .map_or(name.clone(), str::to_string)
}

/// Presses Tab in `window` until the view with the focus is of `class`, at most `limit` times; whether it got there,
/// and the views the focus went through.
fn tab_to(window: &NSWindow, class: &str, limit: usize) -> (bool, String) {
    let mut seen = vec![class_of_focus(window)];
    for _ in 0..limit {
        if seen.last().is_some_and(|last| last == class) {
            break;
        }
        let _ = keys::press(window, TAB.0, TAB.1, NONE);
        let now = class_of_focus(window);
        if seen.last() != Some(&now) {
            seen.push(now);
        }
    }
    (seen.last().is_some_and(|last| last == class), seen.join(" → "))
}

/// ⌃Tab, or ⌃⇧Tab with `back`, in `window`, whose focus is in a text view of `class` that types Tab: whether the
/// focus went to another view without a tab typed, and where it went.
fn control_tab_leaves(window: &NSWindow, class: &str, back: bool) -> (bool, String) {
    let text = keys::focused(window)
        .filter(|view| class_name(view) == class)
        .and_then(|view| view.downcast::<objc2_app_kit::NSTextView>().ok());
    let Some(text) = text else {
        return (
            false,
            format!("the focus is on {}, not {class}", class_of_focus(window)),
        );
    };
    let before = text.string().to_string();
    let modifiers = if back {
        NSEventModifierFlags::Control | NSEventModifierFlags::Shift
    } else {
        NSEventModifierFlags::Control
    };
    let went = keys::press(window, TAB.0, TAB.1, modifiers);
    let typed = text.string().to_string() != before;
    let arrived = class_of_focus(window);
    (
        arrived != class && arrived != "nothing" && !typed,
        format!("to {went}, from {class} to {arrived}, tab typed {typed}"),
    )
}

/// ⌘4 shows the Git panel, Tab reaches its list, Space stages the row Down selected, Tab reaches the message, and ⌘↩
/// commits what was typed there; `then` goes on.
pub fn commit(root: PathBuf, then: fn(PathBuf)) {
    let Some(controller) = window_of(&root) else { return };
    let Some(window) = controller.window() else { return };
    controller.activate_panel(Panel::Files);
    let went = keys::press(&window, "4", 21, COMMAND);
    check(
        "⌘4 shows the Git panel with the focus in it",
        controller.inspect().panel == Some(Panel::Git) && class_of_focus(&window).starts_with("IDEGit"),
        format!("to {went}, focus on {}", class_of_focus(&window)),
    );
    let (reached, path) = tab_to(&window, "IDEGitOutlineView", 40);
    check("Tab reaches the list of changes", reached, path);
    // The first row is the section of changes.
    let _ = keys::press(&window, DOWN.0, DOWN.1, NONE);
    let _ = keys::press(&window, SPACE.0, SPACE.1, NONE);
    let changes = git::seen(&controller).map_or(0, |s| s.changes.len());
    let probe = root.clone();
    wait_until(
        move || {
            window_of(&probe)
                .and_then(|c| git::seen(&c))
                .is_some_and(|s| changes > 0 && s.staged.len() == changes)
        },
        move |ok| {
            let staged = window_of(&root)
                .and_then(|c| git::seen(&c))
                .map_or(0, |s| s.staged.len());
            check(
                "Space on the section of changes stages all its files",
                ok,
                format!("{staged} of {changes} staged"),
            );
            if let Some(controller) = window_of(&root) {
                git::unstage_all(&controller);
            }
            let probe = root.clone();
            wait_until(
                move || {
                    window_of(&probe)
                        .and_then(|c| git::seen(&c))
                        .is_some_and(|s| s.staged.is_empty())
                },
                move |_| stage_one(root, then),
            );
        },
    );
}

/// Down to the first file below the section of changes, and Space stages it alone.
fn stage_one(root: PathBuf, then: fn(PathBuf)) {
    let Some(window) = window_of(&root).and_then(|c| c.window()) else {
        return;
    };
    let selected = || {
        keys::focused(&window)
            .and_then(|view| view.downcast::<objc2_app_kit::NSTableView>().ok())
            .map_or(-1, |table| table.selectedRow())
    };
    for _ in 0..3 {
        if selected() >= 1 {
            break;
        }
        let _ = keys::press(&window, DOWN.0, DOWN.1, NONE);
    }
    let row = selected();
    let _ = keys::press(&window, SPACE.0, SPACE.1, NONE);
    let probe = root.clone();
    wait_until(
        move || {
            window_of(&probe)
                .and_then(|c| git::seen(&c))
                .is_some_and(|s| !s.staged.is_empty())
        },
        move |ok| {
            let staged = window_of(&root)
                .and_then(|c| git::seen(&c))
                .map_or(0, |s| s.staged.len());
            check(
                "Space stages the selected file alone",
                ok && staged == 1,
                format!("row {row} selected, {staged} staged"),
            );
            type_message(root, then);
        },
    );
}

fn type_message(root: PathBuf, then: fn(PathBuf)) {
    let Some(controller) = window_of(&root) else { return };
    let Some(window) = controller.window() else { return };
    let (reached, path) = tab_to(&window, "IDEGitCommitTextView", 20);
    check("Tab reaches the commit message", reached, path);
    keys::type_keys(&window, MESSAGE);
    let _ = keys::press(&window, RETURN.0, RETURN.1, COMMAND);
    let probe = root.clone();
    wait_until(
        move || {
            window_of(&probe)
                .and_then(|c| git::seen(&c))
                .is_some_and(|s| !s.busy && s.top.is_some_and(|(subject, _)| subject == MESSAGE))
        },
        move |ok| {
            let top = window_of(&root).and_then(|c| git::seen(&c)).and_then(|s| s.top);
            check("⌘↩ commits the typed message", ok, format!("{top:?}"));
            secrets(root, then);
        },
    );
}

/// A staged access key: the secrets check stops the commit with a sheet, if gitleaks is installed, and Esc cancels.
fn secrets(root: PathBuf, then: fn(PathBuf)) {
    if let Err(error) = std::fs::write(root.join(CREDENTIALS), FAKE_KEY) {
        check("the file with the access key is written", false, error.to_string());
        return then(root);
    }
    let probe = root.clone();
    let file = root.join(CREDENTIALS);
    wait_until(
        move || {
            window_of(&probe)
                .and_then(|c| git::seen(&c))
                .is_some_and(|s| s.changes.contains(&file))
        },
        move |ok| {
            let Some(controller) = window_of(&root) else { return };
            if !ok || !gitleaks_installed() {
                check(
                    "the secrets sheet shows before a commit",
                    true,
                    "gitleaks is not installed",
                );
                return then(root);
            }
            git::stage(&controller, &root.join(CREDENTIALS));
            let probe = root.clone();
            let file = root.join(CREDENTIALS);
            wait_until(
                move || {
                    window_of(&probe)
                        .and_then(|c| git::seen(&c))
                        .is_some_and(|s| s.staged.contains(&file))
                },
                move |_| {
                    if let Some(controller) = window_of(&root) {
                        git::commit(&controller, "Add credentials");
                    }
                    let probe = root.clone();
                    wait_until(
                        move || window_of(&probe).is_some_and(|c| sheet_ready(&c)),
                        move |ok| secrets_sheet(root, ok, then),
                    );
                },
            );
        },
    );
}

fn secrets_sheet(root: PathBuf, ok: bool, then: fn(PathBuf)) {
    check("the secrets check stops the commit with a sheet", ok, "");
    walk("the secrets sheet misses nothing");
    let Some(controller) = window_of(&root) else { return };
    if let Some(sheet) = controller.window().and_then(|w| w.attachedSheet()) {
        let _ = keys::press(&sheet, ESCAPE.0, ESCAPE.1, NONE);
    }
    let probe = root.clone();
    wait_until(
        move || window_of(&probe).is_some_and(|c| !c.has_sheet()),
        move |ok| {
            check("Esc cancels the secrets sheet", ok, "");
            then(root);
        },
    );
}

fn gitleaks_installed() -> bool {
    crate::environment::get()
        .and_then(|env| {
            env.iter()
                .find(|(name, _)| name == "PATH")
                .map(|(_, path)| path.clone())
        })
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("gitleaks").is_file()))
}

/// The flows after the states: Open Quickly, a new tab, the next panel, a prompt, a plan, opening a folder.
pub fn start(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let Some(window) = controller.window() else { return };
    let went = keys::press(&window, "p", 35, COMMAND);
    let Some(panel) = child_panel(&controller) else {
        check("⌘P opens Open Quickly", false, format!("to {went}"));
        return new_tab(root);
    };
    keys::type_keys(&panel, "readme");
    let probe = root.clone();
    wait_until(
        move || window_of(&probe).is_some_and(|c| c.quick_open_hits().first().is_some_and(|h| h == "README.md")),
        move |ok| {
            check("typing into Open Quickly finds the file", ok, "");
            let Some(controller) = window_of(&root) else { return };
            if let Some(panel) = child_panel(&controller) {
                let _ = keys::press(&panel, RETURN.0, RETURN.1, NONE);
            }
            let titles = controller.tab_titles();
            let selected = controller.selected_tab();
            let opened = controller.tab_with_key(&root.join("README.md").to_string_lossy());
            let focus = controller.window().map(|w| class_of_focus(&w)).unwrap_or_default();
            check(
                "Return opens the file with the focus in its editor",
                opened.is_some() && opened == selected && focus == "IDEEditorTextView",
                format!("{} with focus on {focus}", titles.join(", ")),
            );
            if let Some(window) = controller.window() {
                let (left, detail) = control_tab_leaves(&window, "IDEEditorTextView", false);
                check("⌃Tab leaves the editor without typing a tab", left, detail);
            }
            new_tab(root);
        },
    );
}

/// ⌘T opens the page of a new tab with the focus on its first entry; Tab reaches the terminal, and Space opens it in
/// the page's place with the focus in it.
fn new_tab(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let Some(window) = controller.window() else { return };
    let before = controller.tab_count();
    let went = keys::press(&window, "t", 17, COMMAND);
    let entries = controller.new_tab_page_entries().unwrap_or_default();
    let Some(page_window) = controller.window() else { return };
    let first = class_of_focus(&page_window);
    check(
        "⌘T opens the page of a new tab with the focus on its first entry",
        controller.tab_count() == before + 1
            && entries.first().is_some_and(|e| e == t("Neue Datei"))
            && first == "NSButton",
        format!("to {went}, {entries:?}, focus on {first}"),
    );
    let terminal = t("Terminal").to_string();
    let mut reached = false;
    for _ in 0..60 {
        let title = keys::focused(&page_window)
            .and_then(|v| v.downcast::<objc2_app_kit::NSButton>().ok())
            .map(|b| b.title().to_string());
        if title.as_deref() == Some(terminal.as_str()) {
            reached = true;
            break;
        }
        let _ = keys::press(&page_window, TAB.0, TAB.1, NONE);
    }
    if !reached && let Some(button) = button_titled(&page_window, &terminal) {
        // AppKit lets Tab reach buttons only with the Mac's full keyboard access, which a selftest cannot switch on.
        let loop_views = keys::key_view_loop(&page_window, &[]);
        reached = keys::linked(&button, &loop_views) && page_window.makeFirstResponder(Some(&button));
    }
    if reached {
        let _ = keys::press(&page_window, SPACE.0, SPACE.1, NONE);
    }
    let focus = controller.window().map(|w| class_of_focus(&w)).unwrap_or_default();
    check(
        "Tab reaches the terminal on the page, and Space opens it in the page's place with the focus",
        reached && controller.tab_count() == before + 1 && focus == "IDETerminalView",
        format!("{} then {} documents, focus on {focus}", before, controller.tab_count()),
    );
    next_panel(root);
}

/// ⌥⌘` moves the focus to the other document of a tab that shows two.
fn next_panel(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Some(notes) = controller.tab_with_key(&root.join("notes.txt").to_string_lossy()) {
        controller.select_tab_by_id(notes);
    }
    let Some(window) = controller.window() else { return };
    let before = controller.selected_tab();
    // The menu shows the shortcut in the keyboard layout's own key, as ⌥⌘< on a German one.
    let (key, modifiers) = keys::shortcut_of(&window, objc2::sel!(ideFocusNextPanel:))
        .unwrap_or(("`".into(), NSEventModifierFlags::Option | COMMAND));
    let went = keys::press(&window, &key, 0, modifiers);
    let after = controller.selected_tab();
    let focus = class_of_focus(&window);
    check(
        "⌥⌘` moves the focus to the other document of the tab",
        before.is_some() && after.is_some() && before != after && focus != "nothing",
        format!("{key:?} to {went}, {before:?} then {after:?}, focus on {focus}"),
    );
    prompt(root);
}

/// The test agent of `ide_acp` started for this folder with its scenario `basic`.
fn test_launch(root: &std::path::Path) -> Option<ide_acp::Launch> {
    Some(ide_acp::Launch::new(
        "test",
        "Test",
        std::env::current_exe().ok()?,
        Vec::new(),
        vec![
            ("IDE_ACP_TEST_AGENT".into(), "basic".into()),
            ("PATH".into(), "/usr/bin:/bin".into()),
        ],
        root,
    ))
}

/// ⌘Esc puts the focus in the composer of the AI tab, where the prompt is typed and Return sends it.
fn prompt(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    if let Ok(register) = ide_acp::Register::parse(REGISTRY) {
        crate::agent::set_test_register(register);
    }
    crate::agent::set_test_launch(test_launch(&root));
    let Some(tab) = crate::agent::open_preset(&controller, GEMINI) else {
        check("an AI tab with the test agent opens", false, "");
        return open_folder(root);
    };
    let key = tab.key;
    let live = move || crate::agent::all().into_iter().any(|t| t.key == key && t.is_live());
    wait_until(live, move |ok| {
        check("the test agent's session opens", ok, "");
        let Some(controller) = window_of(&root) else { return };
        let Some(window) = controller.window() else { return };
        let went = keys::press(&window, ESCAPE.0, ESCAPE.1, COMMAND);
        let focus = class_of_focus(&window);
        check(
            "⌘Esc puts the focus in the composer",
            focus == "IDEAgentComposerText",
            format!("to {went}, focus on {focus}"),
        );
        keys::type_keys(&window, PROMPT);
        let _ = keys::press(&window, RETURN.0, RETURN.1, NONE);
        let sent = move || {
            crate::agent::all().into_iter().any(|t| {
                t.key == key
                    && t.snapshot()
                        .entries(&ide_thread::Lane::Main)
                        .iter()
                        .any(|e| matches!(&e.kind, ide_thread::EntryKind::UserMessage(m) if m.text.trim() == PROMPT))
            })
        };
        wait_until(sent, move |ok| {
            check("Return sends the typed prompt", ok, "");
            if let Some(window) = window_of(&root).and_then(|c| c.window()) {
                let composer = "IDEAgentComposerText";
                let (back, detail) = control_tab_leaves(&window, composer, true);
                let _ = keys::press(&window, TAB.0, TAB.1, NONE);
                let returned = class_of_focus(&window) == composer;
                check(
                    "⌃⇧Tab leaves the message field for the view before it, from where Tab comes back",
                    back && returned,
                    format!("{detail}; Tab comes back {returned}"),
                );
                let (on, detail) = control_tab_leaves(&window, composer, false);
                check("⌃Tab leaves the message field for the view after it", on, detail);
            }
            super::plan::start(root, open_folder);
        });
    });
}

/// ⌘O opens the panel to choose a folder; a new window's welcome tab opens a recent folder with Tab and Space.
fn open_folder(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let Some(window) = controller.window() else { return };
    let seen = std::rc::Rc::new(Cell::new(false));
    let saw = seen.clone();
    let tries = Cell::new(0);
    let block = RcBlock::new(move |timer: NonNull<NSTimer>| {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let app = NSApplication::sharedApplication(mtm);
        tries.set(tries.get() + 1);
        let modal = app.modalWindow();
        if let Some(window) = &modal {
            saw.set(true);
            match window.clone().downcast::<NSSavePanel>() {
                // SAFETY: Cancelling takes any sender; none is given.
                Ok(panel) => unsafe { panel.cancel(None) },
                Err(_) => app.abortModal(),
            }
        }
        if modal.is_some() || tries.get() > 40 {
            // SAFETY: The timer calling this block is alive while it runs.
            unsafe { timer.as_ref() }.invalidate();
        }
    });
    // SAFETY: The block holds a flag of the main thread, whose run loop calls it.
    let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(0.25, true, &block) };
    // SAFETY: The main run loop takes the timer in its common modes, which include the one modal panels run in.
    unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
    let went = keys::press(&window, "o", 31, COMMAND);
    timer.invalidate();
    check(
        "⌘O opens the panel to choose a folder",
        seen.get(),
        format!("to {went}"),
    );
    let recent = ide_core::paths::home().join(RECENT);
    let _ = ide_core::settings::State::load_user().note_recent(&recent);
    let before = crate::window::open_windows();
    let Some(any) = before.first() else { return };
    let Some(any_window) = any.window() else { return };
    let went = keys::press(&any_window, "n", 45, NSEventModifierFlags::Shift | COMMAND);
    let opened = crate::window::open_windows()
        .into_iter()
        .find(|w| !before.iter().any(|b| std::ptr::eq(&**b, &**w)));
    let Some(window) = opened.and_then(|c| c.window()) else {
        check("⇧⌘N opens a new window", false, format!("to {went}"));
        return wind_down(Instant::now());
    };
    welcome(root, &window, recent);
}

fn welcome(root: PathBuf, window: &NSWindow, recent: PathBuf) {
    let wanted = crate::app::display_path(&recent);
    let mut reached = false;
    let mut seen = Vec::new();
    for _ in 0..30 {
        let title = keys::focused(window)
            .and_then(|v| v.downcast::<objc2_app_kit::NSButton>().ok())
            .map(|b| b.title().to_string());
        if title.as_deref() == Some(wanted.as_str()) {
            reached = true;
            break;
        }
        seen.push(title.unwrap_or_else(|| class_of_focus(window)));
        let _ = keys::press(window, TAB.0, TAB.1, NONE);
    }
    seen.dedup();
    let mut path = seen.join(" → ");
    if !reached && let Some(button) = button_titled(window, &wanted) {
        // AppKit lets Tab reach buttons only with the Mac's full keyboard access, which a selftest cannot switch on.
        let loop_views = keys::key_view_loop(window, &[]);
        reached = keys::linked(&button, &loop_views) && window.makeFirstResponder(Some(&button));
        path = format!("full keyboard access is off on this Mac; the key view loop links it: {reached}");
    }
    check("Tab reaches a recent folder on the welcome tab", reached, path);
    if reached {
        let _ = keys::press(window, SPACE.0, SPACE.1, NONE);
    }
    let probe = recent.clone();
    wait_until(
        move || super::super::helpers::window_of(&probe).is_some(),
        move |ok| {
            check("Space opens the recent folder", ok, "");
            close_all(root);
        },
    );
}

/// The button of `window` titled `title`.
fn button_titled(window: &NSWindow, title: &str) -> Option<Retained<NSView>> {
    fn find(view: &NSView, title: &str) -> Option<Retained<NSView>> {
        if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>()
            && button.title().to_string() == title
        {
            return Some(view.retain());
        }
        view.subviews().iter().find_map(|sub| find(&sub, title))
    }
    let content = window.contentView()?;
    find(&content, title)
}

/// Closes the CLI thread with Ctrl+C, then every window, and ends once their processes did.
fn close_all(root: PathBuf) {
    if let Some(controller) = window_of(&root) {
        for id in controller.tab_ids() {
            controller.close_tab(id);
        }
    }
    main_after(Duration::from_millis(500), || {
        for controller in crate::window::open_windows() {
            if let Some(window) = controller.window() {
                window.performClose(None);
            }
        }
        wind_down(Instant::now());
    });
}

fn wind_down(since: Instant) {
    let running = Supervisor::global().running();
    if running.is_empty() || since.elapsed() > Duration::from_secs(10) {
        let left: Vec<String> = running
            .iter()
            .map(|process| format!("{:?} {}", process.kind, process.label))
            .collect();
        check(
            "closing the windows ends their processes",
            running.is_empty(),
            left.join(", "),
        );
        end();
    } else {
        main_after(Duration::from_millis(200), move || wind_down(since));
    }
}
