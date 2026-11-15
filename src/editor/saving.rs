//! Writing an editor's document to its file. Every write runs on the disk lane, one after the other, so a volume that
//! stops answering holds the lane and not the window: ⌘S returns at once and the editor takes note when the write is
//! done, while closing and quitting wait for theirs, since what follows depends on it.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use ide_core::explain::Explain;
use ide_document::{SaveError, SaveJob, Saved};
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSModalResponseOK, NSSavePanel};
use splat_i18n::{t, tf};

use super::{EditorTextView, alert, confirm, display_name, indent};
use crate::util::{main_soon, ns};

/// Whether a write of the editor's document runs on the disk lane, and whether ⌘S asked for another meanwhile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Writing {
    #[default]
    Idle,
    Running,
    /// Running, and the document saves again once it is done.
    Again,
}

/// Whether the caller waits for the write or goes on at once.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wait {
    Done,
    Not,
}

/// Saves `view` and waits until the file is written, for the steps that depend on it, such as closing; `save_as`
/// always asks for a file.
pub(crate) fn save(view: &EditorTextView, save_as: bool) {
    let Some(target) = target(view, save_as) else { return };
    // A write still running holds what the document knows of the file; this one waits behind it and replaces it.
    let overwrite = view.ivars().writing.get() != Writing::Idle;
    if let Some(job) = begin(view, target.as_deref(), overwrite) {
        finish(view, target, wait_for(job), Wait::Done);
    }
}

/// Saves `view` as ⌘S does: the write runs on the disk lane, and the editor takes note when it is done.
pub(crate) fn save_soon(view: &EditorTextView, save_as: bool) {
    let writing = view.ivars().writing.get();
    if writing != Writing::Idle {
        if save_as {
            return save(view, true);
        }
        view.ivars().writing.set(Writing::Again);
        return;
    }
    let Some(target) = target(view, save_as) else { return };
    if let Some(job) = begin(view, target.as_deref(), false) {
        write_soon(view, job, target);
    }
}

/// The file to save to: `Some` with a new one the user picked, `None` for the document's own; `None` outside when the
/// user cancelled the pick.
#[expect(clippy::option_option, reason = "cancelled, the document's own file, or a new one")]
fn target(view: &EditorTextView, save_as: bool) -> Option<Option<PathBuf>> {
    let target = if save_as || view.document().path().is_none() {
        Some(ask_path(view)?)
    } else {
        None
    };
    indent::before_save(view, target.as_deref());
    Some(target)
}

fn begin(view: &EditorTextView, target: Option<&Path>, overwrite: bool) -> Option<SaveJob> {
    let job = match target {
        Some(path) => view.document().begin_save_as(path.to_path_buf()),
        None => view.document().begin_save(overwrite),
    };
    job.map_err(|error| failed(view, &error)).ok()
}

fn write_soon(view: &EditorTextView, job: SaveJob, target: Option<PathBuf>) {
    let serial = view.ivars().serial;
    let (pending, kept) = (job.clone(), target.clone());
    let queued = ide_core::lane::DISK.ask(
        move || job.run(),
        move |written| main_soon(move || written_soon(serial, target, written)),
    );
    match queued {
        Ok(()) => view.ivars().writing.set(Writing::Running),
        Err(_) => finish(view, kept, Some(pending.run()), Wait::Done),
    }
}

/// The write that ⌘S started for the editor `serial` is done; `None` if it panicked.
fn written_soon(serial: ide_ui_lsp::EditorSerial, target: Option<PathBuf>, written: Option<Result<Saved, SaveError>>) {
    let Some(view) = super::find_editor(serial) else { return };
    let again = view.ivars().writing.replace(Writing::Idle) == Writing::Again;
    finish(&view, target, written, Wait::Not);
    if again && view.document().is_dirty() {
        save_soon(&view, false);
    }
}

/// Runs `job` on the disk lane behind every write before it and waits for it; on this thread if the lane is full.
fn wait_for(job: SaveJob) -> Option<Result<Saved, SaveError>> {
    let (sent, received) = mpsc::channel();
    let pending = job.clone();
    match ide_core::lane::DISK.ask(move || job.run(), move |written| drop(sent.send(written))) {
        Ok(()) => received.recv().ok().flatten(),
        Err(_) => Some(pending.run()),
    }
}

fn finish(view: &EditorTextView, target: Option<PathBuf>, written: Option<Result<Saved, SaveError>>, wait: Wait) {
    match written {
        Some(Ok(done)) => {
            view.document_mut().end_save(done);
            saved(view, target.as_deref());
        }
        Some(Err(SaveError::ChangedOnDisk)) => {
            let name = view.document().path().map(display_name).unwrap_or_default();
            let agreed = confirm(
                view.mtm(),
                &tf("„{}“ wurde inzwischen außerhalb geändert.", &[&name]),
                t("Sichern überschreibt diese Änderungen."),
                t("Überschreiben"),
            );
            if agreed && let Some(job) = begin(view, None, true) {
                match wait {
                    Wait::Done => finish(view, None, wait_for(job), Wait::Done),
                    Wait::Not => write_soon(view, job, None),
                }
            }
        }
        Some(Err(error)) => failed(view, &error),
        None => alert(
            view.mtm(),
            t("Sichern fehlgeschlagen"),
            t("Das Schreiben der Datei ist abgebrochen."),
        ),
    }
}

fn failed(view: &EditorTextView, error: &SaveError) {
    alert(view.mtm(), t("Sichern fehlgeschlagen"), &error.explain());
}

fn saved(view: &EditorTextView, new_path: Option<&Path>) {
    super::update_dirty(view);
    super::recovery::discard(view);
    super::conflict::saved(view);
    if new_path.is_some() {
        crate::session::changed();
    }
    if let (Some(path), Some(controller)) = (new_path, view.controller()) {
        if let Some(tab) = view.ivars().tab.get() {
            controller.set_tab_title(tab, &super::tab_title(path));
        }
        let language = ide_document::syntax::Language::detect(Some(path), "");
        view.document_mut().set_language(language);
        super::highlight::recolor_all(view);
        ide_ui_lsp::opened(view);
        indent::configure(view);
        super::changes::reload(view, true);
    }
    ide_ui_lsp::saved(view);
    if let (Some(path), Some(controller)) = (view.document().path().map(Path::to_path_buf), view.controller()) {
        crate::worktrees::file_saved(&controller, &path);
    }
}

fn ask_path(view: &EditorTextView) -> Option<PathBuf> {
    let panel = NSSavePanel::savePanel(view.mtm());
    if let Some(root) = view.controller().and_then(|c| c.project_root()) {
        panel.setDirectoryURL(Some(&objc2_foundation::NSURL::fileURLWithPath(&ns(
            &root.to_string_lossy()
        ))));
    }
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    panel
        .URL()
        .and_then(|url| url.path())
        .map(|p| PathBuf::from(p.to_string()))
}
