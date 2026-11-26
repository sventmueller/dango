//! The `terminal-tapa` scenario (phase 14, stage 8): what Tapa's terminal brings beyond typing and drawing. The
//! history's suggestion at the prompt, which a switch of the Terminal settings page takes away at once; the input
//! field from the View menu with its suggestion, completion menu, colours and Return; command blocks with their exit
//! codes, the prompt jumps, selecting a block, copying its output and handing it to an agent; the find bar;
//! notifications and progress of a terminal behind another tab; smart selection and copying with formatting; the
//! sheet before a multi-line paste; and Secure Keyboard Entry. It puts back every setting it switched.

mod blocks;
mod field;
mod selection;
mod signals;

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use ide_core::settings::{Key, Settings, keys};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, Message, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSButton, NSControlStateValueOff, NSControlStateValueOn, NSEvent, NSEventModifierFlags, NSEventType,
    NSMenu, NSMenuItem, NSPasteboard, NSPasteboardType, NSPasteboardTypeString, NSTabViewController, NSView,
};
use objc2_foundation::{NSPoint, NSString};
use serde_json::Value;

use super::{finish, give_up, helpers};
use crate::terminal::{TerminalView, inspect};
use crate::util::constant;
use crate::window::{TabId, WindowController};

const NAME: &str = "terminal-tapa";
const PROJECT: &str = "terminal-tapa-project";

/// The settings the scenario switches, each with the value it starts from, which is Tapa's default.
const SWITCHED: [(Key<bool>, bool); 4] = [
    (keys::TERMINAL_INPUT_FIELD, false),
    (keys::TERMINAL_SUGGESTIONS, true),
    (keys::TERMINAL_BLOCKS, false),
    (keys::TERMINAL_PASTE_PROTECTION, true),
];

/// The settings of `SWITCHED` as the user's settings file held them, by name; none where it held no value.
type Kept = Rc<Vec<(&'static str, Option<Value>)>>;

/// What each step hands the next: the terminal view, its tab, the folder's window and the settings as they were.
#[derive(Clone)]
struct Run {
    id: u64,
    tab: TabId,
    root: PathBuf,
    kept: Kept,
}

pub(crate) fn run() {
    let mut settings = Settings::load_user();
    let stored = settings.user_values();
    let kept: Vec<(&'static str, Option<Value>)> = SWITCHED
        .iter()
        .map(|(key, _)| (key.declared().name, stored.get(key.declared().name).cloned()))
        .collect();
    for (key, start) in &SWITCHED {
        if settings.get(key) != *start {
            let _ = settings.set(key, start);
        }
    }
    let kept = Rc::new(kept);
    helpers::open_project(
        NAME,
        PROJECT,
        &[("README.md", "# Terminal Tapa\n")],
        move |controller| {
            opened(&controller, kept);
        },
    );
}

fn opened(controller: &WindowController, kept: Kept) {
    crate::terminal::open_tab(controller, None);
    let opened = inspect::latest().and_then(|(id, _)| Some((id, crate::terminal::tab_of_view(id)?)));
    let Some((id, tab)) = opened else {
        restore(&kept);
        return give_up(NAME, "a terminal opens in a tab");
    };
    let run = Run {
        id,
        tab,
        root: helpers::root(PROJECT),
        kept,
    };
    wait(
        &run.clone(),
        25,
        |view| view.prompts() > 0,
        move |started| {
            super::check("the shell of the terminal starts", started, screen(run.id));
            field::record(run);
        },
    );
}

/// Puts the settings of `SWITCHED` back as `kept` holds them.
fn restore(kept: &[(&'static str, Option<Value>)]) {
    let mut settings = Settings::load_user();
    for (name, value) in kept {
        let _ = match value {
            Some(value) => settings.set_value(name, value.clone()),
            None => settings.remove(name),
        };
    }
}

/// Ends the scenario early as the failed check `missing`, with the settings put back first.
#[track_caller]
fn stop(run: &Run, missing: &'static str) {
    restore(&run.kept);
    give_up(NAME, missing);
}

/// Ends the scenario with the settings put back.
fn end(run: &Run) {
    restore(&run.kept);
    finish(NAME);
}

fn controller_of(run: &Run) -> Option<Retained<WindowController>> {
    helpers::window_of(&run.root)
}

/// The lines of the terminal `id` that hold text, for a check's detail.
fn screen(id: u64) -> String {
    inspect::view(id)
        .map(|view| view.visible_lines())
        .unwrap_or_default()
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ⏎ ")
}

fn has_line(view: &TerminalView, line: &str) -> bool {
    view.visible_lines().iter().any(|shown| shown == line)
}

/// Calls `then` with whether `done` came true for the terminal of `run` within `seconds`.
fn wait(run: &Run, seconds: u64, done: impl Fn(&TerminalView) -> bool + 'static, then: impl FnOnce(bool) + 'static) {
    let id = run.id;
    helpers::wait_within(
        Duration::from_secs(seconds),
        move || inspect::view(id).is_some_and(|view| done(&view)),
        then,
    );
}

/// Runs `next` once the shell of `run` prompts again after `before` prompts, or after ten seconds.
fn after_prompt(run: Run, before: usize, next: fn(Run)) {
    wait(
        &run.clone(),
        10,
        move |view| view.prompts() > before,
        move |_| next(run),
    );
}

/// The item of the menu bar that sends `action` and that `wanted` picks.
fn menu_item(mtm: MainThreadMarker, action: Sel, wanted: impl Fn(&NSMenuItem) -> bool) -> Option<Retained<NSMenuItem>> {
    fn within(menu: &NSMenu, action: Sel, wanted: &dyn Fn(&NSMenuItem) -> bool) -> Option<Retained<NSMenuItem>> {
        menu.itemArray().iter().find_map(|item| {
            if item.action() == Some(action) && wanted(&item) {
                return Some(item);
            }
            item.submenu().and_then(|inner| within(&inner, action, wanted))
        })
    }
    let bar = NSApplication::sharedApplication(mtm).mainMenu()?;
    within(&bar, action, &wanted)
}

/// Chooses the terminal's command titled `title` from the menu bar for the window of `run`, as a click on the item
/// does; whether the menu bar has it.
fn command(run: &Run, title: &str) -> bool {
    let (Some(controller), Some(mtm)) = (controller_of(run), MainThreadMarker::new()) else {
        return false;
    };
    let Some(item) = menu_item(mtm, sel!(ideTerminalCommand:), |item| item.title().to_string() == title) else {
        return false;
    };
    // SAFETY: The window answers `ideTerminalCommand:`, which takes the menu item that sends it.
    let () = unsafe { msg_send![&*controller, ideTerminalCommand: &*item] };
    true
}

/// Brings the terminal of `run` to the front of its window with the keyboard in it, as a click on its tab and into
/// it does; giving the keyboard away first has the view take it anew.
fn bring_back(run: &Run) {
    let (Some(controller), Some(view)) = (controller_of(run), inspect::view(run.id)) else {
        return;
    };
    controller.select_tab_by_id(run.tab);
    if let Some(window) = view.window() {
        window.makeFirstResponder(None);
        window.makeFirstResponder(Some(&view));
    }
}

/// Sets the switch of the Terminal settings page that sends `action` to `on` as a click does, with the page open;
/// whether the page has that switch.
fn switch(action: Sel, on: bool) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    crate::settings::show_terminal(mtm);
    let Some(button) = terminal_page(mtm).and_then(|page| button_for(&page, action)) else {
        return false;
    };
    button.setState(if on {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
    let target = button.target();
    let sender: &AnyObject = &button;
    // SAFETY: The switch's own action and target, with the switch as the sender, as a click sends them.
    unsafe { NSApplication::sharedApplication(mtm).sendAction_to_from(action, target.as_deref(), Some(sender)) }
}

/// The view of the settings window's Terminal tab.
fn terminal_page(mtm: MainThreadMarker) -> Option<Retained<NSView>> {
    let window = crate::settings::window()?;
    let tabs = window.contentViewController()?.downcast::<NSTabViewController>().ok()?;
    let index = usize::try_from(crate::settings::TERMINAL_TAB).ok()?;
    let page = tabs.tabViewItems().iter().nth(index)?.viewController(mtm)?;
    Some(page.view())
}

fn button_for(view: &NSView, action: Sel) -> Option<Retained<NSButton>> {
    if let Some(button) = view.downcast_ref::<NSButton>()
        && button.action() == Some(action)
    {
        return Some(button.retain());
    }
    view.subviews().iter().find_map(|child| button_for(&child, action))
}

/// The first view at or below `view` that `wanted` picks.
fn find_view(view: &NSView, wanted: &dyn Fn(&NSView) -> bool) -> Option<Retained<NSView>> {
    if wanted(view) {
        return Some(view.retain());
    }
    view.subviews().iter().find_map(|child| find_view(&child, wanted))
}

/// The pane of `view`: the stack of its find bar, notice, the view itself and its input field.
fn pane_of(view: &TerminalView) -> Option<Retained<NSView>> {
    // SAFETY: Asking a view for its superview on the main thread, where views live.
    unsafe { view.superview() }
}

/// Presses the key `code` that types `characters` on `target`, through `keyDown:` as AppKit delivers it.
fn press_on(target: &NSView, code: u16, characters: &str) {
    let window = target.window().map_or(0, |window| window.windowNumber());
    let characters = NSString::from_str(characters);
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::ZERO,
        NSEventModifierFlags::empty(),
        0.0,
        window,
        None,
        &characters,
        &characters,
        false,
        code,
    );
    if let Some(event) = event {
        // SAFETY: `keyDown:` takes the event, which lives for the call.
        let () = unsafe { msg_send![target, keyDown: &*event] };
    }
}

/// The text of the run's pasteboard of `kind`.
fn pasteboard_text(kind: &NSPasteboardType) -> Option<String> {
    NSPasteboard::generalPasteboard()
        .stringForType(kind)
        .map(|text| text.to_string())
}

/// The plain text on the run's pasteboard.
fn pasted_text() -> Option<String> {
    pasteboard_text(constant!(NSPasteboardTypeString))
}

/// ⌃U, which empties the line typed at the shell's prompt.
fn empty_line(view: &TerminalView) {
    view.press(0x20, "\u{15}", "u", NSEventModifierFlags::Control);
}
