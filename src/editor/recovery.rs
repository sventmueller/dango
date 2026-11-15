//! Copies of unsaved text against a crash (E24): written on a lane of their own once typing rests, and a few seconds into
//! steady typing at the latest, and removed once the text is saved or given up.

use std::cell::{Cell, RefCell};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ide_core::lane::Lane;
use ide_document::recovery::{Debounce, Recovery, RecoveryId};
use objc2::DefinedClass;

use super::view::EditorTextView;
use crate::util::main_after;

/// Writes the copies into the app's own data folder, apart from the disk lane, where a volume that stopped answering
/// may hold up every job behind it for minutes.
static COPIES: Lane = Lane::with_start("ide-recovery", 1, 64, ide_process::priority::background);

/// How long typing rests before a copy is written.
const QUIET: Duration = Duration::from_secs(2);
/// How long steady typing goes on before a copy is written all the same.
const LONGEST: Duration = Duration::from_secs(10);

/// The copies of unsaved text that are due and when the timer that writes them fires.
pub(crate) struct RecoveryCopies {
    due: RefCell<Debounce>,
    /// When the timer that writes the copies due fires, if one is set.
    timer: Cell<Option<Instant>>,
}

impl Default for RecoveryCopies {
    fn default() -> Self {
        RecoveryCopies {
            due: RefCell::new(Debounce::new(QUIET, LONGEST)),
            timer: Cell::new(None),
        }
    }
}

fn with_due<R>(f: impl FnOnce(&RefCell<Debounce>) -> R) -> R {
    f(&crate::context::app().editor().recovery_copies.due)
}

/// The view's text changed; its copy is written once the change is due.
pub(crate) fn changed(view: &EditorTextView) {
    let now = Instant::now();
    with_due(|due| due.borrow_mut().changed(&view.ivars().file.recovery, now));
    arm();
}

fn arm() {
    let Some(next) = with_due(|due| due.borrow().next_due()) else {
        return;
    };
    if crate::context::app()
        .editor()
        .recovery_copies
        .timer
        .get()
        .is_some_and(|set| set <= next)
    {
        return;
    }
    crate::context::app().editor().recovery_copies.timer.set(Some(next));
    main_after(next.saturating_duration_since(Instant::now()), fire);
}

fn fire() {
    crate::context::app().editor().recovery_copies.timer.set(None);
    let due = with_due(|due| due.borrow_mut().due(Instant::now()));
    if !due.is_empty() {
        for view in super::open_editors() {
            if due.contains(&view.ivars().file.recovery) {
                store(&view);
            }
        }
    }
    arm();
}

/// Writes a copy of the view's text if it has unsaved changes, else drops the copy; the writing happens on the copies'
/// lane, in the order asked for.
pub(crate) fn store(view: &EditorTextView) {
    let id = view.ivars().file.recovery.clone();
    let document = view.document();
    if !document.is_dirty() {
        drop(document);
        drop_copy(id);
        return;
    }
    let snapshot = document.snapshot();
    let title = document.path().map(|path| path.display().to_string());
    drop(document);
    let project = view.controller().and_then(|controller| controller.project());
    let job = {
        let id = id.clone();
        move || {
            let stored = Recovery::in_data_dir().store(&id, &snapshot, title.as_deref(), project.as_deref());
            if let Err(error) = stored {
                crate::diag::report("recovery copy", error);
            }
        }
    };
    if COPIES.try_run(job).is_err() {
        with_due(|due| due.borrow_mut().changed(&id, Instant::now()));
        arm();
    }
}

/// Drops the copy of a view that was saved or closed.
pub(crate) fn discard(view: &EditorTextView) {
    drop_copy(view.ivars().file.recovery.clone());
}

fn drop_copy(id: RecoveryId) {
    with_due(|due| due.borrow_mut().forget(&id));
    let job = {
        let id = id.clone();
        move || {
            if let Err(error) = Recovery::in_data_dir().discard(&id) {
                crate::diag::report("recovery copy", error);
            }
        }
    };
    if COPIES.try_run(job).is_err()
        && let Err(error) = Recovery::in_data_dir().discard(&id)
    {
        crate::diag::report("recovery copy", error);
    }
}

/// At a quit the user agreed to: drops the copies of every open editor, whose text is saved or given up, and waits up
/// to `limit` for the lane to have written and removed what it was asked to. Removals the lane has not reached by then
/// are done here instead, since a copy left behind would come back at the next launch.
pub(crate) fn quit(limit: Duration) {
    let ids: Vec<RecoveryId> = super::open_editors()
        .iter()
        .map(|view| view.ivars().file.recovery.clone())
        .collect();
    for id in &ids {
        drop_copy(id.clone());
    }
    let (done, finished) = mpsc::channel();
    let flushed = COPIES
        .try_run(move || {
            let _ = done.send(());
        })
        .is_ok()
        && finished.recv_timeout(limit).is_ok();
    if !flushed {
        crate::diag::report("recovery copy", "the lane did not finish before the quit");
        for id in &ids {
            if let Err(error) = Recovery::in_data_dir().discard(id) {
                crate::diag::report("recovery copy", error);
            }
        }
    }
}
