//! Reading on the disk lane what an editor needs from a file: the main thread waits a frame at most, so a slow volume
//! never holds it up; what comes later is taken when it comes, and what wanted the editor of a file gets it then.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dispatch2::MainThreadBound;
use ide_core::lane::{Answer, DISK};
use ide_document::{DiskCheck, Document};
use ide_process::signpost;
use ide_ui_common::window::WindowId;
use objc2::rc::{Retained, Weak};
use objc2::{MainThreadMarker, MainThreadOnly};

use super::{EditorTextView, conflict, opened};
use crate::window::WindowController;

/// How long the main thread waits for a read, a frame at 120 Hz: an answer within it opens the tab at once, as
/// callers that use the editor right after opening expect.
const BUDGET: Duration = Duration::from_millis(8);

type Then = Box<dyn FnOnce(&EditorTextView)>;

/// The files whose read for a tab still runs, with what waits for their editor.
#[derive(Default)]
pub(crate) struct Reads {
    running: RefCell<Vec<Read>>,
}

struct Read {
    window: WindowId,
    path: PathBuf,
    waiting: Vec<Then>,
}

impl Reads {
    fn is_running(&self, window: WindowId, path: &Path) -> bool {
        self.running
            .borrow()
            .iter()
            .any(|r| r.window == window && r.path == path)
    }

    fn start(&self, window: WindowId, path: PathBuf) {
        self.running.borrow_mut().push(Read {
            window,
            path,
            waiting: Vec::new(),
        });
    }

    /// Queues `then` for the editor of `path` in `window` if its read runs; gives it back otherwise.
    fn wait(&self, window: WindowId, path: &Path, then: Then) -> Result<(), Then> {
        let mut running = self.running.borrow_mut();
        match running.iter_mut().find(|r| r.window == window && r.path == path) {
            Some(read) => {
                read.waiting.push(then);
                Ok(())
            }
            None => Err(then),
        }
    }

    /// Ends the read of `path` in `window`, with what waited for it.
    fn finish(&self, window: WindowId, path: &Path) -> Vec<Then> {
        let mut running = self.running.borrow_mut();
        let at = running.iter().position(|r| r.window == window && r.path == path);
        at.map(|at| running.remove(at).waiting).unwrap_or_default()
    }
}

/// Reads `path` on the disk lane and opens its tab: now if the read is quick, else once it is done. A second open
/// while the read runs starts no second one, so the file still gets one document (E24).
pub(super) fn open(controller: &WindowController, path: &Path, font_size: f64, preview: bool) {
    let window = controller.id();
    let reads = &controller.context().editor().reads;
    if reads.is_running(window, path) {
        return;
    }
    let read = path.to_path_buf();
    let answer = DISK.ask_within(BUDGET, move || {
        let _interval = signpost::begin(signpost::name!("open read"));
        Document::open(&read)
    });
    match answer {
        Ok(Answer::Now(document)) => opened(controller, path, finished(document), font_size, preview),
        Ok(Answer::Later(pending)) => {
            reads.start(window, path.to_path_buf());
            let path = path.to_path_buf();
            pending.then(move |document| {
                crate::util::main_soon(move || open_late(window, &path, finished(document), font_size, preview));
            });
        }
        Err(_) => opened(controller, path, Document::open(path), font_size, preview),
    }
}

fn finished(document: Option<std::io::Result<Document>>) -> std::io::Result<Document> {
    document.unwrap_or_else(|| Err(std::io::ErrorKind::Interrupted.into()))
}

fn open_late(window: WindowId, path: &Path, document: std::io::Result<Document>, font_size: f64, preview: bool) {
    let Some(controller) = crate::host::window(window) else {
        return;
    };
    let waiting = controller.context().editor().reads.finish(window, path);
    if controller.tab_with_key(&path.to_string_lossy()).is_some() {
        controller.open_file(path);
    } else {
        opened(&controller, path, document, font_size, preview);
    }
    if let Some(view) = editor_in(&controller, path) {
        for then in waiting {
            then(&view);
        }
    }
}

/// Runs `then` with the editor of `path` in `controller`: now, or once its tab opens after its read.
pub(crate) fn with_opened(controller: &WindowController, path: &Path, then: impl FnOnce(&EditorTextView) + 'static) {
    let Err(then) = controller.when_restored_editor(path, then) else {
        return;
    };
    let reads = &controller.context().editor().reads;
    if let Err(then) = reads.wait(controller.id(), path, Box::new(then))
        && let Some(view) = editor_in(controller, path)
    {
        then(&view);
    }
}

fn editor_in(controller: &WindowController, path: &Path) -> Option<Retained<EditorTextView>> {
    controller
        .context()
        .editors()
        .live()
        .into_iter()
        .find(|v| v.document().path() == Some(path) && v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
}

/// Takes what changed in the file of `view`, then runs `then` with it: now if the look at the file is quick, else
/// once it is done, if the editor is still open.
pub(super) fn take_disk(view: &EditorTextView, then: impl FnOnce(&EditorTextView) + Send + 'static) {
    let Some(probe) = view.document().disk_probe() else {
        return then(view);
    };
    let look = probe.clone();
    match DISK.ask_within(BUDGET, move || look.check()) {
        Ok(Answer::Now(check)) => follow(view, check.and_then(Result::ok), then),
        Ok(Answer::Later(pending)) => {
            let editor = MainThreadBound::new(Weak::from(view), view.mtm());
            pending.then(move |check| {
                crate::util::main_soon(move || {
                    if let Some(view) = MainThreadMarker::new().and_then(|mtm| editor.get(mtm).load()) {
                        follow(&view, check.and_then(Result::ok), then);
                    }
                });
            });
        }
        Err(_) => follow(view, probe.check().ok(), then),
    }
}

fn follow(view: &EditorTextView, check: Option<DiskCheck>, then: impl FnOnce(&EditorTextView)) {
    if let Some(check) = check {
        conflict::follow(view, check);
    }
    then(view);
}
