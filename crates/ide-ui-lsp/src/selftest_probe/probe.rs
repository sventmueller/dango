//! What the self-test reads from the language servers of editors.

use std::path::Path;

use ide_lsp::Status;

use crate::editor::Editor;
use crate::{State, language_of, slot};

/// The diagnostics by path, for the self-test.
pub fn marks_of(view: &dyn Editor) -> Vec<(std::ops::Range<usize>, String)> {
    view.lsp()
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
pub fn token_count(view: &dyn Editor) -> usize {
    view.lsp().borrow().tokens.len()
}

/// Whether the server of `view` runs, for the self-test; `Err` says what keeps it from running.
pub fn server_running(view: &dyn Editor) -> Result<bool, String> {
    let Some(root) = view.project_root() else {
        return Err("no folder".into());
    };
    let Some(id) = view.lsp().borrow().slot else {
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
pub fn quiet_for(view: &dyn Editor) -> Option<std::time::Duration> {
    let root = view.project_root()?;
    let id = view.lsp().borrow().slot?;
    let slot = slot(&root, id)?;
    if !matches!(&*slot.state.borrow(), State::Running(server) if server.status() == Status::Running)
        || !slot.progress.borrow().is_empty()
    {
        return None;
    }
    slot.progress_at.get().map(|at| at.elapsed())
}

/// The state of the second server `name` of `view`, such as ESLint, for the self-test: `Ok(true)` once it runs.
pub fn companion_running(view: &dyn Editor, name: &str) -> Result<bool, String> {
    let root = view.project_root().ok_or("no folder")?;
    let ids: Vec<u64> = view.lsp().borrow().companions.iter().map(|(id, _)| *id).collect();
    let slot = ids
        .into_iter()
        .filter_map(|id| slot(&root, id))
        .find(|slot| slot.language.name() == name)
        .ok_or_else(|| format!("no {name} beside the file"))?;
    let state = slot.state.borrow();
    match &*state {
        State::Running(server) => Ok(server.status() == Status::Running),
        State::Missing(_) => Err("missing".into()),
        State::Failed(message) => Err(message.clone()),
        State::Finding | State::Installing(..) => Ok(false),
    }
}

/// Installs every missing second server of `view` without asking, as "Installieren" does, for the self-test.
pub fn install_companions(view: &dyn Editor) {
    let Some(root) = view.project_root() else {
        return;
    };
    let ids: Vec<u64> = view.lsp().borrow().companions.iter().map(|(id, _)| *id).collect();
    for id in ids {
        if let Some(slot) = slot(&root, id)
            && matches!(&*slot.state.borrow(), State::Missing(_))
        {
            crate::install::install_now(&root, &slot);
        }
    }
}
