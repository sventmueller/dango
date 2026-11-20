//! Part of the `markdown` scenario: a long Markdown line wraps at the editor's visible edge and wraps again when the
//! window narrows, without a horizontal offset; the editor's context menu has the Format menu. The editor keeps the
//! keyboard focus when its preview opens beside it, and the window answers the Format menu for it while the focus is
//! elsewhere (QA-04).

use std::path::Path;

use objc2::rc::Retained;
use objc2::{msg_send, sel};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSMenuItem, NSTextElementProvider, NSView};
use objc2_foundation::NSObjectProtocol;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::check;
use super::helpers::{editor, window_of};
use crate::editor::EditorTextView;
use crate::util::ns;
use crate::window::WindowController;

const SENTENCE: &str = "Ein Satz, der mit vielen anderen einen Absatz bildet, so lang, dass er umbrechen muss. ";

/// How the editor of a wrapping line sits: its width, the width its scroll view shows, the line's height and the
/// horizontal scroll offset.
struct Wrap {
    width: f64,
    shown: f64,
    height: f64,
    offset: f64,
}

fn measure(view: &EditorTextView) -> Option<Wrap> {
    let scroll = view.enclosingScrollView()?;
    let (layout, content) = (view.textLayoutManager()?, view.textContentStorage()?);
    let range = content.documentRange();
    layout.ensureLayoutForRange(&range);
    let fragment = layout.textLayoutFragmentForLocation(&range.location())?;
    Some(Wrap {
        width: view.frame().size.width,
        shown: scroll.contentSize().width,
        height: fragment.layoutFragmentFrame().size.height,
        offset: scroll.contentView().bounds().origin.x,
    })
}

fn fits(wrap: &Wrap) -> bool {
    (wrap.width - wrap.shown).abs() < 1.0 && wrap.offset == 0.0
}

fn describe(wrap: Option<&Wrap>) -> String {
    wrap.map_or_else(
        || "no editor".into(),
        |w| {
            format!(
                "{:.0} of {:.0} pt shown, {:.0} pt high, offset {:.0}",
                w.width, w.shown, w.height, w.offset
            )
        },
    )
}

pub(super) fn run(root: &Path) {
    let file = root.join("lang.md");
    let _ = std::fs::write(&file, format!("{}\n", SENTENCE.repeat(12)));
    let Some(controller) = window_of(root) else { return };
    controller.open_file(&file);
    let Some(view) = editor(&controller, "lang.md") else {
        return check(
            "a long Markdown line wraps at the editor's visible edge",
            false,
            "no editor",
        );
    };
    let kept = has_focus(&view);
    let wide = measure(&view);
    let narrower = view.window().map(|window| {
        let frame = window.frame();
        let smaller = NSRect::new(frame.origin, NSSize::new(frame.size.width - 240.0, frame.size.height));
        window.setFrame_display(smaller, true);
        if let Some(content) = window.contentView() {
            content.layoutSubtreeIfNeeded();
        }
        let narrow = measure(&view);
        window.setFrame_display(frame, true);
        narrow
    });
    let narrow = narrower.flatten();
    check(
        "a long Markdown line wraps at the editor's visible edge",
        wide.as_ref().is_some_and(fits),
        describe(wide.as_ref()),
    );
    check(
        "a narrower window wraps it again at the new edge",
        narrow.as_ref().is_some_and(fits)
            && wide
                .as_ref()
                .zip(narrow.as_ref())
                .is_some_and(|(w, n)| n.height > w.height),
        format!("{} / {}", describe(wide.as_ref()), describe(narrow.as_ref())),
    );
    if let Some(bold) = context_menu(&view) {
        focus(&controller, &view, kept, &bold);
    }
}

fn has_focus(view: &EditorTextView) -> bool {
    let view: &NSView = view;
    view.window()
        .and_then(|window| window.firstResponder())
        .and_then(|focus| focus.downcast::<NSView>().ok())
        .is_some_and(|focus| std::ptr::eq(&*focus, view))
}

/// Whether the editor had the focus once it opened with its preview beside it, and the window enables `bold` for it
/// while the focus is on the window itself.
fn focus(controller: &WindowController, view: &EditorTextView, kept: bool, bold: &NSMenuItem) {
    check(
        "a Markdown file opened with its preview beside it keeps the keyboard focus in its editor",
        kept,
        format!("focus in the editor: {kept}"),
    );
    let Some(window) = view.window() else { return };
    window.makeFirstResponder(None);
    let answers = controller.respondsToSelector(sel!(ideMarkdown:));
    // SAFETY: The window controller answers validateMenuItem: with a bool for the item, which lives for the call.
    let enabled: bool = unsafe { msg_send![controller, validateMenuItem: bold] };
    window.makeFirstResponder(Some(view));
    check(
        "with the focus on the window, the window answers the Format menu for its editor",
        answers && enabled,
        format!("answers {answers}, enabled {enabled}"),
    );
}

/// Checks the editor's context menu; its Format menu's bold item.
fn context_menu(view: &EditorTextView) -> Option<Retained<NSMenuItem>> {
    let window = view.window()?;
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::RightMouseDown,
        view.convertPoint_toView(NSPoint::new(20.0, 20.0), None),
        NSEventModifierFlags::empty(),
        0.0,
        window.windowNumber(),
        None,
        0,
        1,
        1.0,
    );
    let menu = event.and_then(|event| view.menuForEvent(&event));
    let format = menu
        .as_ref()
        .and_then(|menu| menu.itemWithTitle(&ns(t("Format"))))
        .and_then(|item| item.submenu());
    let bold: Option<Retained<NSMenuItem>> = format.as_ref().and_then(|menu| menu.itemWithTitle(&ns(t("Fett"))));
    // SAFETY: The editor answers validateMenuItem: with a bool for the item, which lives for the call.
    let enabled = bold
        .as_ref()
        .is_some_and(|item| unsafe { msg_send![view, validateMenuItem: &**item] });
    let key = bold
        .as_ref()
        .map(|item| item.keyEquivalent().to_string())
        .unwrap_or_default();
    check(
        "the editor's context menu has the Format menu, with its shortcuts and enabled",
        format.is_some_and(|menu| menu.numberOfItems() >= 13) && enabled && key == "b",
        format!("enabled {enabled}, shortcut {key:?}"),
    );
    bold
}
