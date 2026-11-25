//! Looks at an open editor's file on the disk lane, so that a file the watcher reports, or one on a volume that stops
//! answering, never holds up typing; only what the look found comes back to the main thread.

use std::io;

use ide_document::{DiskCheck, DiskProbe};
use objc2::DefinedClass;

use super::view::EditorTextView;
use crate::context::AppContext;
use crate::util::main_soon;

/// Why the file is looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    /// The watcher or a git operation of the app's own reported it.
    Changed,
    /// The editor's text came back after a crash, and its copy is stored again after the look.
    Recovered,
}

/// Looks at the file of `view`, an editor of `cx`, if it has one, and follows what changed there once the look is done.
pub(super) fn look(cx: &AppContext, view: &EditorTextView, why: Look) {
    let Some(probe) = view.document().disk_probe() else {
        return;
    };
    let asked = view.ivars().file.disk_asked.get() + 1;
    view.ivars().file.disk_asked.set(asked);
    let serial = view.ivars().serial;
    let (sent, kept) = (probe.clone(), probe.clone());
    let queued = ide_core::lane::DISK.ask(
        move || sent.check(),
        move |check| main_soon(move || found(serial, asked, &probe, why, check)),
    );
    if queued.is_err() {
        follow(cx, view, why, Some(kept.check()));
    }
}

fn found(
    serial: ide_ui_lsp::EditorSerial,
    asked: u64,
    probe: &DiskProbe,
    why: Look,
    check: Option<io::Result<DiskCheck>>,
) {
    let cx = crate::context::app();
    let Some(view) = super::find_editor_in(&cx, serial) else {
        return;
    };
    if view.ivars().file.disk_asked.get() != asked {
        if why == Look::Recovered {
            super::recovery::store(&cx, &view);
        }
        return;
    }
    // A save or an earlier look moved the document's idea of its file since, which this look did not start from.
    if view.document().disk_probe().as_ref() != Some(probe) {
        look(&cx, &view, why);
        return;
    }
    follow(&cx, &view, why, check);
}

fn follow(cx: &AppContext, view: &EditorTextView, why: Look, check: Option<io::Result<DiskCheck>>) {
    match check {
        Some(Ok(check)) => super::conflict::follow(view, check),
        Some(Err(error)) if why == Look::Recovered => crate::diag::report("looking at a recovered file", error),
        _ => {}
    }
    if why == Look::Recovered {
        super::recovery::store(cx, view);
    }
}
