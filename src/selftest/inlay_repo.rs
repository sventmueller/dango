//! The `inlay-repo` scenario: a file of this repository under the real rust-analyzer, scrolled into its middle; every
//! gap a hint opened in the lines on screen shows its hint, and the pointer turns into a hand over a hint with a place.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2_foundation::NSRect;

use super::{check, finish, helpers, snapshot};
use crate::editor::insets::{self, Layer};
use crate::editor::{self, EditorTextView};
use crate::window;

const SCENARIO: &str = "inlay-repo";
const FILE: &str = "crates/ide-acp/src/capabilities.rs";
/// The line with closure parameters whose types rust-analyzer shows.
const LINE: usize = 146;
/// The longest rust-analyzer may take to index this repository and answer with hints.
const INDEXING: Duration = Duration::from_secs(240);
/// How often hints are asked for again while the server still indexes and answers with none.
const ASK_AGAIN: Duration = Duration::from_secs(2);
/// How long after the checks the later picture is taken, which shows whether the hints stay drawn.
const LATER: Duration = Duration::from_secs(2);

fn root() -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    folder.canonicalize().unwrap_or(folder)
}

fn view() -> Option<Retained<EditorTextView>> {
    let controller = window::for_project(&root())?;
    editor::editor_for(&controller, &root().join(FILE))
}

pub fn run() {
    helpers::open_trusted(&root());
    helpers::wait_for_editor(&root(), &root().join(FILE), |opened| {
        if opened.is_none() {
            check("the file opens", false, "");
            return finish(SCENARIO);
        }
        let asked = Cell::new(Instant::now());
        let hinted = move || {
            let Some(view) = view() else { return false };
            if !insets::of(&view, Layer::Inlay).is_empty() {
                return true;
            }
            if asked.get().elapsed() >= ASK_AGAIN {
                crate::lsp::inlay::request(&view);
                asked.set(Instant::now());
            }
            false
        };
        helpers::wait_within(INDEXING, hinted, |_| scrolled());
    });
}

/// Every gap a hint opened in the lines on screen shows its hint, with the counts for the report.
fn gaps_shown(view: &EditorTextView) -> (bool, usize, usize, usize) {
    let seen = view.visibleRect();
    let lines = editor::utf16_in(view, seen);
    let kerned_here = insets::kerns(view)
        .iter()
        .filter(|(range, _)| lines.contains(&range.start))
        .count();
    let drawn = insets::drawn_in(view, seen);
    (kerned_here > 0 && drawn >= kerned_here, kerned_here, drawn, lines.len())
}

/// Scrolls to `LINE` once rust-analyzer's hints arrived, or once it gave up on them.
fn scrolled() {
    let Some(view) = view() else {
        check("the file opens", false, "");
        return finish(SCENARIO);
    };
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
    helpers::wait_until(|| self::view().is_some_and(|v| gaps_shown(&v).0), |_| look());
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
    helpers::after(LATER, || {
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
