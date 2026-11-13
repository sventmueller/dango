//! The activity popover while its rows change: the keyboard focus stays on its row when a row above it goes, and
//! goes to the nearest row left when its own goes, never back to the popover's start.

use std::ops::Range;
use std::path::{Path, PathBuf};

use objc2::Message;
use objc2::rc::Retained;
use objc2_app_kit::{NSAccessibility, NSButton, NSView, NSWindow};
use splat_i18n::t;

use super::states::SUBAGENT;
use super::window_of;
use crate::selftest::check;
use crate::selftest::helpers::wait_until;

/// The lines of the recording after which its subagent ends, and after which its background shell stops.
const SUBAGENT_ENDS: Range<usize> = 180..182;
const SHELL_STOPS: Range<usize> = 174..176;

fn buttons(view: &NSView, found: &mut Vec<Retained<NSButton>>) {
    if let Some(button) = view.downcast_ref::<NSButton>() {
        found.push(button.retain());
    }
    for sub in view.subviews().iter() {
        buttons(&sub, found);
    }
}

/// A button of the popover and what VoiceOver calls it.
type Labelled = (Retained<NSButton>, String);

/// The popover's window and buttons.
fn shown(root: &Path) -> Option<(Retained<NSWindow>, Vec<Labelled>)> {
    let controller = window_of(root)?;
    let view = crate::activity::popover_view(&controller)?;
    let mut found = Vec::new();
    buttons(&view, &mut found);
    let labelled = found
        .into_iter()
        .map(|b| {
            let label = b.accessibilityLabel().map(|l| l.to_string()).unwrap_or_default();
            (b, label)
        })
        .collect();
    Some((view.window()?, labelled))
}

/// What VoiceOver calls the button with the focus, and how many buttons the popover has.
fn focus(root: &Path) -> (String, usize) {
    let Some((window, buttons)) = shown(root) else {
        return (String::new(), 0);
    };
    let responder = window.firstResponder();
    let label = buttons
        .iter()
        .find(|(button, _)| {
            responder
                .as_ref()
                .is_some_and(|r| Retained::as_ptr(r).cast::<NSButton>() == Retained::as_ptr(button))
        })
        .map(|(_, label)| label.clone())
        .unwrap_or_default();
    (label, buttons.len())
}

fn feed(key: u64, lines: Range<usize>) {
    let frames: String = SUBAGENT
        .lines()
        .skip(lines.start)
        .take(lines.len())
        .map(|line| format!("{line}\n"))
        .collect();
    if let Some(tab) = crate::agent::all().into_iter().find(|tab| tab.key == key) {
        tab.feed(&frames);
    }
}

/// With the focus on the background shell's Stop, the subagent above it ends: the focus stays on the shell's row.
/// Then the shell stops: the focus goes to the nearest row left, the session's.
pub fn rows_change(root: PathBuf, key: u64, then: fn(PathBuf)) {
    let shell = shown(&root).and_then(|(window, buttons)| {
        let (button, _) = buttons
            .into_iter()
            .find(|(_, label)| label.starts_with(t("Stopp")) && label.contains("countdown"))?;
        window.makeFirstResponder(Some(&button)).then_some(())
    });
    if shell.is_none() {
        check(
            "the activity popover keeps the focus on its row as rows go",
            false,
            "no Stop of the shell",
        );
        return then(root);
    }
    let (_, before) = focus(&root);
    feed(key, SUBAGENT_ENDS);
    let probe = root.clone();
    wait_until(
        move || focus(&probe).1 < before,
        move |_| {
            let (label, after) = focus(&root);
            check(
                "the activity popover keeps the focus on its row as rows above it go",
                after < before && label.contains("countdown"),
                format!("{before} then {after} buttons, focus on {label:?}"),
            );
            let title = crate::agent::all()
                .into_iter()
                .find(|tab| tab.key == key)
                .map(|tab| tab.title());
            feed(key, SHELL_STOPS);
            let probe = root.clone();
            wait_until(
                move || focus(&probe).1 < after,
                move |_| {
                    let (label, left) = focus(&root);
                    let session = title.as_deref().is_some_and(|title| label.contains(title));
                    check(
                        "the activity popover moves the focus to the nearest row left when its row goes",
                        left < after && session,
                        format!("{after} then {left} buttons, focus on {label:?}"),
                    );
                    then(root);
                },
            );
        },
    );
}
