//! What the scenarios about editors and their files share: waiting for a condition, typing, the editor of a tab, and
//! answering an alert as a person would.

use std::cell::Cell;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSModalResponse, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRange, NSRunLoop, NSRunLoopCommonModes, NSTimer};

use crate::editor::EditorTextView;
use crate::util::main_after;
use crate::window::{self, WindowController};

const WAIT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(200);

/// Calls `then` with whether `ready` held, as soon as it does or once twenty seconds passed.
pub fn wait_until(ready: impl Fn() -> bool + Send + 'static, then: impl FnOnce(bool) + Send + 'static) {
    poll(Instant::now() + WAIT, ready, then);
}

fn poll(deadline: Instant, ready: impl Fn() -> bool + Send + 'static, then: impl FnOnce(bool) + Send + 'static) {
    main_after(POLL, move || {
        let ok = ready();
        if ok || Instant::now() > deadline {
            then(ok);
        } else {
            poll(deadline, ready, then);
        }
    });
}

/// The window whose folder is `folder`.
pub fn window_of(folder: &Path) -> Option<Retained<WindowController>> {
    let root = folder.canonicalize().ok()?;
    window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&root))
}

/// Opens `folder` in a window of its own, as "In neuem Fenster öffnen" does, for a scenario that needs two folders
/// side by side; opening it as the user does would replace the folder of the window in front (E56).
pub fn open_beside(mtm: MainThreadMarker, folder: &Path) {
    let root = folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
    WindowController::open_new(mtm, Some(root)).restore_layout();
}

/// The editor of the tab titled `title`.
pub fn editor(controller: &WindowController, title: &str) -> Option<Retained<EditorTextView>> {
    let id = tab(controller, title)?;
    crate::editor::editors_of(controller)
        .into_iter()
        .find_map(|(tab, view)| (tab == id).then_some(view))
}

/// The id of the tab titled `title`.
pub fn tab(controller: &WindowController, title: &str) -> Option<isize> {
    let titles = controller.tab_titles();
    let index = titles.iter().position(|t| t == title)?;
    controller.tab_ids().get(index).copied()
}

/// Selects `range` of `view` and types `text` over it, a character at a time as the keyboard does, then lets an event
/// pass: AppKit closes the undo group of typing once the event loop has handled an event, and without one the next
/// undoable change would join it.
pub fn type_at(view: &EditorTextView, range: NSRange, text: &str) {
    view.setSelectedRange(range);
    for c in text.chars() {
        crate::editor::type_text(view, &c.to_string());
    }
    post_event();
}

pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The text of `view`.
pub fn text_of(view: &EditorTextView) -> String {
    view.document().text().to_string()
}

/// Answers the alert that is in front `delay` from now, from a timer of the run loop: an alert opened inside a
/// callback of the main dispatch queue holds up every other such callback, but not the run loop's timers.
pub fn answer_alert_after(delay: Duration, response: NSModalResponse) {
    answer_after(delay, None, response);
}

/// `answer_alert_after` for the alert that says `message` only, so a timer meant for one alert never answers another;
/// the cell tells whether it answered.
pub fn answer_alert_saying(delay: Duration, message: String, response: NSModalResponse) -> Rc<Cell<bool>> {
    answer_after(delay, Some(message), response)
}

fn answer_after(delay: Duration, message: Option<String>, response: NSModalResponse) -> Rc<Cell<bool>> {
    let answered = Rc::new(Cell::new(false));
    let seen = Rc::clone(&answered);
    in_any_mode_after(delay, move || {
        let says = |message: &String| alert_texts().iter().any(|text| text == message);
        if message.as_ref().is_none_or(says) {
            seen.set(true);
            answer_alert(response);
        }
    });
    answered
}

/// Takes a picture named `name` of the alert in front `delay` from now, drawn by AppKit: `screencapture` gets an alert
/// panel only as transparent pixels.
pub fn picture_alert_after(delay: Duration, name: &'static str) {
    in_any_mode_after(delay, move || {
        let alert = MainThreadMarker::new().and_then(|mtm| NSApplication::sharedApplication(mtm).modalWindow());
        if let Some(alert) = alert {
            super::drawn_snapshot(&alert, name);
        }
    });
}

/// Runs `then` once, `delay` from now, also while an alert runs modal.
pub(super) fn in_any_mode_after(delay: Duration, then: impl FnOnce() + 'static) {
    let then = Cell::new(Some(then));
    let block = RcBlock::new(move |_timer: NonNull<NSTimer>| {
        if let Some(then) = then.take() {
            then();
        }
    });
    // SAFETY: The main run loop calls the block on the main thread, the only one that touches what it holds.
    let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(delay.as_secs_f64(), false, &block) };
    // SAFETY: The main run loop takes the timer in its common modes, which include the one modal alerts run in.
    unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
}

/// The texts of the alert in front.
pub(super) fn alert_texts() -> Vec<String> {
    let mut found = Vec::new();
    let mtm = MainThreadMarker::new();
    let alert = mtm.and_then(|mtm| NSApplication::sharedApplication(mtm).modalWindow());
    if let Some(content) = alert.and_then(|alert| alert.contentView()) {
        texts_in(&content, &mut found);
    }
    found
}

fn texts_in(view: &NSView, out: &mut Vec<String>) {
    if let Some(field) = view.downcast_ref::<NSTextField>() {
        out.push(field.stringValue().to_string());
    }
    for child in view.subviews().iter() {
        texts_in(&child, out);
    }
}

/// Ends the alert in front as a click on its button `response` would.
pub fn answer_alert(response: NSModalResponse) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    if app.modalWindow().is_none() {
        return;
    }
    app.stopModalWithCode(response);
    // A stop asked for outside the alert's own events takes effect with the next event.
    post_event();
}

/// Puts an empty event in front of the app's event queue.
pub fn post_event() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
        NSEventType::ApplicationDefined,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags::empty(),
        0.0,
        0,
        None,
        0,
        0,
        0,
    );
    if let Some(event) = event {
        NSApplication::sharedApplication(mtm).postEvent_atStart(&event, true);
    }
}

/// For every table in `window`, how far the buttons below it stand from it and the names of their pictures, left to
/// right; tables without buttons below are left out.
pub fn list_bars(window: &objc2_app_kit::NSWindow) -> Vec<(f64, Vec<String>)> {
    fn collect(
        view: &NSView,
        tables: &mut Vec<Retained<NSView>>,
        buttons: &mut Vec<Retained<objc2_app_kit::NSButton>>,
    ) {
        if view.isHidden() {
            return;
        }
        if let Some(scroll) = view.downcast_ref::<objc2_app_kit::NSScrollView>()
            && scroll
                .documentView()
                .is_some_and(|document| document.downcast_ref::<objc2_app_kit::NSTableView>().is_some())
        {
            tables.push(objc2::Message::retain(view));
            return;
        }
        if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>() {
            buttons.push(objc2::Message::retain(button));
            return;
        }
        for child in view.subviews().iter() {
            collect(&child, tables, buttons);
        }
    }
    let Some(content) = window.contentView() else {
        return Vec::new();
    };
    content.layoutSubtreeIfNeeded();
    let (mut tables, mut buttons) = (Vec::new(), Vec::new());
    collect(&content, &mut tables, &mut buttons);
    let frame = |view: &NSView| view.convertRect_toView(view.bounds(), None);
    tables
        .iter()
        .filter_map(|table| {
            let table = frame(table);
            let mut below: Vec<(objc2_foundation::NSRect, String)> = buttons
                .iter()
                .map(|button| {
                    let name = button
                        .image()
                        .and_then(|image| image.name())
                        .map_or_else(|| button.title().to_string(), |name| name.to_string());
                    (frame(button), name)
                })
                .filter(|(rect, _)| {
                    let gap = table.origin.y - (rect.origin.y + rect.size.height);
                    (-2.0..12.0).contains(&gap)
                        && rect.origin.x >= table.origin.x - 1.0
                        && rect.origin.x < table.origin.x + table.size.width
                })
                .collect();
            below.sort_by(|a, b| a.0.origin.x.total_cmp(&b.0.origin.x));
            let first = below.first()?.0;
            let gap = table.origin.y - (first.origin.y + first.size.height);
            Some((gap, below.into_iter().map(|(_, name)| name).collect()))
        })
        .collect()
}
