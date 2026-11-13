//! The `terminal-view` scenario: what a terminal view does beyond typing and drawing. A title, a bell and a copy a
//! program sends; the context menu and its commands; VoiceOver's text ranges; output while the view is hidden; a
//! program that takes the mouse and the wheel; the history under the wheel; word selection; drops; links; the dev
//! server banner's buttons; and a terminal opened beside.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadOnly, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSPasteboard, NSPasteboardTypePNG,
    NSPasteboardTypeString, NSPasteboardWriting, NSScreen,
};
use objc2_core_foundation::CGPoint;
use objc2_core_graphics::{CGEvent, CGEventField, CGEventType, CGMouseButton, CGScrollEventUnit};
use objc2_foundation::{NSArray, NSData, NSPoint, NSRange, NSRect, NSString, NSURL};
use splat_i18n::t;

use super::{check, finish, helpers};
use crate::terminal::{TerminalView, inspect};
use crate::util::constant;
use crate::window::WindowController;

const NAME: &str = "terminal-view";
const PROJECT: &str = "terminal-view-project";
/// A program that asks for every button press, release and drag, and shows what it gets until ⌃C.
const MOUSE_SCRIPT: &str =
    "trap 'printf \"\\033[?1002l\"; exit 0' INT\nprintf '\\033[?1002h'\necho tv-mouse-ready\ncat -v\n";
/// "tv-copied" in base64, as a program sends it to the pasteboard.
const COPIED: &str = "tv-copied";

type Next = fn(u64, PathBuf);

pub fn run() {
    let _ = std::fs::write(ide_core::paths::home().join("tv-note.txt"), "a note\n");
    helpers::open_project(
        NAME,
        PROJECT,
        &[("README.md", "# Terminal view\n"), ("mouse.sh", MOUSE_SCRIPT)],
        opened,
    );
}

fn opened(controller: Retained<WindowController>) {
    let root = helpers::root(PROJECT);
    crate::terminal::open_tab(&controller, None);
    let Some((id, _)) = inspect::latest() else {
        check("a terminal opens", false, "");
        return finish(NAME);
    };
    wait(
        id,
        25,
        |view| view.prompts() > 0,
        move |started| {
            check("the shell of the terminal starts", started, screen(id));
            program_reports(id, root);
        },
    );
}

fn controller_of(root: &Path) -> Option<Retained<WindowController>> {
    helpers::window_of(root)
}

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

/// Calls `then` with whether `done` came true for the view `id` within `seconds`.
fn wait(id: u64, seconds: u64, done: impl Fn(&TerminalView) -> bool + 'static, then: impl FnOnce(bool) + 'static) {
    helpers::wait_within(
        Duration::from_secs(seconds),
        move || inspect::view(id).is_some_and(|view| done(&view)),
        then,
    );
}

/// Runs `next` once the shell is back at a prompt after `before` prompts.
fn after_prompt(id: u64, root: PathBuf, before: usize, next: Next) {
    wait(id, 10, move |view| view.prompts() > before, move |_| next(id, root));
}

fn general_text() -> Option<String> {
    // SAFETY: The string type is AppKit's constant.
    unsafe { NSPasteboard::generalPasteboard().stringForType(NSPasteboardTypeString) }.map(|text| text.to_string())
}

/// Sends the action `action` to `view` as a menu item would.
fn perform(view: &TerminalView, action: Sel) -> bool {
    let app = NSApplication::sharedApplication(view.mtm());
    let target: &AnyObject = view;
    // SAFETY: Every action the view takes accepts any sender, here none.
    unsafe { app.sendAction_to_from(action, Some(target), None) }
}

/// A title, a bell and text for the pasteboard, all sent by a program as escape sequences.
fn program_reports(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    let before = view.prompts();
    view.type_text("printf '\\033]0;tv-title\\007\\a\\033]52;c;dHYtY29waWVk\\007'; sleep 2\r");
    let window = root.clone();
    let titled = move || {
        controller_of(&window).is_some_and(|c| c.tab_titles().iter().any(|title| title == "tv-title"))
            && general_text().as_deref() == Some(COPIED)
    };
    helpers::wait_within(Duration::from_secs(10), titled, move |titled| {
        let titles = controller_of(&root).map(|c| c.tab_titles()).unwrap_or_default();
        check(
            "a title the program sets names the terminal's tab",
            titles.iter().any(|title| title == "tv-title"),
            titles.join(" | "),
        );
        check(
            "text a program copies lands on the pasteboard",
            titled,
            format!("{:?}", general_text()),
        );
        let rung = inspect::view(id).is_some_and(|view| view.has_rung());
        check("the program's bell rings in the view", rung, "");
        after_prompt(id, root, before, menu_and_paste);
    });
}

/// The titles of the context menu and whether each is enabled.
fn menu_state(view: &TerminalView) -> Vec<(String, bool)> {
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
    );
    let Some(event) = event else { return Vec::new() };
    // SAFETY: `menuForEvent:` takes the event, which lives for the call, and returns a menu or nil.
    let menu: Option<Retained<NSMenu>> = unsafe { msg_send![view, menuForEvent: &*event] };
    let Some(menu) = menu else { return Vec::new() };
    menu.itemArray()
        .iter()
        .filter(|item| !item.isSeparatorItem())
        .map(|item| {
            // SAFETY: `validateMenuItem:` takes the item, which the menu holds for the call.
            let enabled: bool = unsafe { msg_send![view, validateMenuItem: &*item] };
            (item.title().to_string(), enabled)
        })
        .collect()
}

fn menu_and_paste(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    let items = menu_state(&view);
    let expected = [
        (t("Kopieren"), false),
        (t("Einsetzen"), true),
        (t("Alles auswählen"), true),
        (t("Terminal leeren"), true),
        (t("Neues Terminal hier"), true),
    ];
    check(
        "the context menu offers the terminal's commands, Copy only with a selection",
        items.iter().map(|(title, on)| (title.as_str(), *on)).eq(expected),
        format!("{items:?}"),
    );
    let before = view.prompts();
    view.type_text("echo ");
    perform(&view, sel!(paste:));
    view.type_text("\r");
    wait(
        id,
        10,
        |view| has_line(view, COPIED),
        move |pasted| {
            check("Paste types the pasteboard's text into the shell", pasted, screen(id));
            after_prompt(id, root, before, accessibility);
        },
    );
}

/// What VoiceOver asks a text area: lines, ranges, the insertion point and the frame of a line.
fn accessibility(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    let line = view.visible_lines().iter().position(|line| line == COPIED);
    let index = line.and_then(|line| isize::try_from(line).ok()).unwrap_or(-1);
    // SAFETY: Each accessibility getter takes plain values and returns a value, a range, a string or nil.
    let (count, visible, range, text, back, cursor, insertion, frame) = unsafe {
        let count: isize = msg_send![&*view, accessibilityNumberOfCharacters];
        let visible: NSRange = msg_send![&*view, accessibilityVisibleCharacterRange];
        let range: NSRange = msg_send![&*view, accessibilityRangeForLine: index];
        let text: Option<Retained<NSString>> = msg_send![&*view, accessibilityStringForRange: range];
        let location = isize::try_from(range.location).unwrap_or(0);
        let back: isize = msg_send![&*view, accessibilityLineForIndex: location];
        let cursor: NSRange = msg_send![&*view, accessibilitySelectedTextRange];
        let at = isize::try_from(cursor.location).unwrap_or(0);
        let cursor_line: isize = msg_send![&*view, accessibilityLineForIndex: at];
        let insertion: isize = msg_send![&*view, accessibilityInsertionPointLineNumber];
        let frame: NSRect = msg_send![&*view, accessibilityFrameForRange: range];
        (count, visible, range, text, back, cursor_line, insertion, frame)
    };
    let text = text.map(|text| text.to_string()).unwrap_or_default();
    let on_screen = view.window().map(|window| window.frame()).unwrap_or(NSRect::ZERO);
    let inside = frame.size.height > 0.0
        && frame.origin.y >= on_screen.origin.y
        && frame.origin.y + frame.size.height <= on_screen.origin.y + on_screen.size.height;
    check(
        "VoiceOver finds a line of output by its range, its frame and the insertion point's line",
        index >= 0
            && usize::try_from(count).ok() == Some(visible.length)
            && text == format!("{COPIED}\n")
            && back == index
            && insertion == cursor
            && insertion > index
            && inside,
        format!(
            "line {index}, {count} = {visible:?}, {range:?} {text:?} → {back}, cursor {cursor}/{insertion}, {frame:?}"
        ),
    );
    select_copy_clear(id, root);
}

fn select_copy_clear(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    perform(&view, sel!(selectAll:));
    let selected = view.selection().unwrap_or_default();
    let copy_on = menu_state(&view).first().is_some_and(|(_, on)| *on);
    // SAFETY: The getter takes nothing and returns a string or nil.
    let told: Option<Retained<NSString>> = unsafe { msg_send![&*view, accessibilitySelectedText] };
    perform(&view, sel!(copy:));
    check(
        "Select All takes the whole screen, which Copy puts on the pasteboard and VoiceOver reads",
        selected.contains(COPIED)
            && copy_on
            && general_text().as_deref() == Some(selected.as_str())
            && told.is_some_and(|told| told.to_string() == selected),
        format!("{selected:?}"),
    );
    perform(&view, sel!(ideClearTerminal:));
    wait(
        id,
        5,
        |view| !has_line(view, COPIED),
        move |cleared| {
            check("Clear empties the screen", cleared, screen(id));
            hidden_output(id, root);
        },
    );
}

/// The lines the view drew last.
fn drawn_has(view: &TerminalView, line: &str) -> bool {
    view.drawn_lines().iter().any(|drawn| drawn == line)
}

/// Output that arrives while the view is hidden waits for it to show; shown again it takes the focus nobody has.
fn hidden_output(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    view.setHidden(true);
    view.type_text("echo tv-hidden\r");
    wait(
        id,
        10,
        |view| has_line(view, "tv-hidden"),
        move |arrived| {
            let Some(view) = inspect::view(id) else {
                return finish(NAME);
            };
            let drawn_hidden = drawn_has(&view, "tv-hidden");
            if let Some(window) = view.window() {
                window.makeFirstResponder(None);
            }
            view.setHidden(false);
            let focused = view
                .window()
                .and_then(|window| window.firstResponder())
                .is_some_and(|responder| {
                    let responder: &AnyObject = &responder;
                    let view: &AnyObject = &view;
                    std::ptr::eq(responder, view)
                });
            check(
                "output that arrives while the terminal is hidden is drawn once it shows, and it takes the focus back",
                arrived && !drawn_hidden && drawn_has(&view, "tv-hidden") && focused,
                format!("arrived {arrived}, drawn while hidden {drawn_hidden}, focused {focused}"),
            );
            text_commands(id, root);
        },
    );
}

/// A command from the text system outside a key press, a composition given up, and a key let go.
fn text_commands(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    let before = view.prompts();
    view.type_text("echo tv-command");
    // SAFETY: `doCommandBySelector:` takes a selector the text system sends.
    let () = unsafe { msg_send![&*view, doCommandBySelector: sel!(insertNewline:)] };
    view.compose("ka", 2);
    // SAFETY: The text input getters and `unmarkText` take nothing.
    let (composing, given_up) = unsafe {
        let composing: NSRange = msg_send![&*view, selectedRange];
        let () = msg_send![&*view, unmarkText];
        let given_up: NSRange = msg_send![&*view, selectedRange];
        (composing, given_up)
    };
    release_key(&view, 0x00, "a");
    wait(
        id,
        5,
        |view| has_line(view, "tv-command"),
        move |ran| {
            let left = inspect::view(id).and_then(|view| view.composing());
            check(
                "insertNewline: from the text system runs the line, and a composition given up leaves nothing",
                ran && composing.location == 2
                    && usize::try_from(objc2_foundation::NSNotFound).ok() == Some(given_up.location)
                    && left.is_none(),
                format!("{composing:?}, then {given_up:?}, {left:?} | {}", screen(id)),
            );
            after_prompt(id, root, before, mouse_program);
        },
    );
}

fn release_key(view: &TerminalView, code: u16, characters: &str) {
    let window = view.window().map_or(0, |window| window.windowNumber());
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyUp,
        NSPoint::ZERO,
        NSEventModifierFlags::empty(),
        0.0,
        window,
        None,
        &NSString::from_str(characters),
        &NSString::from_str(characters),
        false,
        code,
    );
    if let Some(event) = event {
        // SAFETY: `keyUp:` takes the event, which lives for the call.
        let () = unsafe { msg_send![view, keyUp: &*event] };
    }
}

/// A mouse event at `at` in the view's coordinates from the window server's kind of event, for buttons and the wheel
/// AppKit's own constructors do not make.
fn window_server_event(view: &TerminalView, event: &CGEvent, at: NSPoint) -> Option<Retained<NSEvent>> {
    let window = view.window()?;
    let on_screen = window.convertPointToScreen(view.convertPoint_toView(at, None));
    let top = NSScreen::screens(view.mtm())
        .firstObject()
        .map_or(0.0, |screen| screen.frame().size.height);
    CGEvent::set_location(Some(event), CGPoint::new(on_screen.x, top - on_screen.y));
    let number = i64::try_from(window.windowNumber()).unwrap_or(0);
    CGEvent::set_integer_value_field(Some(event), CGEventField::MouseEventWindowUnderMousePointer, number);
    CGEvent::set_integer_value_field(
        Some(event),
        CGEventField::MouseEventWindowUnderMousePointerThatCanHandleThisEvent,
        number,
    );
    NSEvent::eventWithCGEvent(event)
}

/// Presses and lets go of mouse button `number` (1 right, 2 middle, 3 back) at `at`.
fn press_button(view: &TerminalView, number: i64, at: NSPoint) {
    let (down, up, button) = if number == 1 {
        (
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGMouseButton::Right,
        )
    } else {
        (
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGMouseButton::Center,
        )
    };
    for kind in [down, up] {
        let Some(cg) = CGEvent::new_mouse_event(None, kind, CGPoint::new(0.0, 0.0), button) else {
            continue;
        };
        CGEvent::set_integer_value_field(Some(&cg), CGEventField::MouseEventButtonNumber, number);
        let Some(event) = window_server_event(view, &cg, at) else {
            continue;
        };
        // SAFETY: Each of these takes the event, which lives for the call.
        let () = unsafe {
            match (number, kind == down) {
                (1, true) => msg_send![view, rightMouseDown: &*event],
                (1, false) => msg_send![view, rightMouseUp: &*event],
                (_, true) => msg_send![view, otherMouseDown: &*event],
                (_, false) => msg_send![view, otherMouseUp: &*event],
            }
        };
    }
}

/// Turns the wheel `lines` lines over `at`, with `flags` held.
fn wheel(view: &TerminalView, lines: i32, at: NSPoint, flags: NSEventModifierFlags) {
    let Some(cg) = CGEvent::new_scroll_wheel_event2(None, CGScrollEventUnit::Line, 1, lines, 0, 0) else {
        return;
    };
    CGEvent::set_flags(
        Some(&cg),
        objc2_core_graphics::CGEventFlags(u64::try_from(flags.bits()).unwrap_or(0)),
    );
    let Some(event) = window_server_event(view, &cg, at) else {
        return;
    };
    // SAFETY: `scrollWheel:` takes the event, which lives for the call.
    let () = unsafe { msg_send![view, scrollWheel: &*event] };
}

fn mouse_program(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    view.type_text("sh mouse.sh\r");
    wait(
        id,
        10,
        |view| has_line(view, "tv-mouse-ready"),
        move |ready| {
            let Some(view) = inspect::view(id) else {
                return finish(NAME);
            };
            let at = |line, column| view.cell_center(ide_terminal::Point { line, column });
            press_button(&view, 1, at(1, 3));
            press_button(&view, 2, at(1, 4));
            press_button(&view, 3, at(1, 5));
            let none = NSEventModifierFlags::empty();
            view.mouse(NSEventType::LeftMouseDown, at(2, 2), none);
            view.mouse(NSEventType::LeftMouseDragged, at(2, 6), none);
            view.mouse(NSEventType::LeftMouseUp, at(2, 6), none);
            wheel(&view, 1, at(2, 2), none);
            // The right and middle buttons' presses and releases, the left one's press, drag and release and a turn of the
            // wheel, each echoed as the bytes of a report; the back button's reports come on top where the encoding has room.
            let reported = |view: &TerminalView| view.visible_lines().concat().matches("^[[M").count() >= 8;
            wait(id, 5, reported, move |reported| {
                check(
                    "a program that takes the mouse gets every button, the drag and the wheel",
                    ready && reported,
                    screen(id),
                );
                let Some(view) = inspect::view(id) else {
                    return finish(NAME);
                };
                let before = view.prompts();
                view.press(0x08, "\u{3}", "c", NSEventModifierFlags::Control);
                after_prompt(id, root, before, history_wheel);
            });
        },
    );
}

/// The wheel scrolls the history, ⌥ with it too, and typing goes back to the end.
fn history_wheel(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    view.type_text("seq 1 200\r");
    wait(
        id,
        10,
        |view| has_line(view, "200"),
        move |_| {
            let Some(view) = inspect::view(id) else {
                return finish(NAME);
            };
            let at = view.cell_center(ide_terminal::Point { line: 3, column: 3 });
            wheel(&view, 6, at, NSEventModifierFlags::empty());
            let up = view.scrolled_back();
            wheel(&view, -2, at, NSEventModifierFlags::Option);
            let option = view.scrolled_back();
            view.type_text("x");
            wait(
                id,
                5,
                |view| view.scrolled_back() == 0,
                move |typed| {
                    let Some(view) = inspect::view(id) else {
                        return finish(NAME);
                    };
                    wheel(&view, 6, at, NSEventModifierFlags::empty());
                    let again = view.scrolled_back();
                    view.commit("y");
                    wait(
                        id,
                        5,
                        |view| view.scrolled_back() == 0,
                        move |committed| {
                            if let Some(view) = inspect::view(id) {
                                for _ in 0..2 {
                                    view.press(0x33, "\u{7f}", "\u{7f}", NSEventModifierFlags::empty());
                                }
                            }
                            check(
                                "the wheel scrolls into the history, with ⌥ as well, and a key or text from an input \
                                 method brings the end back",
                                up > 0 && option < up && typed && again > 0 && committed,
                                format!("{up}, then {option}, back {typed}, again {again}, back {committed}"),
                            );
                            select_words(id, root);
                        },
                    );
                },
            );
        },
    );
}

/// A mouse click `count` times at `at` with `flags`.
fn click(view: &TerminalView, at: NSPoint, flags: NSEventModifierFlags, count: isize) {
    let Some(window) = view.window() else { return };
    for kind in [NSEventType::LeftMouseDown, NSEventType::LeftMouseUp] {
        let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind,
            view.convertPoint_toView(at, None),
            flags,
            0.0,
            window.windowNumber(),
            None,
            0,
            count,
            1.0,
        );
        let Some(event) = event else { return };
        // SAFETY: Each of these takes the event, which lives for the call.
        let () = unsafe {
            if kind == NSEventType::LeftMouseDown {
                msg_send![view, mouseDown: &*event]
            } else {
                msg_send![view, mouseUp: &*event]
            }
        };
    }
}

/// A double click takes a word, a ⇧-click carries the selection on to where it lands.
fn select_words(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    view.type_text("echo tv-word other-word\r");
    wait(
        id,
        5,
        |view| has_line(view, "tv-word other-word"),
        move |_| {
            let Some(view) = inspect::view(id) else {
                return finish(NAME);
            };
            let line = view
                .visible_lines()
                .iter()
                .position(|line| line == "tv-word other-word");
            let seen = line.map(|line| {
                let at = |column| view.cell_center(ide_terminal::Point { line, column });
                click(&view, at(2), NSEventModifierFlags::empty(), 2);
                let word = view.selection();
                click(&view, at(12), NSEventModifierFlags::Shift, 1);
                (word, view.selection())
            });
            check(
                "a double click selects a word and a ⇧-click extends the selection",
                seen.as_ref().is_some_and(|(word, extended)| {
                    word.as_deref() == Some("tv-word")
                        && extended.as_deref().is_some_and(|e| e.starts_with("tv-word oth"))
                }),
                format!("{seen:?}"),
            );
            drops(id, root);
        },
    );
}

fn drops(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    let pasteboard = NSPasteboard::pasteboardWithUniqueName();
    pasteboard.clearContents();
    let empty = view.drop_pasteboard(&pasteboard);
    pasteboard.setData_forType(
        Some(&NSData::with_bytes(&super::terminal::PICTURE)),
        constant!(NSPasteboardTypePNG),
    );
    let picture = view.drop_pasteboard(&pasteboard);
    pasteboard.clearContents();
    let file = NSURL::fileURLWithPath(&NSString::from_str(&root.join("README.md").to_string_lossy()));
    let objects: Retained<NSArray<objc2::runtime::ProtocolObject<dyn NSPasteboardWriting>>> =
        NSArray::from_retained_slice(&[objc2::runtime::ProtocolObject::from_retained(file)]);
    pasteboard.writeObjects(&objects);
    let dropped_file = view.drop_pasteboard(&pasteboard);
    pasteboard.clearContents();
    pasteboard.setString_forType(&NSString::from_str(" tv-dropped"), constant!(NSPasteboardTypeString));
    let dropped_text = view.drop_pasteboard(&pasteboard);
    // SAFETY: `releaseGlobally` takes nothing; objc2 leaves it out because of its name.
    let () = unsafe { msg_send![&*pasteboard, releaseGlobally] };
    let shown = |view: &TerminalView| {
        view.visible_lines()
            .iter()
            .any(|line| line.contains("README.md") && line.contains("tv-dropped"))
    };
    wait(id, 5, shown, move |shown| {
        check(
            "a dropped file goes in as its path and dropped text as itself; a picture is welcome, nothing is not",
            shown
                && empty == (false, false)
                && picture == (true, false)
                && dropped_file == (true, true)
                && dropped_text == (true, true),
            format!(
                "{empty:?} {picture:?} {dropped_file:?} {dropped_text:?} | {}",
                screen(id)
            ),
        );
        if let Some(view) = inspect::view(id) {
            view.press(0x20, "\u{15}", "u", NSEventModifierFlags::Control);
        }
        links(id, root);
    });
}

/// ⌘-click opens a web address in a browser tab and a file under the home folder in an editor.
fn links(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    let printed = "http://127.0.0.1:9/tv-link ~/tv-note.txt";
    view.type_text(&format!("echo '{printed}'\r"));
    wait(
        id,
        5,
        move |view| has_line(view, printed),
        move |_| {
            let (Some(view), Some(controller)) = (inspect::view(id), controller_of(&root)) else {
                return finish(NAME);
            };
            let line = view
                .visible_lines()
                .iter()
                .position(|line| line == printed)
                .unwrap_or(0);
            let at = |column| view.cell_center(ide_terminal::Point { line, column });
            let command = NSEventModifierFlags::Command;
            let tabs = controller.tab_ids();
            view.mouse(NSEventType::LeftMouseDown, at(8), command);
            view.mouse(NSEventType::LeftMouseUp, at(8), command);
            let browsed = ide_ui_browser::inspect(view.window_id()).iter().any(|tab| {
                tab.address.contains("tv-link") || tab.url.as_deref().is_some_and(|url| url.contains("tv-link"))
            });
            view.mouse(NSEventType::LeftMouseDown, at(30), command);
            view.mouse(NSEventType::LeftMouseUp, at(30), command);
            let titles = controller.tab_titles();
            check(
                "⌘-click opens a web address in a browser tab and ~/file in an editor",
                browsed && titles.iter().any(|title| title == "tv-note.txt"),
                titles.join(" | "),
            );
            for tab in controller.tab_ids().into_iter().filter(|tab| !tabs.contains(tab)) {
                controller.close_tab(tab);
            }
            dev_server(id, root);
        },
    );
}

/// The banner's close button hides it; its button opens the address in a browser tab.
fn dev_server(id: u64, root: PathBuf) {
    let Some(view) = inspect::view(id) else {
        return finish(NAME);
    };
    view.type_text("printf '  ➜  Local:   http://127.0.0.1:41001/\\n'\r");
    wait(
        id,
        5,
        |view| view.dev_server_banner().is_some(),
        move |raised| {
            let Some(view) = inspect::view(id) else {
                return finish(NAME);
            };
            perform(&view, sel!(ideDismissDevServer:));
            let dismissed = view.dev_server_banner().is_none();
            view.type_text("printf '  ➜  Local:   http://127.0.0.1:41002/\\n'\r");
            wait(
                id,
                5,
                |view| view.dev_server_banner().is_some_and(|b| b.contains("41002")),
                move |again| {
                    let Some(view) = inspect::view(id) else {
                        return finish(NAME);
                    };
                    perform(&view, sel!(ideOpenDevServer:));
                    let opened = ide_ui_browser::inspect(view.window_id()).iter().any(|tab| {
                        tab.address.contains("41002") || tab.url.as_deref().is_some_and(|url| url.contains("41002"))
                    });
                    check(
                        "the dev server banner closes, and its button opens the address in a browser tab and closes it too",
                        raised && dismissed && again && opened && view.dev_server_banner().is_none(),
                        format!("raised {raised}, dismissed {dismissed}, again {again}, opened {opened}"),
                    );
                    beside(id, root);
                },
            );
        },
    );
}

/// "Neues Terminal hier" opens a terminal in the same folder.
fn beside(id: u64, root: PathBuf) {
    let (Some(view), Some(controller)) = (inspect::view(id), controller_of(&root)) else {
        return finish(NAME);
    };
    let tabs = controller.tab_count();
    perform(&view, sel!(ideNewTerminalHere:));
    let opened = inspect::latest().filter(|(other, _)| *other != id);
    check(
        "\"Neues Terminal hier\" opens a terminal tab in the terminal's folder",
        controller.tab_count() == tabs + 1 && opened.as_ref().is_some_and(|(_, other)| other.cwd() == view.cwd()),
        format!("{tabs} then {} tabs", controller.tab_count()),
    );
    finish(NAME);
}
