//! Keys, clicks and scroll-wheel events as the hardware sends them, posted only while the measured process is in
//! front; GPUI does not take keys posted to its process directly.

use objc2_core_foundation::{CFRetained, CGPoint};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation, CGEventType,
    CGMouseButton, CGScrollEventUnit, CGScrollPhase, CGWarpMouseCursorPosition,
};

use crate::compare::apps;
use crate::compare::clock;
use crate::compare::keys::{self, Layout, Stroke};
use crate::compare::tree;

/// Modifiers of a shortcut.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Mods {
    pub(crate) cmd: bool,
    pub(crate) shift: bool,
    pub(crate) option: bool,
    pub(crate) control: bool,
}

pub(crate) const CMD: Mods = Mods {
    cmd: true,
    shift: false,
    option: false,
    control: false,
};
pub(crate) const CTRL: Mods = Mods {
    cmd: false,
    shift: false,
    option: false,
    control: true,
};
pub(crate) const CTRL_CMD: Mods = Mods {
    cmd: true,
    shift: false,
    option: false,
    control: true,
};

/// The virtual key codes of the modifier keys, the same on every layout.
const MOD_COMMAND: u16 = 55;
const MOD_SHIFT: u16 = 56;
const MOD_OPTION: u16 = 58;
const MOD_CONTROL: u16 = 59;

impl Mods {
    fn with(self, other: Mods) -> Mods {
        Mods {
            cmd: self.cmd || other.cmd,
            shift: self.shift || other.shift,
            option: self.option || other.option,
            control: self.control || other.control,
        }
    }

    fn flags(self) -> CGEventFlags {
        let mut flags = CGEventFlags::empty();
        for (on, flag) in [
            (self.cmd, CGEventFlags::MaskCommand),
            (self.shift, CGEventFlags::MaskShift),
            (self.option, CGEventFlags::MaskAlternate),
            (self.control, CGEventFlags::MaskControl),
        ] {
            if on {
                flags |= flag;
            }
        }
        flags
    }
}

/// Where a scroll is in its gesture, as a trackpad reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Began,
    Changed,
    Ended,
}

fn phase_value(phase: Phase) -> i64 {
    let phase = match phase {
        Phase::Began => CGScrollPhase::Began,
        Phase::Changed => CGScrollPhase::Changed,
        Phase::Ended => CGScrollPhase::Ended,
    };
    i64::from(phase.0)
}

/// Events for the process `pid`, with keys of the current layout.
pub(crate) struct Input {
    pid: libc::pid_t,
    layout: Layout,
    source: CFRetained<CGEventSource>,
    /// Whether `undo` deletes the typed character instead, once the warm-up found that ⌘Z does not take it back.
    backspace: std::cell::Cell<bool>,
}

impl Input {
    pub(crate) fn new(pid: libc::pid_t, layout: Layout) -> Result<Input, String> {
        // A private source makes events without the keyboard type and characters AppKit's text input needs.
        let source =
            CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok_or("CoreGraphics made no event source")?;
        Ok(Input {
            pid,
            layout,
            source,
            backspace: std::cell::Cell::new(false),
        })
    }

    /// Posts `event` into the system's event stream as the hardware would, but only while the measured process has
    /// the keyboard, so nothing reaches another app; returns the host time right before posting.
    fn post(&self, event: &CGEvent) -> Result<u64, String> {
        if apps::frontmost() != Some(self.pid) {
            apps::activate(self.pid);
            std::thread::sleep(std::time::Duration::from_millis(300));
            let front = apps::frontmost();
            if front != Some(self.pid) {
                return Err(format!(
                    "the front process is {front:?}, not the measured {}; no event was posted",
                    self.pid
                ));
            }
        }
        let at = clock::now_ns();
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(event));
        Ok(at)
    }

    pub(crate) fn pid(&self) -> libc::pid_t {
        self.pid
    }

    /// Presses and releases `code` with `mods`; returns the host time in nanoseconds right before the press.
    pub(crate) fn key(&self, code: u16, mods: Mods) -> Result<u64, String> {
        self.key_typing(code, mods, None)
    }

    /// A key event of `code` with `mods` that carries `typed` as its characters, as the hardware's events do.
    fn event(&self, code: u16, down: bool, mods: Mods, typed: Option<char>) -> Result<CFRetained<CGEvent>, String> {
        let event = CGEvent::new_keyboard_event(Some(&self.source), code, down).ok_or("no key event")?;
        CGEvent::set_flags(Some(&event), mods.flags());
        CGEvent::set_integer_value_field(
            Some(&event),
            CGEventField::KeyboardEventKeyboardType,
            i64::from(self.layout.keyboard),
        );
        if let Some(c) = typed {
            let mut units = [0u16; 2];
            let units = c.encode_utf16(&mut units);
            // SAFETY: `units` holds the given number of UTF-16 units for the duration of the call.
            unsafe {
                CGEvent::keyboard_set_unicode_string(Some(&event), units.len().try_into().unwrap_or(0), units.as_ptr())
            };
        }
        Ok(event)
    }

    /// Presses the modifier keys of `mods`, then `code`, then releases them in reverse, so the system's modifier
    /// state is clean afterwards and no later click turns into a control-click.
    fn key_typing(&self, code: u16, mods: Mods, typed: Option<char>) -> Result<u64, String> {
        let held: Vec<(u16, Mods)> = [
            (
                mods.control,
                MOD_CONTROL,
                Mods {
                    control: true,
                    ..Mods::default()
                },
            ),
            (
                mods.option,
                MOD_OPTION,
                Mods {
                    option: true,
                    ..Mods::default()
                },
            ),
            (
                mods.shift,
                MOD_SHIFT,
                Mods {
                    shift: true,
                    ..Mods::default()
                },
            ),
            (
                mods.cmd,
                MOD_COMMAND,
                Mods {
                    cmd: true,
                    ..Mods::default()
                },
            ),
        ]
        .into_iter()
        .filter(|(on, _, _)| *on)
        .scan(Mods::default(), |so_far, (_, key, one)| {
            *so_far = so_far.with(one);
            Some((key, *so_far))
        })
        .collect();
        for (key, so_far) in &held {
            let event = self.event(*key, true, *so_far, None)?;
            self.post(&event)?;
        }
        let down = self.event(code, true, mods, typed)?;
        let up = self.event(code, false, mods, typed)?;
        let at = self.post(&down);
        let released = self.post(&up);
        for index in (0..held.len()).rev() {
            let rest = if index == 0 { Mods::default() } else { held[index - 1].1 };
            let event = self.event(held[index].0, false, rest, None)?;
            self.post(&event)?;
        }
        released?;
        at
    }

    fn stroke(&self, c: char) -> Result<Stroke, String> {
        self.layout.stroke(c)
    }

    /// Types `c`; the time right before its key went down.
    pub(crate) fn char(&self, c: char) -> Result<u64, String> {
        let stroke = self.stroke(c)?;
        let mods = Mods {
            shift: stroke.shift,
            option: stroke.option,
            ..Mods::default()
        };
        self.key_typing(stroke.code, mods, Some(c))
    }

    /// The shortcut of `mods` and the key that types `c` unmodified, such as ⌘Z.
    pub(crate) fn shortcut(&self, mods: Mods, c: char) -> Result<u64, String> {
        let stroke = self.stroke(c)?;
        if stroke.shift || stroke.option {
            return Err(format!("{c:?} needs a modifier on the layout {}", self.layout.name));
        }
        self.key_typing(stroke.code, mods, Some(c))
    }

    /// Takes the typed character back: ⌘Z with the key the layout types `z` with, or Backspace after
    /// `undo_with_backspace`.
    pub(crate) fn undo(&self) -> Result<u64, String> {
        if self.backspace.get() {
            return self.key(keys::DELETE, Mods::default());
        }
        let code = self.stroke('z')?.code;
        self.key_typing(code, CMD, Some('z'))
    }

    /// Makes `undo` press Backspace.
    pub(crate) fn undo_with_backspace(&self) {
        self.backspace.set(true);
    }

    /// Closes an open menu of the measured process with Escape, which a click would otherwise pick from.
    fn close_menu(&self) -> Result<(), String> {
        if let Some(menu) = tree::open_menu(self.pid) {
            eprintln!("    a menu of the measured app is open ({menu}); pressing Escape");
            self.key(keys::ESCAPE, Mods::default())?;
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        Ok(())
    }

    /// Presses `at` twice, as a double-click.
    pub(crate) fn double_click(&self, at: CGPoint, window: u32) -> Result<(), String> {
        self.close_menu()?;
        let _ = CGWarpMouseCursorPosition(at);
        self.mouse(CGEventType::MouseMoved, at, window)?;
        for clicks in 1..=2 {
            std::thread::sleep(std::time::Duration::from_millis(40));
            self.mouse_clicks(CGEventType::LeftMouseDown, at, window, clicks)?;
            std::thread::sleep(std::time::Duration::from_millis(20));
            self.mouse_clicks(CGEventType::LeftMouseUp, at, window, clicks)?;
        }
        Ok(())
    }

    /// Types `text` a key at a time, with a pause in between as a quick typist leaves.
    pub(crate) fn text(&self, text: &str) -> Result<(), String> {
        for c in text.chars() {
            self.char(c)?;
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        Ok(())
    }

    /// A mouse event of `kind` at `at`, in points from the top left of the main display, over the window `window`.
    fn mouse(&self, kind: CGEventType, at: CGPoint, window: u32) -> Result<(), String> {
        self.mouse_clicks(kind, at, window, 1)
    }

    /// `mouse` as the `clicks`th click of a row.
    fn mouse_clicks(&self, kind: CGEventType, at: CGPoint, window: u32, clicks: i64) -> Result<(), String> {
        let event =
            CGEvent::new_mouse_event(Some(&self.source), kind, at, CGMouseButton::Left).ok_or("no mouse event")?;
        // An event of the HID system's source would otherwise carry the modifiers the system holds.
        CGEvent::set_flags(Some(&event), CGEventFlags::empty());
        if kind != CGEventType::MouseMoved {
            CGEvent::set_integer_value_field(Some(&event), CGEventField::MouseEventClickState, clicks);
        }
        over(&event, window);
        self.post(&event).map(drop)
    }

    /// Moves the pointer to `at` and clicks there, which gives the view under it the keyboard.
    pub(crate) fn click(&self, at: CGPoint, window: u32) -> Result<(), String> {
        self.close_menu()?;
        let _ = CGWarpMouseCursorPosition(at);
        self.mouse(CGEventType::MouseMoved, at, window)?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.mouse(CGEventType::LeftMouseDown, at, window)?;
        std::thread::sleep(std::time::Duration::from_millis(30));
        self.mouse(CGEventType::LeftMouseUp, at, window)
    }

    /// Moves the pointer to `at` and tells the app, which scrolls the view under its last known pointer.
    pub(crate) fn hover(&self, at: CGPoint, window: u32) -> Result<(), String> {
        let _ = CGWarpMouseCursorPosition(at);
        self.mouse(CGEventType::MouseMoved, at, window)
    }

    /// A precise scroll of `delta` points, positive up, at `at` over the window `window`; the time right before it.
    pub(crate) fn scroll(&self, delta: i32, phase: Phase, at: CGPoint, window: u32) -> Result<u64, String> {
        let event = CGEvent::new_scroll_wheel_event2(Some(&self.source), CGScrollEventUnit::Pixel, 1, delta, 0, 0)
            .ok_or("no scroll event")?;
        CGEvent::set_location(Some(&event), at);
        CGEvent::set_integer_value_field(Some(&event), CGEventField::ScrollWheelEventIsContinuous, 1);
        CGEvent::set_integer_value_field(
            Some(&event),
            CGEventField::ScrollWheelEventScrollPhase,
            phase_value(phase),
        );
        over(&event, window);
        self.post(&event)
    }
}

/// Names `window` as the one under the pointer, which the window server sets only for events it routes itself.
fn over(event: &CGEvent, window: u32) {
    for field in [
        CGEventField::MouseEventWindowUnderMousePointer,
        CGEventField::MouseEventWindowUnderMousePointerThatCanHandleThisEvent,
    ] {
        CGEvent::set_integer_value_field(Some(event), field, i64::from(window));
    }
}
