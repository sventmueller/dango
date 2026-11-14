//! Breakpoints in the editor's gutter: a click on a line number sets or clears one, the context menu changes it, and
//! each draws as a tag over its number, filled when it stops, outlined when it is switched off, as a diamond when it
//! logs (E101: never by colour alone). The paused line gets an arrow over its number.

use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSBezierPath, NSColor, NSMenu, NSMenuItem, NSTextField,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use splat_i18n::{t, tf};

use super::model::breakpoints::{Edit, Look};
use super::panel::DebugPanel;
use ide_ui_common::util::{guarded, item, ns, separator, set_target};
use ide_ui_common::window::Window;

/// What a menu item of a breakpoint changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    Switch,
    Condition,
    HitCondition,
    LogMessage,
    Remove,
    Set,
}

impl What {
    const ALL: [What; 6] = [
        What::Switch,
        What::Condition,
        What::HitCondition,
        What::LogMessage,
        What::Remove,
        What::Set,
    ];

    fn name(self) -> &'static str {
        match self {
            What::Switch => "switch",
            What::Condition => "condition",
            What::HitCondition => "hits",
            What::LogMessage => "log",
            What::Remove => "remove",
            What::Set => "set",
        }
    }
}

/// Separates the parts of a menu item's represented object.
const SEPARATOR: char = '\u{1f}';

/// The menu item `title` acting on the breakpoint at `line` of `path` with `what`.
fn entry(panel: &DebugPanel, title: &str, what: What, path: &Path, line: usize, enabled: bool) -> Retained<NSMenuItem> {
    let action = if what == What::Remove {
        sel!(ideDebugRemoveBreakpoint:)
    } else {
        sel!(ideDebugEditBreakpoint:)
    };
    let entry = item(panel.mtm(), title, Some(action), "");
    let target = format!("{}{SEPARATOR}{line}{SEPARATOR}{}", what.name(), path.display());
    let target: Retained<AnyObject> = ns(&target).into();
    // SAFETY: The represented object is a string, which the item keeps.
    unsafe { entry.setRepresentedObject(Some(&target)) };
    set_target(&entry, panel);
    entry.setEnabled(enabled);
    entry
}

/// What a breakpoint's menu item acts on.
pub fn menu_target(sender: Option<&AnyObject>) -> Option<(What, PathBuf, usize)> {
    let item = sender?.downcast_ref::<NSMenuItem>()?;
    let text = item.representedObject()?.downcast::<NSString>().ok()?.to_string();
    let mut parts = text.splitn(3, SEPARATOR);
    let what = parts.next()?;
    let what = What::ALL.into_iter().find(|w| w.name() == what)?;
    let line = parts.next()?.parse().ok()?;
    Some((what, PathBuf::from(parts.next()?), line))
}

/// The menu items for the breakpoint at `line` of `path`, or for setting one there; greyed out what the running
/// session's adapter cannot do.
pub fn breakpoint_items(panel: &DebugPanel, path: &Path, line: usize) -> Vec<Retained<NSMenuItem>> {
    let model = panel.read();
    let caps = model.current().filter(|s| !s.is_over()).map(|s| s.caps.clone());
    let can = |flag: fn(&super::model::session::Caps) -> bool| caps.as_ref().is_none_or(flag);
    let (conditions, hits, logs) = (can(|c| c.conditions), can(|c| c.hit_conditions), can(|c| c.log_points));
    let Some(breakpoint) = model.breakpoints.at(path, line).cloned() else {
        drop(model);
        return vec![
            entry(panel, t("Haltepunkt setzen"), What::Set, path, line, true),
            entry(panel, t("Bedingung …"), What::Condition, path, line, conditions),
            entry(panel, t("Logmeldung …"), What::LogMessage, path, line, logs),
        ];
    };
    drop(model);
    let switch = if breakpoint.enabled {
        t("Haltepunkt ausschalten")
    } else {
        t("Haltepunkt einschalten")
    };
    vec![
        entry(panel, switch, What::Switch, path, line, true),
        separator(panel.mtm()),
        entry(panel, t("Bedingung …"), What::Condition, path, line, conditions),
        entry(panel, t("Trefferzahl …"), What::HitCondition, path, line, hits),
        entry(panel, t("Logmeldung …"), What::LogMessage, path, line, logs),
        separator(panel.mtm()),
        entry(panel, t("Haltepunkt entfernen"), What::Remove, path, line, true),
    ]
}

/// Carries out a breakpoint's menu item: switches it, sets it, or asks for its condition, hit count or log message.
pub fn edit(panel: &DebugPanel, what: What, path: &Path, line: usize) {
    let existing = panel.read().breakpoints.at(path, line).cloned();
    let (title, info, current) = match what {
        What::Switch => {
            let on = existing.is_none_or(|b| !b.enabled);
            panel.change_breakpoint(path, line, Edit::Enable(on));
            return;
        }
        What::Set | What::Remove => {
            if existing.is_none() || what == What::Remove {
                panel.toggle_breakpoint(path, line);
            }
            return;
        }
        What::Condition => (
            t("Bedingung"),
            t("Das Programm hält nur an, wenn der Ausdruck wahr ist. Leer hält es immer an."),
            existing.as_ref().and_then(|b| b.condition.clone()),
        ),
        What::HitCondition => (
            t("Trefferzahl"),
            t("Das Programm hält erst an, wenn der Haltepunkt so oft erreicht wurde, zum Beispiel 5 oder >= 5."),
            existing.as_ref().and_then(|b| b.hit_condition.clone()),
        ),
        What::LogMessage => (
            t("Logmeldung"),
            t(
                "Ein Logpunkt hält nicht an, sondern schreibt die Meldung in die Debug-Konsole; {Ausdruck} setzt einen Wert ein.",
            ),
            existing.as_ref().and_then(|b| b.log_message.clone()),
        ),
    };
    let Some(window) = panel.controller().and_then(|c| c.window()) else {
        return;
    };
    let (path, id) = (path.to_path_buf(), panel.id());
    ask(&window, title, info, current.as_deref().unwrap_or(""), move |text| {
        super::panel::with_panel(id, |panel| {
            if panel.read().breakpoints.at(&path, line).is_none() {
                panel.toggle_breakpoint(&path, line);
            }
            let edit = match what {
                What::Condition => Edit::Condition(Some(text)),
                What::HitCondition => Edit::HitCondition(Some(text)),
                _ => Edit::LogMessage(Some(text)),
            };
            panel.change_breakpoint(&path, line, edit);
        });
    });
}

/// Asks in a sheet for a line of text, prefilled with `current`; `then` gets it, also empty, unless the user cancels.
fn ask(window: &objc2_app_kit::NSWindow, title: &str, info: &str, current: &str, then: impl FnOnce(String) + 'static) {
    let mtm = window.mtm();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(title));
    alert.setInformativeText(&ns(info));
    alert.addButtonWithTitle(&ns(t("OK")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(current), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 24.0)));
    field.setAccessibilityLabel(Some(&ns(title)));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    let then = std::cell::Cell::new(Some(then));
    let handler = block2::RcBlock::new(move |response: objc2_app_kit::NSModalResponse| {
        guarded((), || {
            if response == NSAlertFirstButtonReturn
                && let Some(then) = then.take()
            {
                then(field.stringValue().to_string());
            }
        });
    });
    alert.beginSheetModalForWindow_completionHandler(window, Some(&handler));
}

/// A click on the line numbers at `line` of `path` in `window`: sets or clears a breakpoint there; whether the window
/// has a debugger to keep it.
pub fn click(window: Window, path: &Path, line: usize) -> bool {
    let Some(panel) = super::panel::of(&window) else {
        return false;
    };
    panel.toggle_breakpoint(path, line);
    true
}

/// The context menu of the line numbers at `line` of `path` in `window`.
pub fn menu(window: Window, path: &Path, line: usize) -> Option<Retained<NSMenu>> {
    let panel = super::panel::of(&window)?;
    let menu = NSMenu::new(panel.mtm());
    menu.setAutoenablesItems(false);
    for entry in breakpoint_items(&panel, path, line) {
        menu.addItem(&entry);
    }
    Some(menu)
}

/// How the line `line` of `path` in `window` is marked in the gutter: its breakpoint, and whether the program is
/// paused there.
pub fn mark(window: Window, path: &Path, line: usize) -> (Option<Look>, bool) {
    let Some(panel) = super::panel::of(&window) else {
        return (None, false);
    };
    let model = panel.read();
    let paused = model
        .paused_at()
        .is_some_and(|frame| frame.line == line && frame.path.as_deref() == Some(path));
    (model.breakpoints.look(path, line), paused)
}

/// How the lines of `path` in `window` are marked in the gutter: their breakpoints by line, and the line the program
/// is paused at.
pub fn marks(window: Window, path: &Path) -> (std::collections::HashMap<usize, Look>, Option<usize>) {
    let Some(panel) = super::panel::of(&window) else {
        return Default::default();
    };
    let model = panel.read();
    let paused = model
        .paused_at()
        .filter(|frame| frame.path.as_deref() == Some(path))
        .map(|frame| frame.line);
    (model.breakpoints.looks(path), paused)
}

/// Draws a line's breakpoint as a tag across `rect`, the gutter up to the change strip; whether the number should be
/// drawn light on it.
pub fn draw(look: Option<Look>, rect: NSRect) -> bool {
    let Some(look) = look else { return false };
    let accent = NSColor::systemBlueColor();
    let tag = NSRect::new(
        NSPoint::new(rect.origin.x, rect.origin.y + 1.0),
        NSSize::new(rect.size.width, (rect.size.height - 2.0).max(4.0)),
    );
    match look {
        Look::Log => {
            // A log point is a diamond at the column's leading edge, the number stays as it is.
            let side = tag.size.height.min(10.0);
            let middle = NSPoint::new(tag.origin.x + side / 2.0, tag.origin.y + tag.size.height / 2.0);
            let path = NSBezierPath::bezierPath();
            path.moveToPoint(NSPoint::new(middle.x, middle.y - side / 2.0));
            path.lineToPoint(NSPoint::new(middle.x + side / 2.0, middle.y));
            path.lineToPoint(NSPoint::new(middle.x, middle.y + side / 2.0));
            path.lineToPoint(NSPoint::new(middle.x - side / 2.0, middle.y));
            path.closePath();
            accent.setFill();
            path.fill();
            false
        }
        Look::Disabled => {
            let path = tag_path(NSRect::new(
                NSPoint::new(tag.origin.x + 0.6, tag.origin.y + 0.6),
                NSSize::new(tag.size.width - 1.2, tag.size.height - 1.2),
            ));
            accent.setStroke();
            path.setLineWidth(1.2);
            path.stroke();
            false
        }
        Look::Unverified => {
            // Grey with a bar across, so it differs from a set breakpoint by shape as well (E101).
            NSColor::systemGrayColor().setFill();
            tag_path(tag).fill();
            let bar = NSRect::new(
                NSPoint::new(tag.origin.x + 3.0, tag.origin.y + tag.size.height / 2.0 - 0.75),
                NSSize::new((tag.size.width - 9.0).max(3.0), 1.5),
            );
            NSColor::whiteColor().setFill();
            NSBezierPath::fillRect(bar);
            false
        }
        Look::Enabled | Look::Conditional => {
            accent.setFill();
            tag_path(tag).fill();
            if look == Look::Conditional {
                let dot = NSRect::new(
                    NSPoint::new(tag.origin.x + 3.0, tag.origin.y + tag.size.height / 2.0 - 1.5),
                    NSSize::new(3.0, 3.0),
                );
                NSColor::whiteColor().setFill();
                NSBezierPath::bezierPathWithOvalInRect(dot).fill();
            }
            true
        }
    }
}

/// Draws the paused line's green arrow across `rect`, over a breakpoint's tag; the number is then drawn light on it.
pub fn draw_paused_arrow(rect: NSRect) {
    let arrow = NSRect::new(
        NSPoint::new(rect.origin.x, rect.origin.y + 1.0),
        NSSize::new(rect.size.width, (rect.size.height - 2.0).max(4.0)),
    );
    NSColor::systemGreenColor().setFill();
    tag_path(arrow).fill();
}

/// A tag pointing right, as Xcode draws a breakpoint and the paused line.
fn tag_path(rect: NSRect) -> Retained<NSBezierPath> {
    let point = (rect.size.height / 2.0).min(6.0);
    let (left, top, bottom) = (rect.origin.x, rect.origin.y, rect.origin.y + rect.size.height);
    let right = rect.origin.x + rect.size.width;
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(left + 2.0, top));
    path.lineToPoint(NSPoint::new(right - point, top));
    path.lineToPoint(NSPoint::new(right, top + rect.size.height / 2.0));
    path.lineToPoint(NSPoint::new(right - point, bottom));
    path.lineToPoint(NSPoint::new(left + 2.0, bottom));
    path.curveToPoint_controlPoint1_controlPoint2(
        NSPoint::new(left, bottom - 2.0),
        NSPoint::new(left + 1.0, bottom),
        NSPoint::new(left, bottom - 1.0),
    );
    path.lineToPoint(NSPoint::new(left, top + 2.0));
    path.curveToPoint_controlPoint1_controlPoint2(
        NSPoint::new(left + 2.0, top),
        NSPoint::new(left, top + 1.0),
        NSPoint::new(left + 1.0, top),
    );
    path.closePath();
    path
}

/// What VoiceOver hears for the line `line`'s marks in the gutter, said with its number.
pub fn words(look: Option<Look>, paused: bool, line: usize) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(look) = look {
        parts.push(super::rows::look_words(look));
    }
    if paused {
        parts.push(t("Programm hier angehalten"));
    }
    (!parts.is_empty()).then(|| tf("{}, Zeile {}", &[&parts.join(", "), &(line + 1).to_string()]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voiceover_hears_a_lines_marks_with_its_number() {
        assert_eq!(words(None, false, 4), None);
        assert_eq!(words(Some(Look::Log), false, 4).as_deref(), Some("Logpunkt, Zeile 5"));
        assert_eq!(
            words(Some(Look::Enabled), true, 0).as_deref(),
            Some("Haltepunkt, Programm hier angehalten, Zeile 1")
        );
        assert_eq!(
            words(None, true, 9).as_deref(),
            Some("Programm hier angehalten, Zeile 10")
        );
    }

    #[test]
    fn each_menu_action_has_a_name_of_its_own() {
        let names: Vec<&str> = What::ALL.iter().map(|w| w.name()).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), What::ALL.len(), "{names:?}");
    }
}
