//! What the scenarios about editors and their files share: waiting for a condition, typing, the editor of a tab, and
//! answering an alert as a person would.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use block2::RcBlock;
use dispatch2::MainThreadBound;
use ide_core::LockIgnoringPoison;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSModalResponse, NSTextField, NSView, NSWindow,
};
use objc2_foundation::{NSPoint, NSRange, NSRunLoop, NSRunLoopCommonModes, NSTimer};

use crate::editor::EditorTextView;
use crate::util::main_after;
use crate::window::{self, TabId, WindowController};

/// How long a condition wait gives up after.
pub(crate) const WAIT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(50);

/// Runs `then` on the main thread `delay` from now, from the main dispatch queue like `main_after`, but it may hold
/// what only the main thread can touch. Only for something that has to elapse; a step that waits for a state uses
/// `wait_until` or `wait_for`.
pub(crate) fn after(delay: Duration, then: impl FnOnce() + 'static) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let then = MainThreadBound::new(then, mtm);
    main_after(delay, move || {
        if let Some(mtm) = MainThreadMarker::new() {
            (then.into_inner(mtm))();
        }
    });
}

/// Calls `then` with whether `ready` held, as soon as it does or once `WAIT` passed.
pub(crate) fn wait_until(ready: impl Fn() -> bool + 'static, then: impl FnOnce(bool) + 'static) {
    wait_within(WAIT, ready, then);
}

/// `wait_until` with a limit of its own, for a condition that takes longer than `WAIT` or one whose failure should
/// end the scenario sooner.
pub(crate) fn wait_within(limit: Duration, ready: impl Fn() -> bool + 'static, then: impl FnOnce(bool) + 'static) {
    wait_for_within(limit, move || ready().then_some(()), move |found| then(found.is_some()));
}

/// Calls `then` with what `find` found, as soon as it finds something or with none once `WAIT` passed.
pub(crate) fn wait_for<T: 'static>(find: impl Fn() -> Option<T> + 'static, then: impl FnOnce(Option<T>) + 'static) {
    wait_for_within(WAIT, find, then);
}

/// `wait_for` with a limit of its own.
pub(crate) fn wait_for_within<T: 'static>(
    limit: Duration,
    find: impl Fn() -> Option<T> + 'static,
    then: impl FnOnce(Option<T>) + 'static,
) {
    poll(Instant::now() + limit, find, then);
}

fn poll<T: 'static>(deadline: Instant, find: impl Fn() -> Option<T> + 'static, then: impl FnOnce(Option<T>) + 'static) {
    after(POLL, move || match find() {
        Some(found) => then(Some(found)),
        None if Instant::now() > deadline => then(None),
        None => poll(deadline, find, then),
    });
}

/// Calls `then` with `root` and whether `ready` held for the window of `root`, as soon as it does or once `WAIT`
/// passed.
pub(crate) fn wait_on(
    root: PathBuf,
    ready: impl Fn(&WindowController) -> bool + 'static,
    then: impl FnOnce(PathBuf, bool) + 'static,
) {
    wait_on_within(root, WAIT, ready, then);
}

/// `wait_on` with a limit of its own.
pub(crate) fn wait_on_within(
    root: PathBuf,
    limit: Duration,
    ready: impl Fn(&WindowController) -> bool + 'static,
    then: impl FnOnce(PathBuf, bool) + 'static,
) {
    let folder = root.clone();
    wait_within(
        limit,
        move || project_window(&folder).is_some_and(|c| ready(&c)),
        move |ok| then(root, ok),
    );
}

/// The window whose folder is `root` as it is given, so that it is still found once the folder is gone.
pub(crate) fn project_window(root: &Path) -> Option<Retained<WindowController>> {
    window::open_windows()
        .into_iter()
        .find(|w| w.project().as_deref() == Some(root))
}

/// The end of a scenario's time, below the runner's limit for it in `xtask/src/selftest.rs`, so that the scenario
/// still writes its report when every wait runs to its bound.
pub(crate) struct Deadline(std::sync::OnceLock<Instant>);

impl Deadline {
    /// A deadline that has not started; until it does, waits keep only their own limits.
    pub(crate) const fn new() -> Deadline {
        Deadline(std::sync::OnceLock::new())
    }

    /// Ends the scenario's time `budget` from now.
    pub(crate) fn start(&self, budget: Duration) {
        let _ = self.0.set(Instant::now() + budget);
    }

    /// Calls `then` with whether `ready` held and how long it waited, as soon as it does or once `within` or the
    /// scenario's time passed.
    pub(crate) fn poll(
        &self,
        within: Duration,
        ready: impl Fn() -> bool + 'static,
        then: impl FnOnce(bool, Duration) + 'static,
    ) {
        let started = Instant::now();
        let limit = self
            .0
            .get()
            .map_or(within, |end| within.min(end.saturating_duration_since(started)));
        wait_within(limit, ready, move |ok| then(ok, started.elapsed()));
    }
}

/// Trusts `folder` and opens it as the user does; its root with links resolved.
pub(crate) fn open_trusted(folder: &Path) -> PathBuf {
    crate::project::trust(folder);
    if let Some(mtm) = MainThreadMarker::new() {
        crate::app::open_project(&crate::context::app(), mtm, folder.to_path_buf());
    }
    folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf())
}

/// Calls `then` with the window of `folder` once it shows, or with none once `WAIT` passed.
pub(crate) fn wait_for_window(folder: &Path, then: impl FnOnce(Option<Retained<WindowController>>) + 'static) {
    let folder = folder.to_path_buf();
    wait_for(move || window_of(&folder), then);
}

/// Calls `then` with the editor of `file` in `folder`'s window, opening it there, once it shows, or with none once
/// `WAIT` passed.
pub(crate) fn wait_for_editor(
    folder: &Path,
    file: &Path,
    then: impl FnOnce(Option<Retained<EditorTextView>>) + 'static,
) {
    let (folder, file) = (folder.to_path_buf(), file.to_path_buf());
    wait_for(
        move || {
            let controller = window_of(&folder)?;
            crate::editor::editor_for(&controller, &file).or_else(|| {
                controller.open_file(&file);
                crate::editor::editor_for(&controller, &file)
            })
        },
        then,
    );
}

/// The folder `name` in the run's home, which a scenario fills and opens.
pub(crate) fn folder(name: &str) -> PathBuf {
    ide_core::paths::home().join(name)
}

/// `folder(name)` as the app knows it, with links resolved; the folder itself when it does not exist yet.
pub(crate) fn root(name: &str) -> PathBuf {
    let folder = folder(name);
    folder.canonicalize().unwrap_or(folder)
}

/// Makes the folder `name` with `files`, given by path within it and text, and returns its root.
pub(crate) fn make_project(name: &str, files: &[(&str, &str)]) -> std::io::Result<PathBuf> {
    let folder = folder(name);
    std::fs::create_dir_all(&folder)?;
    for (path, text) in files {
        let file = folder.join(path);
        std::fs::create_dir_all(file.parent().unwrap_or(&folder))?;
        std::fs::write(file, text)?;
    }
    Ok(root(name))
}

/// Makes the folder `name` with `files`, trusts it, opens it as the user does and calls `then` with its window once it
/// shows; when it does not, that is a failed check and the end of `scenario`.
pub(crate) fn open_project(
    scenario: &'static str,
    name: &str,
    files: &[(&str, &str)],
    then: impl FnOnce(Retained<WindowController>) + 'static,
) {
    let root = match make_project(name, files) {
        Ok(root) => root,
        Err(error) => {
            return super::give_up_saying(scenario, "the scenario's folder is made", error.to_string());
        }
    };
    let root = open_trusted(&root);
    wait_for_window(&root.clone(), move |controller| match controller {
        Some(controller) => then(controller),
        None => {
            super::give_up_saying(
                scenario,
                "the scenario's folder opens in a window",
                root.display().to_string(),
            );
        }
    });
}

/// The window whose folder is `folder`.
pub(crate) fn window_of(folder: &Path) -> Option<Retained<WindowController>> {
    let root = folder.canonicalize().ok()?;
    window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&root))
}

/// Opens `folder` in a window of its own, as "In neuem Fenster öffnen" does, for a scenario that needs two folders
/// side by side; opening it as the user does would replace the folder of the window in front (E56).
pub(crate) fn open_beside(mtm: MainThreadMarker, folder: &Path) {
    let root = folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
    WindowController::open_new(&crate::context::app(), mtm, Some(root)).restore_layout();
}

/// The editor of the tab titled `title`.
pub(crate) fn editor(controller: &WindowController, title: &str) -> Option<Retained<EditorTextView>> {
    let id = tab(controller, title)?;
    crate::editor::editors_of(controller)
        .into_iter()
        .find_map(|(tab, view)| (tab == id).then_some(view))
}

/// The id of the tab titled `title`.
pub(crate) fn tab(controller: &WindowController, title: &str) -> Option<TabId> {
    let titles = controller.tab_titles();
    let index = titles.iter().position(|t| t == title)?;
    controller.tab_ids().get(index).copied()
}

/// Selects `range` of `view` and types `text` over it, a character at a time as the keyboard does, then lets an event
/// pass: AppKit closes the undo group of typing once the event loop has handled an event, and without one the next
/// undoable change would join it.
pub(crate) fn type_at(view: &EditorTextView, range: NSRange, text: &str) {
    view.setSelectedRange(range);
    for c in text.chars() {
        crate::editor::type_text(view, &c.to_string());
    }
    post_event();
}

pub(crate) fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The text of `view`.
pub(crate) fn text_of(view: &EditorTextView) -> String {
    view.document().text().to_string()
}

/// How often a call waiting for an alert looks for it.
const ALERT_POLL: Duration = Duration::from_millis(50);

/// The calls waiting for an alert, in the order they began to, and the alerts handed to one already.
struct AlertWaits {
    next: u64,
    /// Each waiting call's place in line and the message it waits for, if it waits for one.
    waiting: Vec<(u64, Option<String>)>,
    /// The window numbers of the alerts handed to a call, which ends them.
    handed: Vec<isize>,
}

static ALERT_WAITS: Mutex<AlertWaits> = Mutex::new(AlertWaits {
    next: 0,
    waiting: Vec::new(),
    handed: Vec::new(),
});

/// The messages of the alerts `answer_when_shown` answered, first first.
static ANSWERED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Calls `then` with the alert or panel that runs modal as soon as one shows that says `saying`, any when none, and
/// `then` ends it. Each alert goes to the call that began to wait first among those it suits; one that does not show
/// within `WAIT` leaves `then` uncalled. Run loop timers look for it, which fire also while an alert runs modal and
/// holds up the main dispatch queue.
pub(crate) fn when_alert_shows(saying: Option<String>, then: impl FnOnce(&NSWindow) + 'static) -> AlertWait {
    wait_for_alert(saying, true, Box::new(then))
}

/// `when_alert_shows` for a look at the alert, such as a picture, that leaves it to the next call waiting for it.
pub(crate) fn look_at_alert(saying: Option<String>, then: impl FnOnce(&NSWindow) + 'static) -> AlertWait {
    wait_for_alert(saying, false, Box::new(then))
}

/// A call that waits for an alert, which may show or not.
pub(crate) struct AlertWait(u64);

impl AlertWait {
    /// Stops waiting, once the alert can no longer show, so that the next alert goes to the call waiting after it.
    pub(crate) fn stop(self) {
        ALERT_WAITS
            .lock_ignoring_poison()
            .waiting
            .retain(|(waiting, _)| *waiting != self.0);
    }
}

fn wait_for_alert(saying: Option<String>, takes: bool, then: Box<dyn FnOnce(&NSWindow)>) -> AlertWait {
    let place = {
        let mut waits = ALERT_WAITS.lock_ignoring_poison();
        let place = waits.next;
        waits.next += 1;
        waits.waiting.push((place, saying));
        place
    };
    poll_alert(place, takes, Instant::now() + WAIT, then);
    AlertWait(place)
}

fn poll_alert(place: u64, takes: bool, deadline: Instant, then: Box<dyn FnOnce(&NSWindow)>) {
    in_any_mode_after(ALERT_POLL, move || {
        let shown = MainThreadMarker::new().and_then(|mtm| NSApplication::sharedApplication(mtm).modalWindow());
        let waiting = ALERT_WAITS
            .lock_ignoring_poison()
            .waiting
            .iter()
            .any(|(waiting, _)| *waiting == place);
        if !waiting {
            return;
        }
        if let Some(alert) = shown.filter(|alert| first_in_line(place, alert, takes)) {
            then(&alert);
        } else if Instant::now() > deadline {
            ALERT_WAITS
                .lock_ignoring_poison()
                .waiting
                .retain(|(waiting, _)| *waiting != place);
        } else {
            poll_alert(place, takes, deadline, then);
        }
    });
}

/// Whether `alert` goes to the call waiting at `place`, which then leaves the line, marking it handed if it `takes`
/// it: no call waits before it that `alert` suits, and no call took `alert` already.
fn first_in_line(place: u64, alert: &NSWindow, takes: bool) -> bool {
    let number = alert.windowNumber();
    let texts = alert_texts();
    let says = |saying: &Option<String>| saying.as_ref().is_none_or(|message| texts.contains(message));
    let mut waits = ALERT_WAITS.lock_ignoring_poison();
    let first = waits
        .waiting
        .iter()
        .find(|(_, saying)| says(saying))
        .map(|(waiting, _)| *waiting);
    if waits.handed.contains(&number) || first != Some(place) {
        return false;
    }
    waits.waiting.retain(|(waiting, _)| *waiting != place);
    if takes {
        waits.handed.push(number);
    }
    true
}

/// Answers the alert that says `message`, as soon as it shows, with its button `response`; the cell tells whether it
/// did.
pub(crate) fn answer_alert_saying(message: String, response: NSModalResponse) -> Rc<Cell<bool>> {
    answer_alert_when_shown(Some(message), response)
}

/// Answers the next alert, or the one that says `saying`, with its button `response` as soon as it shows; the cell
/// tells whether it did.
pub(crate) fn answer_alert_when_shown(saying: Option<String>, response: NSModalResponse) -> Rc<Cell<bool>> {
    let answered = Rc::new(Cell::new(false));
    let seen = Rc::clone(&answered);
    when_alert_shows(saying, move |_| {
        seen.set(true);
        answer_alert(response);
    });
    answered
}

/// Answers the next alert with `response` as soon as it shows, typing `text` into its field first when given, and
/// notes its message for `answered`.
pub(crate) fn answer_when_shown(response: NSModalResponse, text: Option<&'static str>) {
    when_alert_shows(None, move |alert| {
        if let (Some(text), Some(content)) = (text, alert.contentView()) {
            fill_field(&content, text);
        }
        let message = alert_texts().into_iter().find(|t| !t.is_empty()).unwrap_or_default();
        ANSWERED.lock_ignoring_poison().push(message);
        answer_alert(response);
    });
}

/// The messages of the alerts `answer_when_shown` answered, first first.
pub(crate) fn answered() -> Vec<String> {
    ANSWERED.lock_ignoring_poison().clone()
}

fn fill_field(view: &NSView, text: &str) -> bool {
    if let Some(field) = view.downcast_ref::<NSTextField>()
        && field.isEditable()
    {
        field.setStringValue(&crate::util::ns(text));
        return true;
    }
    view.subviews().iter().any(|child| fill_field(&child, text))
}

/// Takes a picture named `name` of the next alert as soon as it shows, drawn by AppKit: `screencapture` gets an alert
/// panel only as transparent pixels.
pub(crate) fn picture_alert(name: &'static str) {
    look_at_alert(None, move |alert| super::drawn_snapshot(alert, name));
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
pub(crate) fn answer_alert(response: NSModalResponse) {
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
pub(crate) fn post_event() {
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
pub(crate) fn list_bars(window: &objc2_app_kit::NSWindow) -> Vec<(f64, Vec<String>)> {
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
