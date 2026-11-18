//! Part of the `markdown` scenario: a Markdown file and its preview share their tab half and half, keep that share
//! as the window narrows and the one the user drags to; only the preview has a badge, whose × closes just it.

use std::path::Path;

use objc2_foundation::{NSRect, NSSize};
use splat_i18n::{t, tf};

use super::helpers::{tab, window_of};
use super::{check, snapshot};
use crate::window::{TabId, WindowController};

/// The share of the tab of `id` after the window took `narrower` points off its width; the window gets its width back.
fn share_when_narrower(controller: &WindowController, id: TabId, narrower: f64) -> Option<f64> {
    let window = controller.window()?;
    let frame = window.frame();
    let smaller = NSRect::new(
        frame.origin,
        NSSize::new(frame.size.width - narrower, frame.size.height),
    );
    let lay_out = || {
        if let Some(content) = window.contentView() {
            content.layoutSubtreeIfNeeded();
        }
    };
    window.setFrame_display(smaller, true);
    lay_out();
    let share = controller.share_of(id);
    window.setFrame_display(frame, true);
    lay_out();
    share
}

fn near(share: Option<f64>, wanted: f64) -> bool {
    share.is_some_and(|share| (share - wanted).abs() < 0.02)
}

pub(super) fn run(root: &Path) {
    let Some(controller) = window_of(root) else { return };
    let Some(id) = tab(&controller, "lang.md") else {
        return check(
            "a document and its preview share their tab half and half",
            false,
            "no tab",
        );
    };
    if let Some(content) = controller.window().and_then(|w| w.contentView()) {
        content.layoutSubtreeIfNeeded();
    }
    let start = controller.share_of(id);
    let narrow = share_when_narrower(&controller, id, 200.0);
    check(
        "a document and its preview share their tab half and half, also in a narrower window",
        near(start, 0.5) && near(narrow, 0.5),
        format!("{start:?}, narrower {narrow:?}"),
    );
    controller.drag_share_of(id, 0.35);
    let dragged = controller.share_of(id);
    let kept = share_when_narrower(&controller, id, 200.0);
    check(
        "the share the user drags to stays as the window narrows",
        near(dragged, 0.35) && near(kept, 0.35),
        format!("{dragged:?}, narrower {kept:?}"),
    );
    controller.set_share_of(id, 0.5);
    if let Some(window) = controller.window() {
        snapshot(&window, "split");
    }
    let badges = controller.badges_of(id);
    let preview = (t("Vorschau").to_string(), t("Vorschau schließen").to_string());
    check(
        "only the preview has a badge, which names it and its close button",
        badges == [None, Some(preview)],
        format!("{badges:?}"),
    );
    let preview_id = tab(&controller, &tf("Vorschau: {}", &["lang.md"]));
    if let Some(button) = preview_id.and_then(|preview| controller.badge_close(preview)) {
        // SAFETY: A click with no sender, which the close action takes.
        unsafe { button.performClick(None) };
    }
    let left = controller.window_tabs();
    check(
        "the badge's × closes the preview only, its document stays in the tab",
        left.iter().any(|docs| docs == &["lang.md".to_string()]),
        format!("{left:?}"),
    );
}
