//! Copies of unsaved text against a crash (E24): written on a lane of their own once typing rests, and a few seconds into
//! steady typing at the latest, and removed once the text is saved or given up.

use std::cell::{Cell, RefCell};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ide_core::lane::Lane;
use ide_document::recovery::{Debounce, Recovery, RecoveryId};
use objc2::DefinedClass;

use super::view::EditorTextView;
use crate::context::AppContext;
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

/// The text of `view`, an editor of `cx`, changed; its copy is written once the change is due.
pub(crate) fn changed(cx: &AppContext, view: &EditorTextView) {
    let now = Instant::now();
    let copies = &cx.editor().recovery_copies;
    copies.due.borrow_mut().changed(&view.ivars().file.recovery, now);
    arm(copies);
}

fn arm(copies: &RecoveryCopies) {
    let Some(next) = copies.due.borrow().next_due() else {
        return;
    };
    if copies.timer.get().is_some_and(|set| set <= next) {
        return;
    }
    copies.timer.set(Some(next));
    main_after(next.saturating_duration_since(Instant::now()), fire);
}

fn fire() {
    let cx = crate::context::app();
    let copies = &cx.editor().recovery_copies;
    copies.timer.set(None);
    let due = copies.due.borrow_mut().due(Instant::now());
    if !due.is_empty() {
        for view in cx.editors().live() {
            if due.contains(&view.ivars().file.recovery) {
                store(&cx, &view);
            }
        }
    }
    arm(copies);
}

/// Writes a copy of the text of `view`, an editor of `cx`, if it has unsaved changes, else drops the copy; the writing
/// happens on the copies' lane, in the order asked for.
pub(crate) fn store(cx: &AppContext, view: &EditorTextView) {
    let id = view.ivars().file.recovery.clone();
    let document = view.document();
    if !document.is_dirty() {
        drop(document);
        drop_copy(&cx.editor().recovery_copies, id);
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
        let copies = &cx.editor().recovery_copies;
        copies.due.borrow_mut().changed(&id, Instant::now());
        arm(copies);
    }
}

/// Drops the copy of `view`, an editor of `cx` that was saved or closed.
pub(crate) fn discard(cx: &AppContext, view: &EditorTextView) {
    drop_copy(&cx.editor().recovery_copies, view.ivars().file.recovery.clone());
}

fn drop_copy(copies: &RecoveryCopies, id: RecoveryId) {
    copies.due.borrow_mut().forget(&id);
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

/// What a quit waits for of the copies: the copies' lane to have written and removed what it was asked to.
pub(crate) struct QuitCopies {
    ids: Vec<RecoveryId>,
    finished: Option<mpsc::Receiver<()>>,
}

/// At a quit the user agreed to: drops the copies of every open editor, whose text is saved or given up; what to wait
/// for before the app ends.
pub(crate) fn quit(cx: &crate::context::AppContext) -> QuitCopies {
    let ids: Vec<RecoveryId> = cx
        .editors()
        .live()
        .iter()
        .map(|view| view.ivars().file.recovery.clone())
        .collect();
    for id in &ids {
        drop_copy(&cx.editor().recovery_copies, id.clone());
    }
    let (done, finished) = mpsc::channel();
    let queued = COPIES.try_run(move || {
        let _ = done.send(());
    });
    QuitCopies {
        ids,
        finished: queued.is_ok().then_some(finished),
    }
}

impl QuitCopies {
    /// Waits up to `limit` for the lane, on any thread; removals it has not reached by then are done here instead,
    /// since a copy left behind would come back at the next launch.
    pub(crate) fn finish(self, limit: Duration) {
        if self
            .finished
            .is_some_and(|finished| finished.recv_timeout(limit).is_ok())
        {
            return;
        }
        crate::diag::report("recovery copy", "the lane did not finish before the quit");
        for id in &self.ids {
            if let Err(error) = Recovery::in_data_dir().discard(id) {
                crate::diag::report("recovery copy", error);
            }
        }
    }
}
