//! Part of the `markdown` scenario: a Markdown file and its preview share their tab half and half, keep that share
//! as the window narrows and the one the user drags to; the format bar spans both in place of the preview's badge, and
//! its preview switch closes just the preview (E195).

use std::path::Path;

use objc2_foundation::{NSRect, NSSize};
use splat_i18n::tf;

use super::helpers::{tab, window_of};
use super::{check, snapshot};
use crate::markdown::Command;
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
    let title = controller.format_bar_title(id);
    check(
        "the format bar spans document and preview, named after the document, and the preview has no badge",
        badges == [None, None] && title.as_deref() == Some("lang.md"),
        format!("{badges:?}, bar {title:?}"),
    );
    let preview_id = tab(&controller, &tf("Vorschau: {}", &["lang.md"]));
    let from_preview = preview_id.and_then(|preview| controller.format_bar_title(preview));
    check(
        "the format bar stays while the preview is in front",
        from_preview.as_deref() == Some("lang.md"),
        format!("{from_preview:?}"),
    );
    if let Some(button) = controller.format_bar_button(id, Command::Preview) {
        // SAFETY: A click with no sender, which the bar's action takes.
        unsafe { button.performClick(None) };
    }
    let left = controller.window_tabs();
    check(
        "the bar's preview switch closes the preview only, its document stays in the tab",
        left.iter().any(|docs| docs == &["lang.md".to_string()]),
        format!("{left:?}"),
    );
    let still = controller.format_bar_title(id);
    check(
        "the format bar stays over the document alone",
        still.as_deref() == Some("lang.md"),
        format!("{still:?}"),
    );
}
