//! The terminal's shortcuts (E26): a key monitor runs the terminal command whose shortcut a key press is before the
//! window and the menu bar see it, while the keyboard focus is in a terminal's pane; any other key press goes on as it
//! came, so ⌘I, ⌘↑ or ⌘F keep their meaning in the editor.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSEventMask, NSWindow};
use term_config::Chord;

use super::commands::{self, Command};
use crate::context::AppContext;
use crate::util::guarded;

/// Starts the key monitor of the app with `cx`; once.
pub(crate) fn watch(cx: &AppContext) {
    let back = cx.leitstand().back();
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        guarded(event.as_ptr(), || {
            // SAFETY: AppKit passes a live event for the call.
            let pressed = unsafe { event.as_ref() };
            let window = MainThreadMarker::new().and_then(|mtm| pressed.window(mtm));
            let taken = window.is_some_and(|window| back.with(|cx| run_shortcut(cx, &window, pressed)) == Some(true));
            if taken { std::ptr::null_mut() } else { event.as_ptr() }
        })
    });
    // SAFETY: The block returns the event it was given, or none for a key press it ran a command for.
    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block) };
    cx.terminals().keep_monitor(monitor);
}

/// Runs the terminal command whose shortcut `event` is, if the keyboard focus of `window` is in a terminal's pane;
/// whether it did. The self-test's key presses go to the window past the monitor, so it reaches the shortcuts here.
pub(crate) fn run_shortcut(cx: &AppContext, window: &NSWindow, event: &NSEvent) -> bool {
    let Some(chord) = term_ui::chord_of(event) else {
        return false;
    };
    let named = cx
        .keymap()
        .borrow()
        .terminal_keys()
        .find(|(_, key)| is_chord(key, &chord))
        .and_then(|(name, _)| Command::named(name));
    let Some(command) = named else { return false };
    let Some(view) = commands::focused_in(window) else {
        commands::hide_shortcuts(cx, MainThreadMarker::from(window));
        return false;
    };
    let Some(controller) = cx.windows().get(view.window_id().get()) else {
        return false;
    };
    commands::run(&controller, &view, command);
    true
}

/// Whether the shortcut `key`, written like `shift+cmd+up`, is `chord` as Tapa reads a key press.
fn is_chord(key: &str, chord: &Chord) -> bool {
    let Some((modifiers, name)) = ide_core::keymap::split(key) else {
        return false;
    };
    let has = |modifier: &str| modifiers.contains(&modifier);
    let name = if name == "return" { "enter" } else { name };
    (has("ctrl"), has("alt"), has("shift"), has("cmd")) == (chord.ctrl, chord.alt, chord.shift, chord.cmd)
        && name.chars().flat_map(char::to_lowercase).eq(chord.key.chars())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> Chord {
        text.parse().unwrap()
    }

    #[test]
    fn a_shortcut_of_the_table_is_the_chord_of_its_key_press() {
        assert!(is_chord("cmd+up", &chord("cmd+up")));
        assert!(is_chord("shift+cmd+g", &chord("cmd+shift+g")));
        assert!(is_chord("shift+cmd+return", &chord("shift+cmd+enter")));
        assert!(is_chord("alt+shift+cmd+c", &chord("shift+alt+cmd+c")));
        assert!(!is_chord("cmd+i", &chord("shift+cmd+i")));
        assert!(!is_chord("cmd+up", &chord("cmd+down")));
        assert!(!is_chord("hyper+k", &chord("cmd+k")));
    }
}
