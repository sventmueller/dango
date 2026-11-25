//! The states the audit walks, one after another in the window of the test project: the trust sheet, files and
//! search, Open Quickly, an editor whose language server is missing, the Git panel with a diff and a sheet, the CI
//! panel with a log, an AI tab playing a recording with the AI panel and the activity popover, a terminal, a CLI
//! thread, a browser tab, two panels, and the settings window.

use std::path::PathBuf;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{NSAlertFirstButtonReturn, NSButton, NSEventModifierFlags, NSTabViewController, NSView};

use super::super::git::{answer_sheet, sheet_ready};
use super::super::helpers::{after, wait_until};
use super::{flows, keys, name, picture, walk, window_of};
use crate::selftest::{check, give_up};
use crate::window::{Panel, search_selftest};
use ide_ui_agent::AgentKey;
use ide_ui_git::selftest as git;

/// The file of the editor state, in a language whose server is not installed, so its status bar offers it.
pub(crate) const SCRIPT: &str = "src/app.ts";
/// How long a view takes to lay out and draw what it just got.
const SETTLE: Duration = Duration::from_millis(600);
pub(super) const SUBAGENT: &str =
    include_str!("../../../crates/ide-claude/tests/fixtures/recorded_subagent_background.ndjson");
const EDIT: &str = include_str!("../../../crates/ide-claude/tests/fixtures/recorded_edit_diff.ndjson");
/// The lines of `SUBAGENT` after which its subagent and background shell both run.
const RUNNING: usize = 174;

/// Calls `then` with `root` once `ready` holds for its window, or with false after the usual wait.
fn when(
    root: PathBuf,
    ready: impl Fn(&crate::window::WindowController) -> bool + 'static,
    then: impl FnOnce(PathBuf, bool) + 'static,
) {
    let probe = root.clone();
    wait_until(
        move || super::super::helpers::window_of(&probe).is_some_and(|c| ready(&c)),
        move |ok| then(root, ok),
    );
}

/// Goes on with `then` once the views had a moment to draw.
fn settled(root: PathBuf, then: impl FnOnce(PathBuf) + 'static) {
    after(SETTLE, move || then(root));
}

pub(crate) fn start(root: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(&crate::context::app(), mtm, root.clone());
    when(root, sheet_ready, trust_sheet);
}

fn trust_sheet(root: PathBuf, ok: bool) {
    check("an untrusted folder asks for trust", ok, "");
    walk("the trust sheet misses nothing");
    let Some(controller) = window_of(&root) else { return };
    if let Some(sheet) = controller.window().and_then(|w| w.attachedSheet()) {
        let _ = keys::press(&sheet, "\r", 36, NSEventModifierFlags::empty());
    }
    let probe = root.clone();
    wait_until(
        move || crate::project::is_trusted(&probe),
        move |ok| {
            check("Return on the trust sheet trusts the folder", ok, "");
            if !ok {
                answer_sheet(&root, NSAlertFirstButtonReturn, None);
            }
            when(root, |c| c.tree_rows() > 0 && !c.has_sheet(), files);
        },
    );
}

fn files(root: PathBuf, ok: bool) {
    check("the files panel lists the folder", ok, "");
    walk("the files panel misses nothing");
    picture(&root, "files");
    let Some(controller) = window_of(&root) else { return };
    controller.show_search();
    if let Some(panel) = controller.search_panel() {
        search_selftest::type_query(&panel, "greet");
    }
    when(
        root,
        |c| {
            c.search_panel()
                .is_some_and(|p| search_selftest::seen(&p).files.len() == 1)
        },
        search,
    );
}

fn search(root: PathBuf, ok: bool) {
    check("the project search finds the file", ok, "");
    walk("the project search misses nothing");
    let Some(controller) = window_of(&root) else { return };
    controller.quick_open_with("app");
    when(root, |c| !c.quick_open_hits().is_empty(), quick_open);
}

fn quick_open(root: PathBuf, ok: bool) {
    check("Open Quickly finds the file", ok, "");
    walk("Open Quickly misses nothing");
    let Some(controller) = window_of(&root) else { return };
    if let Some(panel) = flows::child_panel(&controller) {
        let _ = keys::press(&panel, "\u{1b}", 53, NSEventModifierFlags::empty());
    }
    check("Esc closes Open Quickly", flows::child_panel(&controller).is_none(), "");
    let file = root.join(SCRIPT);
    controller.open_file(&file);
    when(
        root,
        move |c| crate::editor::editor_for(c, &file).is_some(),
        |root, _| settled(root, editor),
    );
}

fn editor(root: PathBuf) {
    walk("the editor with its status bar misses nothing");
    picture(&root, "editor");
    let Some(controller) = window_of(&root) else { return };
    controller.activate_panel(Panel::Git);
    when(root, |c| git::seen(c).is_some_and(|s| s.changes.len() >= 3), git_panel);
}

fn git_panel(root: PathBuf, ok: bool) {
    check("the Git panel lists the changes", ok, "");
    let seen = window_of(&root).and_then(|c| git::seen(&*c));
    check(
        "the Git panel read while out of view shows its rows once it comes into view",
        seen.as_ref()
            .is_some_and(|s| usize::try_from(s.rows).unwrap_or(0) > s.changes.len()),
        format!("{:?}", seen.map(|s| (s.rows, s.changes.len()))),
    );
    walk("the Git panel misses nothing");
    picture(&root, "git");
    let Some(controller) = window_of(&root) else { return };
    git::show_diff(&controller, &root.join(SCRIPT), false);
    let file = root.join(SCRIPT);
    when(root, move |c| git::diff_counts(c, &file, false).is_some(), diff);
}

fn diff(root: PathBuf, ok: bool) {
    check("the diff tab reads the diff", ok, "");
    walk("the diff tab misses nothing");
    picture(&root, "diff");
    let Some(controller) = window_of(&root) else { return };
    git::discard(&controller, std::path::Path::new(SCRIPT));
    when(root, |c| c.has_sheet(), confirm_sheet);
}

fn confirm_sheet(root: PathBuf, ok: bool) {
    check("discarding asks first", ok, "");
    walk("the sheet before discarding misses nothing");
    let Some(controller) = window_of(&root) else { return };
    if let Some(sheet) = controller.window().and_then(|w| w.attachedSheet()) {
        let _ = keys::press(&sheet, "\u{1b}", 53, NSEventModifierFlags::empty());
    }
    let script = root.join(SCRIPT);
    when(
        root,
        |c| !c.has_sheet(),
        move |root, ok| {
            let kept = std::fs::read_to_string(&script).is_ok_and(|text| text.contains("Hello"));
            check("Esc on the sheet keeps the change", ok && kept, "");
            if !ok {
                answer_sheet(&root, objc2_app_kit::NSAlertSecondButtonReturn, None);
            }
            ci(root);
        },
    );
}

fn ci(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    controller.activate_panel(Panel::Ci);
    when(
        root,
        |c| crate::ci_panel::selftest::seen(c).is_some_and(|s| s.runs.len() == 2),
        ci_panel,
    );
}

fn ci_panel(root: PathBuf, ok: bool) {
    check("the CI panel lists the runs", ok, "");
    walk("the CI panel misses nothing");
    let Some(controller) = window_of(&root) else { return };
    crate::ci_panel::selftest::press_return_on(&controller, 9001, 72);
    when(
        root,
        |c| crate::ci_panel::selftest::log(c, 72, false).is_some_and(|(lines, _)| !lines.is_empty()),
        ci_log,
    );
}

fn ci_log(root: PathBuf, ok: bool) {
    check("Return on a job opens its log", ok, "");
    walk("the log tab misses nothing");
    flows::commit(root, ai);
}

fn ai(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    let Some(tab) = ide_ui_agent::open_replay(&controller) else {
        return give_up(name(), "an AI tab opens");
    };
    let key = tab.key;
    tab.feed(EDIT);
    let running: String = SUBAGENT.lines().take(RUNNING).map(|line| format!("{line}\n")).collect();
    tab.feed(&running);
    tab.timeline().scroll_to_end();
    settled(root, move |root| ai_tab(root, key));
}

/// `key` is the AI tab that plays the recording with a subagent and a background shell.
fn ai_tab(root: PathBuf, key: AgentKey) {
    walk("the AI tab misses nothing");
    picture(&root, "ai");
    let Some(controller) = window_of(&root) else { return };
    controller.activate_panel(Panel::Agents);
    settled(root, move |root| agents_panel(root, key));
}

fn agents_panel(root: PathBuf, key: AgentKey) {
    walk("the AI panel misses nothing");
    let Some(controller) = window_of(&root) else { return };
    let (lines, _) = crate::activity::inspect(&controller);
    check(
        "the activity popover lists what runs",
        !lines.is_empty(),
        lines.join(" | "),
    );
    settled(root, move |root| activity(root, key));
}

/// How long the popover runs on while a button of it has the focus, past its updates each second.
const TICKS: Duration = Duration::from_millis(2500);

fn activity(root: PathBuf, key: AgentKey) {
    walk("the activity popover misses nothing");
    let Some(controller) = window_of(&root) else { return };
    let button = crate::activity::popover_view(&controller).and_then(|view| first_button(&view));
    let Some((button, window)) = button.and_then(|b| b.window().map(|w| (b, w))) else {
        check(
            "the activity popover keeps the focus while its rows update",
            false,
            "no button",
        );
        return terminal_tab(root);
    };
    let focused = window.makeFirstResponder(Some(&button));
    let address = Retained::as_ptr(&button) as usize;
    let before = crate::activity::updated_in_place();
    after(TICKS, move || {
        let Some(controller) = window_of(&root) else { return };
        let now = crate::activity::popover_view(&controller).and_then(|view| first_button(&view));
        let kept = now.as_ref().is_some_and(|button| {
            let same = Retained::as_ptr(button) as usize == address;
            let responder = button.window().and_then(|w| w.firstResponder());
            same && responder.is_some_and(|r| Retained::as_ptr(&r).cast::<AnyObject>() as usize == address)
        });
        let updates = crate::activity::updated_in_place() - before;
        check(
            "the activity popover keeps the focus while its rows update",
            focused && kept && updates > 0,
            format!("focused {focused}, kept {kept}, {updates} updates in place"),
        );
        super::popover::rows_change(root, key, terminal_tab);
    });
}

/// The first button in `view` or below it.
fn first_button(view: &NSView) -> Option<Retained<NSButton>> {
    if let Some(button) = view.downcast_ref::<NSButton>() {
        return Some(button.retain());
    }
    view.subviews().iter().find_map(|sub| first_button(&sub))
}

fn terminal_tab(root: PathBuf) {
    let Some(controller) = window_of(&root) else { return };
    crate::activity::close(&controller);
    crate::terminal::open_tab(&controller, None);
    let id = crate::terminal::inspect::latest().map_or(0, |(id, _)| id);
    when(
        root,
        move |_| crate::terminal::inspect::view(id).is_some_and(|v| v.shell().is_some()),
        terminal,
    );
}

fn terminal(root: PathBuf, ok: bool) {
    check("the terminal's shell starts", ok, "");
    walk("the terminal misses nothing");
    picture(&root, "terminal");
    let Some(controller) = window_of(&root) else { return };
    match crate::selftest::browser::serve_page() {
        Ok(port) => crate::selftest::browser::PAGE_PORT.store(port, std::sync::atomic::Ordering::SeqCst),
        Err(error) => check("the page server starts", false, error.to_string()),
    }
    crate::browser::open_tab(&controller, Some(&crate::selftest::browser::page_url("/")));
    when(
        root,
        |c| c.tab_titles().iter().any(|t| t == crate::selftest::browser::PAGE_TITLE),
        browser,
    );
}

fn browser(root: PathBuf, ok: bool) {
    check("the browser tab loads the page", ok, "");
    walk("the browser tab misses nothing");
    let Some(controller) = window_of(&root) else { return };
    controller.open_file_beside(&root.join("notes.txt"), crate::window::panes::Side::Right);
    when(root, |c| c.pane_bars().len() == 2, beside);
}

fn beside(root: PathBuf, ok: bool) {
    check("a file opens beside the browser in a pane of its own", ok, "");
    walk("two documents side by side miss nothing");
    picture(&root, "beside");
    if let Some(controller) = window_of(&root) {
        check(
            "every pane shows its own tab bar in place of the window's (E145)",
            !controller.tab_bar_visible() && controller.pane_bars().iter().all(|(bar, _)| *bar),
            format!("{:?} {:?}", controller.window_tabs(), controller.pane_bars()),
        );
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::settings::show(&crate::context::app(), mtm);
    settled(root, settings_general);
}

fn settings_general(root: PathBuf) {
    walk("the general settings miss nothing");
    if let Some(window) = crate::settings::window() {
        crate::selftest::snapshot(&window, &format!("{}-settings", name()));
    }
    let tabs = crate::settings::window()
        .and_then(|w| w.contentViewController())
        .and_then(|c| c.downcast::<NSTabViewController>().ok());
    if let Some(tabs) = tabs {
        tabs.setSelectedTabViewItemIndex(1);
    }
    settled(root, settings_ci);
}

fn settings_ci(root: PathBuf) {
    walk("the CI settings miss nothing");
    if let Some(window) = crate::settings::window() {
        window.orderOut(None);
    }
    flows::start(root);
}
