//! What the self-test reads from and does with the parts of `mod.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// What the self-test reads from the popover and the toolbar item: the popover's lines and the item's tooltip.
pub fn inspect(controller: &WindowController) -> (Vec<String>, String) {
    let Some(activity) = of(controller) else {
        return (Vec::new(), String::new());
    };
    activity.show();
    let tabs = crate::agent::of_window(controller);
    let lines: Vec<String> = agent_lines(&tabs, SystemTime::now(), false)
        .into_iter()
        .chain(process_lines(controller, &processes(), SystemTime::now()))
        .map(|l| {
            let buttons: Vec<&str> = l.buttons.iter().map(|(title, _)| *title).collect();
            format!("{} — {} [{}]", l.title, l.detail, buttons.join(", "))
        })
        .collect();
    let tooltip = activity
        .items()
        .first()
        .and_then(|i| i.toolTip())
        .map(|t| t.to_string())
        .unwrap_or_default();
    (lines, tooltip)
}

/// Closes the popover, for the self-test's later pictures.
pub fn close(controller: &WindowController) {
    if let Some(activity) = of(controller)
        && let Some(popover) = activity.ivars().popover.borrow().as_ref()
    {
        // SAFETY: Closing takes any sender; none is given.
        unsafe { popover.performClose(None) };
    }
}

/// The lines of the open popover, as the session overview shows them.
pub fn overview_lines(controller: &WindowController) -> Vec<String> {
    let Some(activity) = of(controller) else {
        return Vec::new();
    };
    let tabs = crate::agent::of_window(controller);
    let now = SystemTime::now();
    match activity.ivars().only.get() {
        Some((key, part)) => session_lines(&tabs, key, part, now),
        None => agent_lines(&tabs, now, false),
    }
    .into_iter()
    .map(|l| format!("{} — {}", l.title, l.detail))
    .collect()
}

/// How often a popover changed the texts of its rows in place.
pub(super) static UPDATED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How often a popover changed the texts of its rows in place, for the self-test.
pub fn updated_in_place() -> usize {
    UPDATED.load(std::sync::atomic::Ordering::Relaxed)
}

/// The popover's content, for the self-test's picture.
pub fn popover_view(controller: &WindowController) -> Option<Retained<NSView>> {
    let activity = of(controller)?;
    let popover = activity.ivars().popover.borrow().clone()?;
    Some(popover.contentViewController()?.view())
}
