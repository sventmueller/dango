//! The `activity-actions` scenario: the activity item opens and closes its popover, and the popover's buttons do what
//! they say: Show goes to the AI tab, to a subagent's place in it and to the terminal a process runs in, Stop ends
//! that process. The terminal is one a command was typed into as it opened, as a debugger's `runInTerminal` asks; the
//! title the command sets becomes its tab's, and a process it starts leads to the tab too. Of two terminals, the one in
//! view is the one whose screen `@terminal` hands over.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use ide_ui_agent::AgentKey;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadOnly, Message, sel};
use objc2_app_kit::{NSAccessibility, NSApplication, NSButton, NSToolbarItem, NSView};

use super::agent::tab_of;
use super::{check, finish, give_up, helpers};
use crate::window::{TabId, WindowController};

const NAME: &str = "activity-actions";
const SUBAGENT: &str = include_str!("../../crates/ide-claude/tests/fixtures/recorded_subagent_background.ndjson");
/// The lines of `SUBAGENT` after which its subagent and background shell both run.
const RUNNING: usize = 174;
const MARKER: &str = "activity-marker";
const TITLE: &str = "activity-title";
/// What the shell prints before the number of the process it started in the background.
const CHILD: &str = "child:";

/// The shell's process the typed command reached, 0 before the answer and `u32::MAX` for none.
static SHELL: AtomicU32 = AtomicU32::new(0);

pub(crate) fn run() {
    helpers::open_project(NAME, "activity-project", &[], move |controller| {
        let root = helpers::root("activity-project");
        let Some(tab) = ide_ui_agent::open_replay(&controller) else {
            return give_up(NAME, "an AI tab opens");
        };
        let running: String = SUBAGENT.lines().take(RUNNING).map(|line| format!("{line}\n")).collect();
        tab.feed(&running);
        let key = tab.key;
        let line = format!("printf '\\033]0;%s\\007' {TITLE}; echo {MARKER}; sleep 20 & echo {CHILD}$!\n");
        crate::terminal::open_running(&controller, &root, line, |pid| {
            SHELL.store(pid.unwrap_or(u32::MAX), Ordering::SeqCst);
        });
        helpers::wait_until(|| SHELL.load(Ordering::SeqCst) != 0, move |_| typed(root, key));
    });
}

fn typed(root: PathBuf, key: AgentKey) {
    let Some(controller) = helpers::window_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let pid = SHELL.load(Ordering::SeqCst);
    let tab = crate::terminal::tab_of_process(&controller, pid);
    check(
        "a command typed into a new terminal reaches its shell, whose tab the process leads to",
        pid != 0 && pid != u32::MAX && tab.is_some(),
        format!("pid {pid}, tab {tab:?}"),
    );
    let probe = root.clone();
    helpers::wait_until(
        move || {
            helpers::window_of(&probe)
                .and_then(|c| crate::terminal::recent_output(&c))
                .is_some_and(|(_, text)| text.matches(MARKER).count() >= 2 && child_of(&text).is_some())
        },
        move |ok| {
            let output = helpers::window_of(&root).and_then(|c| crate::terminal::recent_output(&c));
            check(
                "the terminal shows the command and what it printed",
                ok,
                format!("{output:?}"),
            );
            let child = output.as_ref().and_then(|(_, text)| child_of(text));
            let leads = helpers::window_of(&root)
                .zip(child)
                .and_then(|(c, child)| crate::terminal::tab_of_process(&c, child));
            check(
                "a process the shell started leads to the terminal's tab, through its parent",
                tab.is_some() && leads == tab,
                format!("child {child:?} → {leads:?}, terminal {tab:?}"),
            );
            let titles = helpers::window_of(&root).map(|c| c.tab_titles()).unwrap_or_default();
            check(
                "a title the shell sets becomes the terminal tab's title",
                titles.iter().any(|title| title.contains(TITLE)),
                format!("{titles:?}"),
            );
            popover(root, key, tab);
        },
    );
}

fn popover(root: PathBuf, key: AgentKey, terminal: Option<TabId>) {
    let Some(controller) = helpers::window_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    click_activity_item(&controller);
    check("the activity item opens its popover", popover_shown(&controller), "");
    let pid = SHELL.load(Ordering::SeqCst);
    let shell = ide_process::Supervisor::global()
        .running()
        .into_iter()
        .find(|p| p.pid == pid)
        .map(|p| p.label)
        .unwrap_or_default();
    let session = tab_of(key).map(|tab| tab.title()).unwrap_or_default();
    let labels: Vec<String> = buttons(&controller).into_iter().map(|(label, _)| label).collect();
    check(
        "the popover has buttons for the session, its subagent, its background shell and the terminal's process",
        !shell.is_empty()
            && [session.as_str(), "list files", "30-tick", shell.as_str()]
                .iter()
                .all(|part| labels.iter().any(|l| l.contains(part))),
        format!("{} (shell {shell:?}, session {session:?})", labels.join(" | ")),
    );
    let Some(ai) = tab_of(key).and_then(|tab| controller.tab_with_key(&tab.tab_key())) else {
        return give_up(NAME, "the AI tab is in the window");
    };
    let Some(terminal) = terminal else {
        return give_up(NAME, "the terminal tab is open");
    };
    let show = splat_i18n::t("Zeigen");
    press(&controller, |label| label == format!("{show}: {shell}"));
    check(
        "Show of a terminal's process selects its tab",
        controller.selected_tab() == Some(terminal),
        format!("{:?}, terminal {terminal:?}", controller.selected_tab()),
    );
    press(&controller, |label| label == format!("{show}: {session}"));
    check(
        "Show of a session selects its AI tab",
        controller.selected_tab() == Some(ai),
        format!("{:?}, AI tab {ai:?}", controller.selected_tab()),
    );
    controller.select_tab_by_id(terminal);
    press(&controller, |label| {
        label.starts_with(show) && label.contains("list files")
    });
    check(
        "Show of a subagent brings its AI tab to the front",
        controller.selected_tab() == Some(ai),
        format!("{:?}, AI tab {ai:?}", controller.selected_tab()),
    );
    let (probe, wanted) = (root.clone(), format!("{}: {shell}", splat_i18n::t("Stopp")));
    helpers::wait_until(
        move || helpers::window_of(&probe).is_some_and(|c| buttons(&c).iter().any(|(label, _)| *label == wanted)),
        move |_| stop(root, shell),
    );
}

fn stop(root: PathBuf, shell: String) {
    let Some(controller) = helpers::window_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let pid = SHELL.load(Ordering::SeqCst);
    let stop = splat_i18n::t("Stopp");
    let pressed = press(&controller, |label| label == format!("{stop}: {shell}"));
    let running = move || ide_process::Supervisor::global().running().iter().any(|p| p.pid == pid);
    helpers::wait_until(
        move || !running(),
        move |ok| {
            check(
                "Stop of a terminal's process ends it",
                pressed && ok,
                format!("pressed {pressed}"),
            );
            let Some(controller) = helpers::window_of(&root) else {
                return give_up(NAME, "the folder's window is open");
            };
            click_activity_item(&controller);
            let probe = root.clone();
            let closing = move || helpers::window_of(&probe).is_some_and(|c| !popover_shown(&c));
            helpers::wait_until(closing, move |closed| {
                check("the activity item closes its open popover", closed, "");
                second_terminal(root);
            });
        },
    );
}

/// The number of the process the shell said it started, from the screen's `text`.
fn child_of(text: &str) -> Option<u32> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix(CHILD))
        .find_map(|rest| rest.trim().parse().ok())
}

/// Of two terminals, the one in view is the one whose screen `@terminal` hands over, also when the other is newer.
fn second_terminal(root: PathBuf) {
    let Some(controller) = helpers::window_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let first = crate::terminal::recent_output(&controller).map(|(label, _)| label);
    crate::terminal::open_tab(&controller, None);
    let probe = root.clone();
    let before = first.clone();
    helpers::wait_until(
        move || {
            helpers::window_of(&probe)
                .and_then(|c| crate::terminal::recent_output(&c))
                .is_some_and(|(label, text)| Some(&label) != before.as_ref() && !text.trim().is_empty())
        },
        move |opened| {
            let Some(controller) = helpers::window_of(&root) else {
                return give_up(NAME, "the folder's window is open");
            };
            let titles = controller.tab_titles();
            let first_tab = titles
                .iter()
                .position(|title| title.contains(TITLE))
                .and_then(|at| controller.tab_ids().get(at).copied());
            if let Some(id) = first_tab {
                controller.select_tab_by_id(id);
            }
            let now = crate::terminal::recent_output(&controller).map(|(label, _)| label);
            check(
                "of two terminals, the one in view is the one whose screen is handed over, not the newer",
                opened && first_tab.is_some() && now == first,
                format!("first {first:?}, now {now:?}, tabs {titles:?}"),
            );
            finish(NAME);
        },
    );
}

/// Sends the activity toolbar item's action, as a click on it does.
fn click_activity_item(controller: &WindowController) {
    let item: Option<Retained<NSToolbarItem>> = controller.window().and_then(|w| w.toolbar()).and_then(|toolbar| {
        toolbar
            .items()
            .iter()
            .find(|item| item.itemIdentifier().to_string() == "ide.activity")
    });
    let Some(item) = item else {
        check("the window has the activity item", false, "");
        return;
    };
    let app = NSApplication::sharedApplication(controller.mtm());
    let target = item.target();
    if let Some(action) = item.action() {
        // SAFETY: The item's own action and target, with the item as the sender, as AppKit sends them.
        unsafe { app.sendAction_to_from(action, target.as_deref(), Some(&item)) };
    }
}

fn popover_shown(controller: &WindowController) -> bool {
    crate::activity::popover_view(controller)
        .and_then(|view| view.window())
        .is_some_and(|window| window.isVisible())
}

/// The popover's buttons with their VoiceOver labels.
fn buttons(controller: &WindowController) -> Vec<(String, Retained<NSButton>)> {
    let mut found = Vec::new();
    if let Some(view) = crate::activity::popover_view(controller) {
        collect(&view, &mut found);
    }
    found
}

fn collect(view: &NSView, found: &mut Vec<(String, Retained<NSButton>)>) {
    if let Some(button) = view.downcast_ref::<NSButton>()
        && button.action() == Some(sel!(ideActivityAction:))
    {
        let label = button.accessibilityLabel().map(|l| l.to_string()).unwrap_or_default();
        found.push((label, button.retain()));
    }
    for child in view.subviews().iter() {
        collect(&child, found);
    }
}

/// Clicks the first button of the popover whose label `wanted` accepts; whether there was one.
fn press(controller: &WindowController, wanted: impl Fn(&str) -> bool) -> bool {
    let Some((_, button)) = buttons(controller).into_iter().find(|(label, _)| wanted(label)) else {
        return false;
    };
    let target: Option<Retained<AnyObject>> = button.target();
    let app = NSApplication::sharedApplication(controller.mtm());
    if let Some(action) = button.action() {
        // SAFETY: The button's own action and target, with the button as the sender, as a click sends them.
        unsafe { app.sendAction_to_from(action, target.as_deref(), Some(&button)) };
    }
    true
}
