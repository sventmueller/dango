//! Opening documents on their servers, watching changes, and releasing idle servers.

use super::{
    Editor, LspState, REFRESH_DELAY, Slot, State, Workspace, actions, complete, decorate, events, host, hover,
    language_of, lease, next_id, outline, project_symbols, registry, servers_of, signature, slot, status, text,
    trusted_root, version, workspace, workspace_or_new,
};
use ide_core::settings::Settings;
use ide_document::Change;
use ide_lsp::lsp_types::TextDocumentContentChangeEvent;
use ide_lsp::{Event, Language, NotFound, Server, ServerConfig, uri};
use ide_process::Supervisor;
use ide_ui_common::util::{main_after, main_soon};
use objc2::sel;
use splat_i18n::tf;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// An editor opened, or its file got a new name: it is opened on the server of its language.
pub fn opened(view: &dyn Editor) {
    opened_in(&host::state(), view);
}

/// Opens `view` on its server, as `opened` does, with the crate's `state` at hand.
pub(crate) fn opened_in(state: &LspState, view: &dyn Editor) {
    let path = view.document().path().map(Path::to_path_buf);
    let servers = servers_of(state, view);
    let previous = view.lsp().borrow_mut().uri.take();
    if let Some(previous) = previous {
        for (_, server, _) in &servers {
            server.close(&previous);
        }
    }
    release(state, view);
    view.lsp().borrow_mut().slot = None;
    view.lsp().borrow_mut().companions.clear();
    let Some((path, (language, language_id))) = path.and_then(|p| language_of(&p).map(|l| (p, l))) else {
        status::refresh_in(state, view);
        return;
    };
    let Some(root) = trusted_root(view) else {
        status::refresh_in(state, view);
        return;
    };
    let workspace = workspace_or_new(state, &root);
    let Some(slot) = slot_for(state, &workspace, &language, &path) else {
        status::refresh_in(state, view);
        return;
    };
    {
        let mut view_state = view.lsp().borrow_mut();
        view_state.slot = Some(slot.id);
        view_state.uri = Some(uri::from_path(&path));
    }
    if let State::Running(server) = &*slot.state.borrow() {
        let text = view.document().text().to_string();
        server.open(&uri::from_path(&path), &language_id, version(view), &text);
    }
    open_companions(state, view, &workspace, &path, &language_id);
    status::refresh_in(state, view);
    view.later(sel!(ideLspRefresh), REFRESH_DELAY);
}

/// Opens `view` on the second servers its file has beside the main one (E140), started if new.
fn open_companions(state: &LspState, view: &dyn Editor, workspace: &Rc<Workspace>, path: &Path, language_id: &str) {
    if !path.starts_with(&workspace.root) {
        return;
    }
    for companion in ide_lsp::companions(path, &workspace.root, emmet_on()) {
        let root = ide_lsp::root(&companion, path, &workspace.root);
        let (slot, _) = slot_at(state, workspace, &companion, root);
        view.lsp()
            .borrow_mut()
            .companions
            .push((slot.id, language_id.to_string()));
        if let State::Running(server) = &*slot.state.borrow() {
            let text = view.document().text().to_string();
            server.open(&uri::from_path(path), language_id, version(view), &text);
        }
    }
}

/// Whether Emmet's abbreviations show in completion (E109).
fn emmet_on() -> bool {
    Settings::load_user().get(&ide_core::settings::keys::EMMET)
}

/// The slot that serves `path`: within the folder the one of its project root, started if new; outside, as for a
/// library's source a definition led to, an existing one of the language, since nothing starts outside the folder.
fn slot_for(state: &LspState, workspace: &Rc<Workspace>, language: &Language, path: &Path) -> Option<Rc<Slot>> {
    if !path.starts_with(&workspace.root) {
        return workspace
            .slots
            .borrow()
            .iter()
            .find(|s| s.language == *language)
            .cloned();
    }
    let root = ide_lsp::root(language, path, &workspace.root);
    Some(slot_at(state, workspace, language, root).0)
}

/// The slot of `language` for the project root `root`, and whether it is new: started then, unless it was there.
pub(super) fn slot_at(
    state: &LspState,
    workspace: &Rc<Workspace>,
    language: &Language,
    root: PathBuf,
) -> (Rc<Slot>, bool) {
    if let Some(found) = workspace
        .slots
        .borrow()
        .iter()
        .find(|s| s.language == *language && s.root == root)
    {
        return (found.clone(), false);
    }
    let slot = Rc::new(Slot {
        id: next_id(state),
        language: language.clone(),
        root,
        state: RefCell::new(State::Finding),
        progress: RefCell::new(BTreeMap::new()),
        #[cfg(feature = "selftest")]
        progress_at: std::cell::Cell::new(None),
        message: RefCell::new(None),
        hold: Cell::new(lease::Hold::default()),
        timer: Cell::new(false),
    });
    workspace.slots.borrow_mut().push(slot.clone());
    find(&workspace.root, &slot);
    (slot, true)
}

/// Looks for the slot's server off the main thread and starts it when found.
pub(super) fn find(folder: &Path, slot: &Slot) {
    let (folder, id, language, root) = (folder.to_path_buf(), slot.id, slot.language.clone(), slot.root.clone());
    if let Err(why) = ide_ui_common::util::spawn("lsp-find", move || {
        let registry = registry();
        let found = registry
            .find(&language, &root, &folder)
            .map(|launch| registry.config(&launch, &root));
        main_soon(move || found_server(&folder, id, found));
    }) {
        *slot.state.borrow_mut() = State::Failed(why.0);
    }
}

/// The search for the server of slot `id` of the window of `folder` ended: the slot starts what was `found`, or keeps
/// what is missing, and its editors open on a running server.
fn found_server(folder: &Path, id: u64, found: Result<ServerConfig, NotFound>) {
    let lsp = host::state();
    let Some(slot) = slot(&lsp, folder, id) else { return };
    let state = match found {
        Err(missing) => State::Missing(missing),
        Ok(mut config) => {
            config.client_watches_files = true;
            let label = config.label.clone();
            let root = folder.to_path_buf();
            let sink = move |event: Event| {
                let root = root.clone();
                main_soon(move || events::event_arrived(&root, id, event));
            };
            match Server::start(Supervisor::global(), config, sink) {
                Ok(server) => State::Running(server),
                Err(error) => State::Failed(tf("{} startet nicht: {}", &[&label, &error.to_string()])),
            }
        }
    };
    *slot.state.borrow_mut() = state;
    if let State::Running(server) = &*slot.state.borrow() {
        lsp.lsp_diagnostics.started(&slot.root, slot.language.clone());
        for view in views_of(folder, id) {
            let Some(uri) = view.lsp().borrow().uri.clone() else {
                continue;
            };
            let companion = view
                .lsp()
                .borrow()
                .companions
                .iter()
                .find(|(slot, _)| *slot == id)
                .map(|(_, language_id)| language_id.clone());
            let Some(language_id) =
                companion.or_else(|| view.document().path().and_then(language_of).map(|(_, id)| id))
            else {
                continue;
            };
            let text = view.document().text().to_string();
            server.open(&uri, &language_id, version(&*view), &text);
        }
    }
    refresh_slot(&lsp, folder, id);
}

/// The open editors of the window of `folder` that belong to slot `id`.
pub(super) fn views_of(folder: &Path, id: u64) -> Vec<Box<dyn Editor>> {
    host::all_editors()
        .into_iter()
        .filter(|v| {
            let state = v.lsp().borrow();
            state.slot == Some(id) || state.companions.iter().any(|(slot, _)| *slot == id)
        })
        .filter(|v| v.project_root().as_deref() == Some(folder))
        .collect()
}

/// The status bars of the editors of slot `id` in the window of `folder` follow it, and the symbol search there.
pub(super) fn refresh_slot(state: &LspState, folder: &Path, id: u64) {
    for view in views_of(folder, id) {
        status::refresh_in(state, &*view);
    }
    project_symbols::pump(state, folder);
}

/// An edit of `view`'s document: passed on to its server at once, as the protocol's incremental change.
pub fn changed(view: &dyn Editor, change: &Change) {
    {
        let mut state = view.lsp().borrow_mut();
        decorate::shift(&mut state, change);
        complete::shift(&mut state, change);
        outline::shift(&mut state, change);
    }
    let state = host::state();
    let servers = servers_of(&state, view);
    for (_, server, uri) in &servers {
        let event = TextDocumentContentChangeEvent {
            range: Some(text::change_range(change)),
            range_length: None,
            text: change.text.clone(),
        };
        if let Err(error) = server.change(uri, version(view), &[event]) {
            ide_ui_common::diag::report("telling the language server about an edit", error);
        }
    }
    if !servers.is_empty() {
        view.later(sel!(ideLspRefresh), REFRESH_DELAY);
    }
    actions::moved(&state, view);
    if !view.lsp().borrow().marks.is_empty() || !view.lsp().borrow().tokens.is_empty() {
        view.later(sel!(ideDecorate), 0.0);
    }
}

/// The watcher of the window of `folder` saw `files` change: each running server hears of those it watches, which
/// are picked off the main thread.
pub fn files_changed(
    folder: &Path,
    files: &std::sync::Arc<std::collections::BTreeMap<PathBuf, ide_project::FileChange>>,
) {
    let Some(workspace) = workspace(&host::state(), folder) else {
        return;
    };
    for slot in workspace.slots.borrow().iter() {
        if let State::Running(server) = &*slot.state.borrow() {
            match server.files_changed(files.clone()) {
                Ok(()) | Err(ide_lsp::Error::NotRunning | ide_lsp::Error::ServerGone) => {}
                Err(error) => ide_ui_common::diag::report("telling the language server about changed files", error),
            }
        }
    }
}

/// The document was saved.
pub fn saved(view: &dyn Editor) {
    for (_, server, uri) in servers_of(&host::state(), view) {
        server.save(&uri);
    }
}

/// The editor closes.
pub fn closed(view: &dyn Editor) {
    let state = host::state();
    for (_, server, uri) in servers_of(&state, view) {
        server.close(&uri);
    }
    release(&state, view);
    complete::dismiss_for(&state.completion, view);
    hover::dismiss(view);
    signature::dismiss_for(&state.signature_help, view);
}

/// The user trusts `root` now: its editors get their servers.
pub fn trust_changed(root: &Path) {
    let state = host::state();
    for view in host::all_editors() {
        if view.project_root().as_deref() == Some(root) {
            opened_in(&state, &*view);
        }
    }
}

/// The window of `root` closed: its servers stop and installations into it end.
pub fn window_closed(root: &Path) {
    let state = host::state();
    let Some(workspace) = workspace(&state, root) else {
        return;
    };
    for slot in workspace.slots.borrow().iter() {
        end(&state, slot);
    }
    state.lsp_workspaces.workspaces.borrow_mut().retain(|w| w.root != root);
    project_symbols::forget(&state.server_leases, root);
}

/// Stops the slot's server, whose diagnostics then leave `state`, or ends its installation.
fn end(state: &LspState, slot: &Slot) {
    match &*slot.state.borrow() {
        State::Running(server) => {
            server.stop();
            state.lsp_diagnostics.stopped(&slot.root, &slot.language);
        }
        State::Installing(cancel, _) => cancel.cancel(),
        _ => {}
    }
}

/// `view` no longer uses its slot: the slot goes once no editor has used it for `IDLE_STOP` and no lease holds it.
fn release(state: &LspState, view: &dyn Editor) {
    let Some(root) = view.project_root() else {
        return;
    };
    let ids: Vec<u64> = {
        let view_state = view.lsp().borrow();
        view_state
            .slot
            .into_iter()
            .chain(view_state.companions.iter().map(|(id, _)| *id))
            .collect()
    };
    for id in ids {
        let Some(slot) = slot(state, &root, id) else { continue };
        let mut hold = slot.hold.get();
        hold.release(std::time::Instant::now());
        slot.hold.set(hold);
        end_later(&root, &slot);
    }
}

/// Looks, once the lease and the grace after the last editor run out, whether the slot should end.
pub(super) fn end_later(folder: &Path, slot: &Slot) {
    if slot.timer.replace(true) {
        return;
    }
    let now = std::time::Instant::now();
    let wait = slot.hold.get().next_check(now).saturating_duration_since(now);
    let (folder, id) = (folder.to_path_buf(), slot.id);
    main_after(wait, move || end_if_unused(&folder, id));
}

/// Ends slot `id` of the window of `folder` unless an editor or a lease holds it, or it installs.
fn end_if_unused(folder: &Path, id: u64) {
    let state = host::state();
    let (Some(workspace), Some(slot)) = (workspace(&state, folder), slot(&state, folder, id)) else {
        return;
    };
    slot.timer.set(false);
    let installing = matches!(&*slot.state.borrow(), State::Installing(..));
    let editors = views_of(folder, id).len();
    if installing || editors > 0 {
        return;
    }
    if !slot.hold.get().due(std::time::Instant::now(), editors) {
        end_later(folder, &slot);
        return;
    }
    end(&state, &slot);
    workspace.slots.borrow_mut().retain(|s| s.id != id);
}
