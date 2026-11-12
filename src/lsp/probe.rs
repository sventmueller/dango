//! What the self-test reads from the language servers of editors.

use std::path::Path;

use ide_lsp::Status;
use objc2::DefinedClass;

use super::{State, language_of, slot};
use crate::editor::EditorTextView;

/// The diagnostics by path, for the self-test.
pub fn marks_of(view: &EditorTextView) -> Vec<(std::ops::Range<usize>, String)> {
    view.ivars()
        .lsp
        .borrow()
        .marks
        .iter()
        .map(|m| (m.range.clone(), m.message.clone()))
        .collect()
}

/// The name of the language whose server takes `path`, for the self-test.
pub fn language_name(path: &Path) -> Option<String> {
    language_of(path).map(|(language, _)| language.name().to_string())
}

/// How many semantic tokens `view` paints, for the self-test.
pub fn token_count(view: &EditorTextView) -> usize {
    view.ivars().lsp.borrow().tokens.len()
}

/// Whether the server of `view` runs, for the self-test; `Err` says what keeps it from running.
pub fn server_running(view: &EditorTextView) -> Result<bool, String> {
    let Some(root) = view.controller().and_then(|c| c.project_root()) else {
        return Err("no folder".into());
    };
    let Some(id) = view.ivars().lsp.borrow().slot else {
        return Err("no server for the document".into());
    };
    let Some(slot) = slot(&root, id) else {
        return Err("no slot".into());
    };
    match &*slot.state.borrow() {
        State::Running(server) => Ok(server.status() == Status::Running),
        State::Missing(_) => Err("missing".into()),
        State::Failed(message) => Err(message.clone()),
        State::Finding | State::Installing(..) => Ok(false),
    }
}

/// How long the server of `view` has reported no work, for the self-test; `None` while it works, before it reported
/// any, or when it does not run.
pub fn quiet_for(view: &EditorTextView) -> Option<std::time::Duration> {
    let root = view.controller().and_then(|c| c.project_root())?;
    let id = view.ivars().lsp.borrow().slot?;
    let slot = slot(&root, id)?;
    if !matches!(&*slot.state.borrow(), State::Running(server) if server.status() == Status::Running)
        || !slot.progress.borrow().is_empty()
    {
        return None;
    }
    slot.progress_at.get().map(|at| at.elapsed())
}
