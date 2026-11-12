//! The `inlay-repo` scenario: a file of this repository under the real rust-analyzer, scrolled into its middle; every
//! gap a hint opened in the lines on screen shows its hint, and the pointer turns into a hand over a hint with a place.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_foundation::NSRect;

use super::{check, finish, snapshot};
use crate::editor::insets::{self, Layer};
use crate::editor::{self, EditorTextView};
use crate::util::main_after;
use crate::window;

const SCENARIO: &str = "inlay-repo";
const FILE: &str = "crates/ide-acp/src/capabilities.rs";
/// The line with closure parameters whose types rust-analyzer shows.
const LINE: usize = 146;

fn root() -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    folder.canonicalize().unwrap_or(folder)
}

fn view() -> Option<Retained<EditorTextView>> {
    let controller = window::for_project(&root())?;
    editor::editor_for(&controller, &root().join(FILE))
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&root());
    crate::app::open_project(mtm, root());
    main_after(Duration::from_secs(1), || {
        let Some(controller) = window::for_project(&root()) else {
            check("the repository opens", false, "");
            return finish(SCENARIO);
        };
        controller.open_file(&root().join(FILE));
        wait(Instant::now());
    });
}

/// Waits for rust-analyzer's hints, asking again every two seconds, as a server that is still indexing has none.
fn wait(started: Instant) {
    let Some(view) = view() else {
        check("the file opens", false, "");
        return finish(SCENARIO);
    };
    if insets::of(&view, Layer::Inlay).is_empty() && started.elapsed() < Duration::from_secs(240) {
        crate::lsp::inlay::request(&view);
        return main_after(Duration::from_secs(2), move || wait(started));
    }
    let at = view.line_start(LINE).unwrap_or(0);
    let before = editor::LAID_OUT.load(std::sync::atomic::Ordering::Relaxed);
    editor::scroll_to_byte(&view, view.document().text().utf16_to_byte(at).unwrap_or(0));
    if let Some(window) = view.window() {
        window.displayIfNeeded();
    }
    let after = editor::LAID_OUT.load(std::sync::atomic::Ordering::Relaxed);
    check(
        "after a scroll TextKit's layout of the lines on screen redraws the overlay",
        after > before,
        format!("{} layouts reported", after - before),
    );
    main_after(Duration::from_secs(3), look);
}

fn look() {
    let Some(view) = view() else { return finish(SCENARIO) };
    if let Some(window) = view.window() {
        window.displayIfNeeded();
        snapshot(&window, "inlay-repo");
        let probe = std::env::temp_dir().join("inlay-repo-capture.png");
        let real = super::capture(&["-o", "-l", &window.windowNumber().to_string()], &probe);
        check("the picture comes from the screen, not from AppKit's drawing", real, "");
    }
    let seen = view.visibleRect();
    let lines = editor::utf16_in(&view, seen);
    let hints = insets::of(&view, Layer::Inlay);
    let kerns = insets::kerns(&view);
    let kerned_here = kerns.iter().filter(|(range, _)| lines.contains(&range.start)).count();
    let hints_here = hints.iter().filter(|hint| lines.contains(&hint.at)).count();
    let drawn = insets::drawn_in(&view, seen);
    let shown = insets::shown(&view).len();
    check(
        "every gap in the lines on screen shows its hint",
        kerned_here > 0 && drawn >= kerned_here,
        format!(
            "{} hints, {hints_here} on screen; {} kerns, {kerned_here} on screen; drawn {drawn}, shown {shown}, lines {lines:?}, visible {seen:?}",
            hints.len(),
            kerns.len()
        ),
    );
    let target = insets::shown(&view)
        .into_iter()
        .find(|s| insets::hits(&view, mid(s.rect)));
    let hand = target.map(|s| {
        move_mouse(&view, mid(s.rect));
        objc2_app_kit::NSCursor::currentCursor() == objc2_app_kit::NSCursor::pointingHandCursor()
    });
    check(
        "over a hint with a place the pointer is a hand",
        hand == Some(true),
        format!("{hand:?}"),
    );
    let order: Vec<String> = view
        .subviews()
        .iter()
        .map(|sub| sub.class().name().to_string_lossy().into_owned())
        .collect();
    check(
        "the overlay is the text view's top subview",
        order.last().is_some_and(|c| c == "IDEEditorOverlay"),
        order.join(" < "),
    );
    main_after(Duration::from_secs(2), || {
        if let Some(window) = self::view().and_then(|v| v.window()) {
            window.displayIfNeeded();
            snapshot(&window, "inlay-repo-later");
        }
        finish(SCENARIO);
    });
}

/// Sends `view` the mouse moving to `point`, as the window does.
fn move_mouse(view: &EditorTextView, point: objc2_foundation::NSPoint) {
    let Some(window) = view.window() else { return };
    let at = view.convertPoint_toView(point, None);
    let event = objc2_app_kit::NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        objc2_app_kit::NSEventType::MouseMoved,
        at,
        objc2_app_kit::NSEventModifierFlags::empty(),
        0.0,
        window.windowNumber(),
        None,
        0,
        0,
        0.0,
    );
    if let Some(event) = event {
        view.mouseMoved(&event);
    }
}

fn mid(rect: NSRect) -> objc2_foundation::NSPoint {
    objc2_foundation::NSPoint::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    )
}
