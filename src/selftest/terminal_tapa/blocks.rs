//! Command blocks and the find bar: blocks switched on in the settings, two commands of which one fails, the prompt
//! jumps, selecting the last block, copying its output, handing it to an agent, and searching the terminal.

use std::rc::Rc;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSMenuItem, NSTextField, NSView};
use objc2_foundation::NSPoint;
use splat_i18n::{t, tf};

use super::super::{check, helpers, snapshot};
use super::{
    Run, after_prompt, bring_back, command, controller_of, inspect, menu_item, pane_of, pasted_text, stop, wait,
};
use crate::terminal::TerminalView;
use crate::window::TabId;

/// A command whose output reaches past the screen, so that its prompt scrolls off.
const LONG: &str = "seq 1 150";
/// A command that fails with exit code 3 after one line of output.
const FAILING: &str = "sh -c 'echo tapa-failing; exit 3'";
/// What the failing command prints, which the find bar then looks for.
const FAILED_OUTPUT: &str = "tapa-failing";

/// Switches the blocks on in the settings and runs the two commands.
pub(super) fn switch_on(run: Run) {
    let found = super::switch(sel!(ideToggleTerminalBlocks:), true);
    check("the Terminal settings page has the switch of the blocks", found, "");
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let before = view.prompts();
    view.type_text(&format!("{LONG}\r"));
    after_prompt(run, before, failing);
}

fn failing(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let before = view.prompts();
    view.type_text(&format!("{FAILING}\r"));
    after_prompt(run, before, report);
}

/// The exit code of the block whose command is `line` among those `view` shows, once it ended.
fn exit_code(view: &TerminalView, line: &str) -> Option<i32> {
    let shown = view.with_session(term_session::Session::blocks_view)?;
    shown
        .blocks
        .iter()
        .chain(shown.sticky.iter())
        .find(|entry| entry.block.command.as_deref().map(str::trim) == Some(line))
        .and_then(|entry| entry.block.exit_code)
}

/// Whether the view draws the blocks over its lines.
fn draws_blocks(view: &TerminalView) -> bool {
    view.subviews()
        .iter()
        .any(|child| child.class().name().to_bytes() == b"TermBlockOverlay")
}

/// Both commands show as blocks with their exit codes, drawn over the lines.
fn report(run: Run) {
    let shown = |view: &TerminalView| {
        exit_code(view, LONG) == Some(0) && exit_code(view, FAILING) == Some(3) && draws_blocks(view)
    };
    wait(&run.clone(), 10, shown, move |shown| {
        let report = inspect::view(run.id)
            .map(|view| view.blocks_report())
            .unwrap_or_default();
        check(
            "with blocks switched on in the settings, both commands show as blocks drawn over the lines with their \
             exit codes",
            shown,
            report,
        );
        jumps(run);
    });
}

fn scrolled_back(run: &Run) -> usize {
    inspect::view(run.id).map_or(0, |view| view.scrolled_back())
}

/// The previous prompt's command scrolls to the top, the next one back towards the end, and once more to the end.
fn jumps(run: Run) {
    let previous = command(&run, t("Voriger Prompt"));
    wait(
        &run.clone(),
        5,
        |view| view.scrolled_back() > 0,
        move |_| {
            let back = scrolled_back(&run);
            let top: Vec<String> = inspect::view(run.id)
                .map(|view| view.visible_lines().into_iter().take(3).collect())
                .unwrap_or_default();
            let next = command(&run, t("Nächster Prompt"));
            wait(
                &run.clone(),
                5,
                move |view| view.scrolled_back() < back,
                move |_| {
                    let forward = scrolled_back(&run);
                    command(&run, t("Nächster Prompt"));
                    wait(
                        &run.clone(),
                        5,
                        |view| view.scrolled_back() == 0,
                        move |end| {
                            check(
                                "the prompt jumps scroll the previous command's prompt to the top and then back to \
                                 the end",
                                previous
                                    && next
                                    && back > 0
                                    && top.iter().any(|line| line.ends_with(LONG))
                                    && forward < back
                                    && end,
                                format!("{back} lines back with {top:?} on top, then {forward}, at the end {end}"),
                            );
                            select_and_copy(run);
                        },
                    );
                },
            );
        },
    );
}

/// Select Block takes the last command with its output; Copy Output puts the output on the pasteboard.
fn select_and_copy(run: Run) {
    let Some(view) = inspect::view(run.id) else {
        return stop(&run, "the terminal is open");
    };
    let selected = command(&run, t("Block auswählen"));
    let selection = view.selection().unwrap_or_default();
    let copied = command(&run, t("Ausgabe kopieren"));
    let pasted = pasted_text().unwrap_or_default();
    check(
        "Select Block selects the last command with its output, and Copy Output puts its output on the pasteboard",
        selected
            && copied
            && selection.contains(FAILING)
            && selection.contains(&format!("\n{FAILED_OUTPUT}"))
            && pasted.trim() == FAILED_OUTPUT,
        format!("selected {selection:?}, pasted {pasted:?}"),
    );
    to_agent(run);
}

/// What the newest AI tab of the run's window holds: its draft, its text from outside and its attachments' names.
#[derive(Debug, Default)]
struct Handed {
    draft: String,
    outside: Vec<String>,
    names: Vec<String>,
}

impl Handed {
    fn of(run: &Run) -> Handed {
        agent_tab(run).map_or_else(Handed::default, |tab| Handed {
            draft: tab.composer_text(),
            outside: tab.outside_texts(),
            names: tab.attachment_names(),
        })
    }

    /// Whether the draft names the failed command and its exit code, and the attachment holds the block as text
    /// from outside.
    fn holds_block(&self) -> bool {
        let block = self.outside.first().map(String::as_str).unwrap_or_default();
        self.draft.contains(FAILING)
            && self.draft.contains('3')
            && block.starts_with(&format!("$ {FAILING}\n"))
            && block.contains(FAILED_OUTPUT)
            && self.names.iter().any(|name| name.ends_with("(external)"))
    }
}

/// Calls `then` with the tab that opened in the run's window since it had `tabs`, once it holds text from outside,
/// and what it holds.
fn when_handed(run: Run, tabs: Vec<TabId>, then: impl FnOnce(Run, Option<TabId>, Handed) + 'static) {
    let opened = {
        let run = run.clone();
        move || {
            controller_of(&run)?
                .tab_ids()
                .into_iter()
                .find(|id| !tabs.contains(id))
                .filter(|_| !Handed::of(&run).outside.is_empty())
        }
    };
    helpers::wait_for_within(Duration::from_secs(5), opened, move |tab| {
        let handed = Handed::of(&run);
        then(run, tab, handed);
    });
}

/// Block to Agent from the Edit menu opens an AI tab with a draft about the failure and the block as text from
/// outside.
fn to_agent(run: Run) {
    let Some(controller) = controller_of(&run) else {
        return stop(&run, "the folder's window is open");
    };
    let tabs = controller.tab_ids();
    let chosen = command(&run, t("Block an Agent übergeben"));
    when_handed(run, tabs, move |run, opened, handed| {
        check(
            "Block to Agent from the Edit menu opens an AI tab whose draft names the failed command and whose \
             attachment holds its command line and output as text from outside",
            chosen && opened.is_some() && handed.holds_block(),
            format!("{opened:?}: {handed:?}"),
        );
        bring_back(&run);
        context_to_agent(run);
    });
}

/// The item of the terminal's context menu that sends `action`, as a right click on the view shows the menu.
fn context_item(view: &TerminalView, action: objc2::runtime::Sel) -> Option<Retained<NSMenuItem>> {
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::RightMouseDown,
        NSPoint::new(10.0, 10.0),
        NSEventModifierFlags::empty(),
        0.0,
        view.window().map_or(0, |window| window.windowNumber()),
        None,
        0,
        1,
        1.0,
    )?;
    // SAFETY: `menuForEvent:` takes the event, which lives for the call, and returns a menu or nil.
    let menu: Option<Retained<NSMenu>> = unsafe { msg_send![view, menuForEvent: &*event] };
    menu?.itemArray().iter().find(|item| item.action() == Some(action))
}

/// The terminal's context menu ends with Block to Agent, tagged with the view's id, which hands the block over as the
/// Edit menu's item does.
fn context_to_agent(run: Run) {
    let (Some(controller), Some(view)) = (controller_of(&run), inspect::view(run.id)) else {
        return stop(&run, "the terminal and the folder's window are open");
    };
    let Some(item) = context_item(&view, sel!(ideTerminalContextCommand:)) else {
        return stop(&run, "the terminal's context menu offers Block to Agent");
    };
    let (title, tag) = (item.title().to_string(), item.tag());
    let tabs = controller.tab_ids();
    // SAFETY: The window answers `ideTerminalContextCommand:`, which takes the menu item that sends it.
    let () = unsafe { msg_send![&*controller, ideTerminalContextCommand: &*item] };
    when_handed(run, tabs, move |run, opened, handed| {
        check(
            "the terminal's context menu ends with Block to Agent, tagged with the view, which hands the block to a \
             new AI tab as the Edit menu's item does",
            title == t("Block an Agent übergeben")
                && u64::try_from(tag).ok() == Some(run.id)
                && opened.is_some()
                && handed.holds_block(),
            format!("{title:?} tagged {tag}, {opened:?}: {handed:?}"),
        );
        let Some(other) = opened else {
            return stop(&run, "the AI tab opens");
        };
        bring_back(&run);
        find(run, other);
    });
}

/// The newest AI tab of the run's window.
fn agent_tab(run: &Run) -> Option<Rc<ide_ui_agent::AgentTab>> {
    ide_ui_agent::of_window(&*controller_of(run)?).last().cloned()
}

/// The find bar of `view` while it shows: whether its field has the keyboard, and what its labels say.
fn find_bar(view: &TerminalView) -> Option<(bool, Vec<String>)> {
    let pane = pane_of(view)?;
    let mut fields = Vec::new();
    text_fields(&pane, &mut fields);
    let shown: Vec<_> = fields
        .iter()
        .filter(|field| !field.isHiddenOrHasHiddenAncestor())
        .collect();
    let search = shown.iter().find(|field| field.isEditable())?;
    let labels = shown
        .iter()
        .filter(|field| !field.isEditable())
        .map(|field| field.stringValue().to_string())
        .collect();
    Some((search.currentEditor().is_some(), labels))
}

fn text_fields(view: &NSView, found: &mut Vec<objc2::rc::Retained<NSTextField>>) {
    if let Some(field) = view.downcast_ref::<NSTextField>() {
        found.push(objc2::Message::retain(field));
    }
    for child in view.subviews().iter() {
        text_fields(&child, found);
    }
}

/// ⌘F opens the find bar with the keyboard in it, a search counts its matches, and closing it hands the keyboard
/// back; `other` is the AI tab, which the next step puts in front of the terminal.
fn find(run: Run, other: TabId) {
    let (Some(controller), Some(view), Some(mtm)) =
        (controller_of(&run), inspect::view(run.id), MainThreadMarker::new())
    else {
        return stop(&run, "the terminal and the folder's window are open");
    };
    let Some(item) = menu_item(mtm, sel!(performTextFinderAction:), |item| item.tag() == 1) else {
        return stop(&run, "the Find menu has its Find item");
    };
    // SAFETY: The window answers `performTextFinderAction:`, which takes the menu item that sends it.
    let () = unsafe { msg_send![&*controller, performTextFinderAction: &*item] };
    let opened = find_bar(&view);
    view.search_for(FAILED_OUTPUT);
    let counted = |labels: &[String]| {
        (1..=2).any(|at| {
            labels
                .iter()
                .any(|label| *label == tf("{} von {}", &[&at.to_string(), "2"]))
        })
    };
    wait(
        &run.clone(),
        5,
        move |view| find_bar(view).is_some_and(|(_, labels)| counted(&labels)),
        move |found| {
            let Some(view) = inspect::view(run.id) else {
                return stop(&run, "the terminal is open");
            };
            let labels = find_bar(&view).map(|(_, labels)| labels).unwrap_or_default();
            if let Some(window) = view.window() {
                snapshot(&window, "terminal-tapa-find");
            }
            view.end_find();
            let closed = find_bar(&view).is_none();
            let back = view
                .window()
                .and_then(|window| window.firstResponder())
                .is_some_and(|responder| {
                    let responder: &AnyObject = &responder;
                    let own: &AnyObject = &view;
                    std::ptr::eq(responder, own)
                });
            check(
                "⌘F opens the terminal's find bar with the keyboard in it, a search counts its matches, and closing \
                 it gives the keyboard back to the terminal",
                opened.as_ref().is_some_and(|(focused, _)| *focused) && found && closed && back,
                format!("opened {opened:?}, then {labels:?}, closed {closed}, terminal has the keyboard {back}"),
            );
            super::signals::behind(run, other);
        },
    );
}
