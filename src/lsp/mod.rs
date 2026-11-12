//! Language servers for the editors of a window (E10): started only in folders the user trusts (E22), through the
//! supervisor with the login shell's environment (E23), and told about every edit without making typing wait (E25).
//! A missing server is offered for installation in the editor's status bar, never installed silently.

mod actions;
mod agent;
mod apply;
mod complete;
mod decorate;
mod edits;
mod hierarchy;
mod hover;
pub mod inlay;
mod lease;
mod markdown;
mod navigate;
mod outline;
mod project_symbols;
mod rename;
mod signature;
pub mod status;
mod text;
mod watch;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use ide_core::settings::{Key, Settings};
use ide_document::Change;
use ide_lsp::lsp_types::{Diagnostic, MessageType, TextDocumentContentChangeEvent, Uri, WorkDoneProgress};
use ide_lsp::{Cancel, CustomServer, Event, Language, NotFound, Refresh, Registry, Server, ServerConfig, Status, uri};
use ide_process::Supervisor;
use objc2::rc::Retained;
use objc2::{DefinedClass, sel};
use splat_i18n::{t, tf};

use crate::editor::{self, EditorTextView};
use crate::util::{main_after, main_soon};

pub use actions::{Kind as ActionKind, bulb_line, place_bulb};
pub use agent::agent_diagnostics;
pub use decorate::{Severity, follow, line_marks, paint, text_range};
pub use hierarchy::{Direction, Hierarchy, HierarchyKind, call_hierarchy, kind_symbol, type_hierarchy};
pub use hover::{hover_at_cursor, hover_rest, mouse_moved, rect_of};
pub use navigate::{Goto, definition, go_to, jump_to, next_problem, references, symbols};
#[cfg(feature = "selftest")]
pub use project_symbols::held as symbol_servers;
pub use project_symbols::{FoundSymbol, SymbolAnswer, open_symbol, search_symbols};
pub use rename::{format, rename, save};

/// How long typing must pause before semantic tokens and pulled diagnostics are asked for again.
const REFRESH_DELAY: f64 = 0.4;

/// The language servers the user set up, which take their files from the built-in ones.
pub const LANGUAGE_SERVERS: Key<Vec<CustomServer>> = Key::user("languageServers", Vec::new);

/// The protocol's error for an answer the server dropped because the document changed meanwhile; asking again helps.
const CONTENT_MODIFIED: i64 = -32801;

/// What the language server side knows about one editor.
#[derive(Default)]
pub struct ViewState {
    /// Tells editors apart in callbacks from other threads, which cannot hold a view.
    pub serial: u64,
    slot: Option<u64>,
    uri: Option<Uri>,
    marks: Vec<decorate::Mark>,
    tokens: Vec<decorate::Painted>,
    snippet: Option<complete::Session>,
    status: Option<status::Bar>,
    hover: hover::State,
    squiggles: Option<objc2::rc::Weak<decorate::Squiggles>>,
    crumbs: Vec<outline::Crumb>,
    actions: actions::State,
}

/// A server for one language and project root, or what keeps it from running.
enum State {
    Finding,
    Running(Server),
    Missing(NotFound),
    Installing(Cancel, String),
    Failed(String),
}

struct Slot {
    id: u64,
    language: Language,
    root: PathBuf,
    state: RefCell<State>,
    /// Work the server reports, such as indexing, by its token: its title and the text shown for it.
    progress: RefCell<BTreeMap<String, (String, String)>>,
    /// When the server last reported progress, for the self-test to tell when it settled.
    #[cfg(feature = "selftest")]
    progress_at: std::cell::Cell<Option<std::time::Instant>>,
    /// The last message the server asked to show.
    message: RefCell<Option<String>>,
    /// When editors and the project symbol search last let go of the slot.
    hold: Cell<lease::Hold>,
    /// Whether a look at the end of the slot is pending.
    timer: Cell<bool>,
}

/// The servers of one window's folder.
struct Workspace {
    root: PathBuf,
    slots: RefCell<Vec<Rc<Slot>>>,
}

thread_local! {
    static WORKSPACES: RefCell<Vec<Rc<Workspace>>> = const { RefCell::new(Vec::new()) };
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
    static CUSTOM: RefCell<Option<Vec<Arc<CustomServer>>>> = const { RefCell::new(None) };
}

static REGISTRY: OnceLock<Arc<Registry>> = OnceLock::new();

/// The language servers the user set up, read from the settings once and again after `servers_changed`.
fn custom_servers() -> Vec<Arc<CustomServer>> {
    CUSTOM.with(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| {
                Settings::load_user()
                    .get(&LANGUAGE_SERVERS)
                    .into_iter()
                    .map(Arc::new)
                    .collect()
            })
            .clone()
    })
}

/// The language of `path` and the document's language id, the user's servers first.
fn language_of(path: &Path) -> Option<(Language, String)> {
    Language::detect(path, &custom_servers())
}

/// Looks off the main thread for the server of every built-in language, as for a file in the home folder, and hands
/// `done` each language's name with where its server is or what is missing, on the main thread.
pub fn survey(done: fn(Vec<(String, String)>)) {
    let _ = std::thread::Builder::new().name("lsp-survey".into()).spawn(move || {
        let registry = registry();
        let home = ide_core::paths::home();
        let found: Vec<(Language, Result<PathBuf, NotFound>)> = ide_lsp::BUILT_IN
            .iter()
            .map(|language| {
                let found = registry.find(language, &home, &home).map(|launch| launch.program);
                (language.clone(), found)
            })
            .collect();
        main_soon(move || {
            let lines = found
                .into_iter()
                .map(|(language, found)| (language.name().to_string(), survey_text(found)))
                .collect();
            done(lines);
        });
    });
}

fn survey_text(found: Result<PathBuf, NotFound>) -> String {
    match found {
        Ok(program) => crate::app::display_path(&program),
        Err(missing) if missing.install.is_some() && missing.missing.is_none() => {
            tf("{} fehlt – im Editor installierbar", &[&missing.program])
        }
        Err(missing) => missing_text(&missing).0,
    }
}

/// The user changed their language servers: every editor goes to the server that serves its file now.
pub fn servers_changed() {
    CUSTOM.with(|cell| cell.borrow_mut().take());
    for view in editor::all_editors() {
        opened(&view);
    }
}

fn next_id() -> u64 {
    NEXT_ID.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    })
}

/// A number that tells this editor apart from every other one while the app runs.
pub fn new_serial() -> u64 {
    next_id()
}

impl ViewState {
    /// The state of a new editor, with a serial of its own.
    pub fn new() -> ViewState {
        ViewState {
            serial: new_serial(),
            ..ViewState::default()
        }
    }
}

/// Whether the menu command `action` can run in `view`: the code commands need a running server, the problem
/// commands problems; `None` for every other command.
pub fn validate(view: &EditorTextView, action: Option<objc2::runtime::Sel>) -> Option<bool> {
    let action = action?;
    let code = [
        sel!(ideComplete:),
        sel!(ideHover:),
        sel!(ideDefinition:),
        sel!(ideReferences:),
        sel!(ideSymbols:),
        sel!(ideRename:),
        sel!(ideFormat:),
    ];
    if code.contains(&action) {
        return Some(server_of(view).is_some());
    }
    if let Some(offered) = navigate::offered(view, action) {
        return Some(offered);
    }
    if let Some(offered) = actions::validate(view, action).or_else(|| signature::validate(view, action)) {
        return Some(offered);
    }
    if action == sel!(ideNextProblem:) || action == sel!(idePreviousProblem:) {
        return Some(!view.ivars().lsp.borrow().marks.is_empty());
    }
    None
}

/// Asks for completions at the cursor; false when no server serves the editor, so the system's completion runs.
pub fn complete(view: &EditorTextView) -> bool {
    if server_of(view).is_none() {
        return false;
    }
    complete::complete(view);
    true
}

/// Text was typed into `view`: the completion list follows it.
pub fn typed(view: &EditorTextView, text: &objc2::runtime::AnyObject) {
    let typed = if let Some(s) = text.downcast_ref::<objc2_foundation::NSString>() {
        s.to_string()
    } else if let Some(s) = text.downcast_ref::<objc2_foundation::NSAttributedString>() {
        s.string().to_string()
    } else {
        return;
    };
    hover::dismiss(view);
    complete::typed(view, &typed);
    signature::typed(view, &typed);
}

/// A command of the keyboard reached `view`; whether the completion list, a snippet or the signature window took it.
pub fn command(view: &EditorTextView, command: objc2::runtime::Sel) -> bool {
    complete::command(view, command) || signature::command(view, command)
}

/// After a command the editor ran itself, such as deleting backwards: the completion list follows or closes.
pub fn after_command(view: &EditorTextView, command: objc2::runtime::Sel) {
    complete::after_command(view, command);
}

/// The cursor moved: the breadcrumbs, the code actions and the signature window follow.
pub fn selection_moved(view: &EditorTextView) {
    outline::selection_moved(view);
    actions::moved(view);
    signature::moved(view);
}

/// Lists the code actions of `kind` at the cursor of `view` in a menu there (E82).
pub fn code_actions(view: &EditorTextView, kind: ActionKind) {
    actions::open(view, kind);
}

/// Shows the signature of the call around the cursor of `view` (E84).
pub fn signature_help(view: &EditorTextView) {
    signature::show(view);
}

/// A click with ⌘ goes to the definition of what it hit; whether it was one.
pub fn command_click(view: &EditorTextView, event: &objc2_app_kit::NSEvent) -> bool {
    use objc2_app_kit::NSEventModifierFlags;
    let flags = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
    if flags != NSEventModifierFlags::Command || server_of(view).is_none() {
        return false;
    }
    let point = view.convertPoint_fromView(event.locationInWindow(), None);
    let index = view.characterIndexForInsertionAtPoint(point);
    view.setSelectedRange(objc2_foundation::NSRange::new(index, 0));
    navigate::definition_at(view, index);
    true
}

/// Puts the diagnostics' overlay over `view` and paints the server's decorations again when `scroll` shows other
/// lines.
pub fn follow_scrolling(scroll: &objc2_app_kit::NSScrollView, view: &EditorTextView) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::NSViewBoundsDidChangeNotification;
    use objc2_foundation::{NSNotification, NSNotificationCenter};

    decorate::Squiggles::attach(view);
    let clip = scroll.contentView();
    clip.setPostsBoundsChangedNotifications(true);
    let editor = objc2::rc::Weak::from(view);
    let name = crate::util::constant!(NSViewBoundsDidChangeNotification);
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        if let Some(editor) = editor.load() {
            let busy = {
                let state = editor.ivars().lsp.borrow();
                !state.marks.is_empty() || !state.tokens.is_empty()
            };
            if busy {
                editor.later(sel!(ideDecorate), 0.05);
            }
            hover::dismiss(&editor);
            signature::dismiss_for(&editor);
            inlay::scrolled(&editor);
        }
    });
    // SAFETY: The block only asks the editor to paint later; the weak reference makes it do nothing once it is gone.
    let observer = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(name),
            Some(&clip),
            None,
            &block,
        )
    };
    std::mem::forget(observer);
}

/// The registry, built once the login shell's environment is known; blocks until then, so only off the main thread.
fn registry() -> Arc<Registry> {
    REGISTRY
        .get_or_init(|| {
            let environment = crate::environment::wait(Duration::from_secs(20))
                .map(<[(String, String)]>::to_vec)
                .unwrap_or_default();
            Arc::new(Registry::new(Supervisor::global(), environment))
        })
        .clone()
}

fn workspace(root: &Path) -> Option<Rc<Workspace>> {
    WORKSPACES.with(|all| all.borrow().iter().find(|w| w.root == root).cloned())
}

fn workspace_or_new(root: &Path) -> Rc<Workspace> {
    workspace(root).unwrap_or_else(|| {
        let created = Rc::new(Workspace {
            root: root.to_path_buf(),
            slots: RefCell::new(vec![]),
        });
        WORKSPACES.with(|all| all.borrow_mut().push(created.clone()));
        created
    })
}

fn slot(root: &Path, id: u64) -> Option<Rc<Slot>> {
    workspace(root)?.slots.borrow().iter().find(|s| s.id == id).cloned()
}

/// The window folder of `view`, if the user trusts it.
fn trusted_root(view: &EditorTextView) -> Option<PathBuf> {
    let root = view.controller()?.project_root()?;
    crate::project::is_trusted(&root).then_some(root)
}

/// The slot `view` belongs to and the running server in it.
fn server_of(view: &EditorTextView) -> Option<(Server, Uri)> {
    let root = view.controller()?.project_root()?;
    let (id, uri) = {
        let state = view.ivars().lsp.borrow();
        (state.slot?, state.uri.clone()?)
    };
    let slot = slot(&root, id)?;
    let state = slot.state.borrow();
    match &*state {
        State::Running(server) => Some((server.clone(), uri)),
        _ => None,
    }
}

fn version(view: &EditorTextView) -> i32 {
    i32::try_from(view.document().revision()).unwrap_or(i32::MAX)
}

/// An editor opened, or its file got a new name: it is opened on the server of its language.
pub fn opened(view: &EditorTextView) {
    let path = view.document().path().map(Path::to_path_buf);
    let previous = view.ivars().lsp.borrow_mut().uri.take();
    if let (Some(previous), Some((server, _))) = (previous, server_of(view)) {
        server.close(&previous);
    }
    release(view);
    view.ivars().lsp.borrow_mut().slot = None;
    let Some((path, (language, language_id))) = path.and_then(|p| language_of(&p).map(|l| (p, l))) else {
        status::refresh(view);
        return;
    };
    let Some(root) = trusted_root(view) else {
        status::refresh(view);
        return;
    };
    let workspace = workspace_or_new(&root);
    let Some(slot) = slot_for(&workspace, &language, &path) else {
        status::refresh(view);
        return;
    };
    {
        let mut state = view.ivars().lsp.borrow_mut();
        state.slot = Some(slot.id);
        state.uri = Some(uri::from_path(&path));
    }
    if let State::Running(server) = &*slot.state.borrow() {
        let text = view.document().text().to_string();
        server.open(&uri::from_path(&path), &language_id, version(view), &text);
    }
    status::refresh(view);
    view.later(sel!(ideLspRefresh), REFRESH_DELAY);
}

/// The slot that serves `path`: within the folder the one of its project root, started if new; outside, as for a
/// library's source a definition led to, an existing one of the language, since nothing starts outside the folder.
fn slot_for(workspace: &Rc<Workspace>, language: &Language, path: &Path) -> Option<Rc<Slot>> {
    if !path.starts_with(&workspace.root) {
        return workspace
            .slots
            .borrow()
            .iter()
            .find(|s| s.language == *language)
            .cloned();
    }
    let root = ide_lsp::root(language, path, &workspace.root);
    Some(slot_at(workspace, language, root).0)
}

/// The slot of `language` for the project root `root`, and whether it is new: started then, unless it was there.
fn slot_at(workspace: &Rc<Workspace>, language: &Language, root: PathBuf) -> (Rc<Slot>, bool) {
    if let Some(found) = workspace
        .slots
        .borrow()
        .iter()
        .find(|s| s.language == *language && s.root == root)
    {
        return (found.clone(), false);
    }
    let slot = Rc::new(Slot {
        id: next_id(),
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
fn find(folder: &Path, slot: &Slot) {
    let (folder, id, language, root) = (folder.to_path_buf(), slot.id, slot.language.clone(), slot.root.clone());
    let _ = std::thread::Builder::new().name("lsp-find".into()).spawn(move || {
        let registry = registry();
        let found = registry
            .find(&language, &root, &folder)
            .map(|launch| registry.config(&launch, &root));
        main_soon(move || found_server(&folder, id, found));
    });
}

fn found_server(folder: &Path, id: u64, found: Result<ServerConfig, NotFound>) {
    let Some(slot) = slot(folder, id) else { return };
    let state = match found {
        Err(missing) => State::Missing(missing),
        Ok(mut config) => {
            config.client_watches_files = true;
            let label = config.label.clone();
            let root = folder.to_path_buf();
            let sink = move |event: Event| {
                let root = root.clone();
                main_soon(move || event_arrived(&root, id, event));
            };
            match Server::start(Supervisor::global(), config, sink) {
                Ok(server) => State::Running(server),
                Err(error) => State::Failed(tf("{} startet nicht: {}", &[&label, &error.to_string()])),
            }
        }
    };
    *slot.state.borrow_mut() = state;
    if let State::Running(server) = &*slot.state.borrow() {
        agent::started(&slot.root, slot.language.clone());
        for view in views_of(folder, id) {
            let Some(uri) = view.ivars().lsp.borrow().uri.clone() else {
                continue;
            };
            let Some((_, language_id)) = view.document().path().and_then(language_of) else {
                continue;
            };
            let text = view.document().text().to_string();
            server.open(&uri, &language_id, version(&view), &text);
        }
    }
    refresh_slot(folder, id);
}

/// The open editors of the window of `folder` that belong to slot `id`.
fn views_of(folder: &Path, id: u64) -> Vec<Retained<EditorTextView>> {
    editor::all_editors()
        .into_iter()
        .filter(|v| v.ivars().lsp.borrow().slot == Some(id))
        .filter(|v| v.controller().and_then(|c| c.project_root()).as_deref() == Some(folder))
        .collect()
}

fn refresh_slot(folder: &Path, id: u64) {
    for view in views_of(folder, id) {
        status::refresh(&view);
    }
    project_symbols::pump(folder);
}

/// An edit of `view`'s document: passed on to its server at once, as the protocol's incremental change.
pub fn changed(view: &EditorTextView, change: &Change) {
    {
        let mut state = view.ivars().lsp.borrow_mut();
        decorate::shift(&mut state, change);
        complete::shift(&mut state, change);
        outline::shift(&mut state, change);
    }
    if let Some((server, uri)) = server_of(view) {
        let event = TextDocumentContentChangeEvent {
            range: Some(text::change_range(change)),
            range_length: None,
            text: change.text.clone(),
        };
        if let Err(error) = server.change(&uri, version(view), &[event]) {
            crate::diag::report("telling the language server about an edit", error);
        }
        view.later(sel!(ideLspRefresh), REFRESH_DELAY);
    }
    actions::moved(view);
    if !view.ivars().lsp.borrow().marks.is_empty() || !view.ivars().lsp.borrow().tokens.is_empty() {
        view.later(sel!(ideDecorate), 0.0);
    }
}

/// The watcher of the window of `folder` saw `files` change: each running server hears of those it watches.
pub fn files_changed(folder: &Path, files: &std::collections::BTreeMap<PathBuf, ide_project::FileChange>) {
    let Some(workspace) = workspace(folder) else { return };
    for slot in workspace.slots.borrow().iter() {
        if let State::Running(server) = &*slot.state.borrow() {
            let events = watch::events(&server.watchers(), files);
            if events.is_empty() {
                continue;
            }
            match server.watched_files_changed(events) {
                Ok(()) | Err(ide_lsp::Error::NotRunning | ide_lsp::Error::ServerGone) => {}
                Err(error) => crate::diag::report("telling the language server about changed files", error),
            }
        }
    }
}

/// The document was saved.
pub fn saved(view: &EditorTextView) {
    if let Some((server, uri)) = server_of(view) {
        server.save(&uri);
    }
}

/// The editor closes.
pub fn closed(view: &EditorTextView) {
    if let Some((server, uri)) = server_of(view) {
        server.close(&uri);
    }
    release(view);
    complete::dismiss_for(view);
    hover::dismiss(view);
    signature::dismiss_for(view);
}

/// The user trusts `root` now: its editors get their servers.
pub fn trust_changed(root: &Path) {
    for view in editor::all_editors() {
        if view.controller().and_then(|c| c.project_root()).as_deref() == Some(root) {
            opened(&view);
        }
    }
}

/// The window of `root` closed: its servers stop and installations into it end.
pub fn window_closed(root: &Path) {
    let Some(workspace) = workspace(root) else { return };
    for slot in workspace.slots.borrow().iter() {
        end(slot);
    }
    WORKSPACES.with(|all| all.borrow_mut().retain(|w| w.root != root));
    project_symbols::forget(root);
}

/// Stops the slot's server or ends its installation.
fn end(slot: &Slot) {
    match &*slot.state.borrow() {
        State::Running(server) => {
            server.stop();
            agent::stopped(&slot.root, &slot.language);
        }
        State::Installing(cancel, _) => cancel.cancel(),
        _ => {}
    }
}

/// `view` no longer uses its slot: the slot goes once no editor has used it for `IDLE_STOP` and no lease holds it.
fn release(view: &EditorTextView) {
    let Some(root) = view.controller().and_then(|c| c.project_root()) else {
        return;
    };
    let Some(id) = view.ivars().lsp.borrow().slot else {
        return;
    };
    let Some(slot) = slot(&root, id) else { return };
    let mut hold = slot.hold.get();
    hold.release(std::time::Instant::now());
    slot.hold.set(hold);
    end_later(&root, &slot);
}

/// Looks, once the lease and the grace after the last editor run out, whether the slot should end.
fn end_later(folder: &Path, slot: &Slot) {
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
    let (Some(workspace), Some(slot)) = (workspace(folder), slot(folder, id)) else {
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
    end(&slot);
    workspace.slots.borrow_mut().retain(|s| s.id != id);
}

/// Asks again, after a pause in typing, for what the server computes over the whole document.
pub fn refresh(view: &EditorTextView) {
    let Some((server, uri)) = server_of(view) else {
        return;
    };
    outline::refresh(view, &server, &uri);
    inlay::request(view);
    let serial = view.ivars().lsp.borrow().serial;
    let revision = view.document().revision();
    let _ = server.semantic_tokens(&uri, move |answer| {
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            match answer {
                Ok(tokens) if view.document().revision() == revision => decorate::set_tokens(&view, &tokens),
                Err(ide_lsp::Error::Server { code, .. }) if code == CONTENT_MODIFIED => {
                    view.later(sel!(ideLspRefresh), REFRESH_DELAY);
                }
                _ => {}
            }
        });
    });
    let pulls = server.capabilities().is_some_and(|c| c.diagnostic_provider.is_some());
    if pulls {
        let _ = server.pull_diagnostics(&uri, move |answer| {
            if let Ok(diagnostics) = answer {
                main_soon(move || {
                    if let Some(view) = editor::find_editor(serial) {
                        decorate::set_diagnostics(&view, &diagnostics);
                    }
                });
            }
        });
    }
}

fn event_arrived(folder: &Path, id: u64, event: Event) {
    let Some(slot) = slot(folder, id) else { return };
    match event {
        Event::Diagnostics { uri, diagnostics, .. } => diagnostics_arrived(folder, &uri, &diagnostics),
        Event::Status(Status::Running) => {
            for view in views_of(folder, id) {
                refresh(&view);
            }
        }
        Event::Progress { token, progress } => {
            let token = match token {
                ide_lsp::lsp_types::NumberOrString::Number(n) => n.to_string(),
                ide_lsp::lsp_types::NumberOrString::String(s) => s,
            };
            #[cfg(feature = "selftest")]
            slot.progress_at.set(Some(std::time::Instant::now()));
            let mut all = slot.progress.borrow_mut();
            match progress {
                WorkDoneProgress::Begin(begin) => {
                    let text = progress_text(&begin.title, begin.message.as_deref(), begin.percentage);
                    all.insert(token, (begin.title, text));
                }
                WorkDoneProgress::Report(report) => {
                    let title = all.get(&token).map(|(title, _)| title.clone()).unwrap_or_default();
                    let text = progress_text(&title, report.message.as_deref(), report.percentage);
                    all.insert(token, (title, text));
                }
                WorkDoneProgress::End(_) => {
                    all.remove(&token);
                }
            }
        }
        Event::Message { level, message } => {
            if level == MessageType::ERROR || level == MessageType::WARNING {
                *slot.message.borrow_mut() = Some(message.clone());
            }
            crate::diag::report("language server", message);
        }
        Event::Crashed { stderr, restarting, .. } => {
            let tail: String = stderr.lines().rev().take(5).collect::<Vec<_>>().join(" | ");
            crate::diag::report("language server crashed", format!("restarting: {restarting}; {tail}"));
        }
        Event::Refresh(Refresh::SemanticTokens | Refresh::Diagnostics) => {
            for view in views_of(folder, id) {
                refresh(&view);
            }
        }
        Event::Refresh(Refresh::InlayHints) => {
            for view in views_of(folder, id) {
                inlay::request(&view);
            }
        }
        Event::ApplyEdit { edit, reply, .. } => {
            let applied = edits::apply_workspace_edit(folder, &edit);
            reply.send(ide_lsp::lsp_types::ApplyWorkspaceEditResponse {
                applied: applied.is_ok(),
                failed_change: applied
                    .as_ref()
                    .err()
                    .and_then(|f| f.index)
                    .and_then(|i| u32::try_from(i).ok()),
                failure_reason: applied.as_ref().err().map(|f| f.message.clone()),
            });
            if let Err(failure) = applied {
                actions::failed(failure.message);
            }
        }
        Event::MessageRequest {
            message,
            actions,
            reply,
            ..
        } => edits::ask(folder, &message, actions, reply),
        Event::ShowDocument {
            uri, selection, reply, ..
        } => {
            let shown = uri::to_path(&uri).is_some_and(|path| navigate::show(folder, &path, selection));
            reply.send(ide_lsp::lsp_types::ShowDocumentResult { success: shown });
        }
        Event::Status(_) | Event::Log { .. } | Event::CapabilitiesChanged | Event::Refresh(_) => {}
    }
    refresh_slot(folder, id);
}

/// A server's progress as the status bar shows it; the share only once, also when the server already wrote it.
fn progress_text(title: &str, message: Option<&str>, percentage: Option<u32>) -> String {
    let mut text = title.to_string();
    if let Some(message) = message.filter(|m| !m.is_empty()) {
        text.push_str(" – ");
        text.push_str(message);
    }
    if let Some(percentage) = percentage {
        let written = [format!("{percentage}%"), format!("{percentage} %")]
            .iter()
            .any(|share| text.contains(share.as_str()));
        if !written {
            text.push_str(&format!(" ({percentage} %)"));
        }
    }
    text
}

fn diagnostics_arrived(folder: &Path, uri: &Uri, diagnostics: &[Diagnostic]) {
    let Some(path) = uri::to_path(uri) else { return };
    agent::publish(&path, diagnostics);
    for view in editor::all_editors() {
        if view.document().path() == Some(&path)
            && view.controller().and_then(|c| c.project_root()).as_deref() == Some(folder)
        {
            decorate::set_diagnostics(&view, diagnostics);
        }
    }
}

/// What the status bar of `view` says about its server, and the action its button offers.
pub(crate) fn server_status(view: &EditorTextView) -> (String, status::Action) {
    let Some(path) = view.document().path().map(Path::to_path_buf) else {
        return (String::new(), status::Action::None);
    };
    if language_of(&path).is_none() {
        return (String::new(), status::Action::None);
    }
    let root = view.controller().and_then(|c| c.project_root());
    let found = root
        .clone()
        .zip(view.ivars().lsp.borrow().slot)
        .and_then(|(root, id)| slot(&root, id));
    if let Some(slot) = found {
        return slot_status(&slot);
    }
    let text = match root {
        None => t("Sprachserver nur in einem Ordner"),
        Some(root) if !crate::project::is_trusted(&root) => t("Sprachserver aus: Ordner nicht vertraut"),
        Some(_) => "",
    };
    (text.to_string(), status::Action::None)
}

fn slot_status(slot: &Slot) -> (String, status::Action) {
    match &*slot.state.borrow() {
        State::Finding => (
            tf("Suche Sprachserver für {} …", &[slot.language.name()]),
            status::Action::None,
        ),
        State::Missing(missing) => missing_text(missing),
        State::Installing(_, progress) => (progress.clone(), status::Action::Cancel),
        State::Failed(message) => (message.clone(), status::Action::Retry),
        State::Running(server) => {
            let name = server.label().to_string();
            match server.status() {
                Status::Starting => (tf("{} startet …", &[&name]), status::Action::None),
                Status::Restarting { .. } => (tf("{} startet neu …", &[&name]), status::Action::None),
                Status::Failed => (tf("{} ist wiederholt abgestürzt", &[&name]), status::Action::Restart),
                Status::Stopping | Status::Stopped => (tf("{} ist beendet", &[&name]), status::Action::Restart),
                Status::Running => {
                    let busy = slot.progress.borrow().values().next().map(|(_, text)| text.clone());
                    let message = slot.message.borrow().clone();
                    let text = match (busy, message) {
                        (Some(busy), _) => format!("{name}: {busy}"),
                        (None, Some(message)) => format!("{name}: {message}"),
                        (None, None) => name,
                    };
                    (text, status::Action::None)
                }
            }
        }
    }
}

fn missing_text(missing: &NotFound) -> (String, status::Action) {
    let not_on_path = |program: &str| {
        tf(
            "Kein Sprachserver für {}: {} fehlt auf dem PATH der Login-Shell",
            &[missing.language.name(), program],
        )
    };
    match (missing.missing, missing.install, missing.hint) {
        (Some(tool), ..) => (not_on_path(tool), status::Action::None),
        (None, Some(_), _) => (
            tf("{} fehlt – in den App-Ordner installieren?", &[&missing.program]),
            status::Action::Install,
        ),
        (None, None, Some(hint)) => (
            tf("{} fehlt – installieren mit „{}“", &[&missing.program, hint]),
            status::Action::None,
        ),
        (None, None, None) => (not_on_path(&missing.program), status::Action::None),
    }
}

/// The button in the status bar of `view` was pressed: install, cancel, or start the server again.
pub fn status_action(view: &EditorTextView) {
    let Some(root) = trusted_root(view) else { return };
    let Some(id) = view.ivars().lsp.borrow().slot else {
        return;
    };
    let Some(slot) = slot(&root, id) else { return };
    let action = slot_status(&slot).1;
    match action {
        status::Action::Install => install(&root, &slot),
        status::Action::Cancel => {
            if let State::Installing(cancel, _) = &*slot.state.borrow() {
                cancel.cancel();
            }
        }
        status::Action::Retry => {
            *slot.state.borrow_mut() = State::Finding;
            find(&root, &slot);
        }
        status::Action::Restart => {
            if let State::Running(server) = &*slot.state.borrow() {
                server.restart();
            }
        }
        status::Action::None => {}
    }
    refresh_slot(&root, id);
}

/// Installs the slot's missing server into the app's tools folder, after the user asked for it in the status bar.
fn install(folder: &Path, slot: &Slot) {
    let what = match &*slot.state.borrow() {
        State::Missing(NotFound {
            install: Some(what), ..
        }) => *what,
        _ => return,
    };
    let cancel = Cancel::new();
    *slot.state.borrow_mut() = State::Installing(cancel.clone(), t("Installiere …").to_string());
    let (folder, id) = (folder.to_path_buf(), slot.id);
    let _ = std::thread::Builder::new().name("lsp-install".into()).spawn(move || {
        let registry = registry();
        let environment = crate::environment::get().map(<[_]>::to_vec).unwrap_or_default();
        let installer = ide_lsp::Installer::new(
            Supervisor::global(),
            environment,
            registry.tools().to_path_buf(),
            Arc::new(ide_lsp::Http::new()),
        );
        let report = {
            let folder = folder.clone();
            move |progress: ide_lsp::Progress| {
                let folder = folder.clone();
                main_soon(move || install_progress(&folder, id, &progress));
            }
        };
        let mut report = report;
        let result = installer.install(what, &cancel, &mut report);
        main_soon(move || installed(&folder, id, result.map(|_| ()).map_err(|e| e.to_string())));
    });
}

fn install_progress(folder: &Path, id: u64, progress: &ide_lsp::Progress) {
    let Some(slot) = slot(folder, id) else { return };
    let text = match progress {
        ide_lsp::Progress::Step(step) => match step {
            ide_lsp::Step::Resolving => t("Installation: suche die neueste Version …").to_string(),
            ide_lsp::Step::Downloading => t("Installation: lade herunter …").to_string(),
            ide_lsp::Step::Verifying => t("Installation: prüfe die Prüfsumme …").to_string(),
            ide_lsp::Step::Unpacking => t("Installation: entpacke …").to_string(),
            ide_lsp::Step::Running | ide_lsp::Step::Done => t("Installiere …").to_string(),
        },
        ide_lsp::Progress::Bytes {
            done,
            total: Some(total),
        } if *total > 0 => tf(
            "Installation: lade herunter … {} %",
            &[&(done.saturating_mul(100) / total).to_string()],
        ),
        ide_lsp::Progress::Bytes { .. } => return,
        ide_lsp::Progress::Output(line) => {
            let line: String = line.chars().take(80).collect();
            tf("Installiere … {}", &[&line])
        }
    };
    if let State::Installing(_, shown) = &mut *slot.state.borrow_mut() {
        *shown = text;
    }
    refresh_slot(folder, id);
}

fn installed(folder: &Path, id: u64, result: Result<(), String>) {
    let Some(slot) = slot(folder, id) else { return };
    match result {
        Ok(()) => {
            *slot.state.borrow_mut() = State::Finding;
            find(folder, &slot);
        }
        Err(error) => {
            crate::diag::report("installing a language server", &error);
            *slot.state.borrow_mut() = State::Failed(tf("Installation fehlgeschlagen: {}", &[&error]));
        }
    }
    refresh_slot(folder, id);
}

/// Counts of the problems `view` shows, errors first, or `None` when no server watches its document.
pub(crate) fn problem_counts(view: &EditorTextView) -> Option<(usize, usize)> {
    server_of(view)?;
    let state = view.ivars().lsp.borrow();
    let errors = state.marks.iter().filter(|m| m.severity == Severity::Error).count();
    let warnings = state.marks.iter().filter(|m| m.severity == Severity::Warning).count();
    Some((errors, warnings))
}

/// The diagnostics by path, for the self-test.
#[cfg(feature = "selftest")]
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
#[cfg(feature = "selftest")]
pub fn language_name(path: &Path) -> Option<String> {
    language_of(path).map(|(language, _)| language.name().to_string())
}

/// How many semantic tokens `view` paints, for the self-test.
#[cfg(feature = "selftest")]
pub fn token_count(view: &EditorTextView) -> usize {
    view.ivars().lsp.borrow().tokens.len()
}

/// Whether the server of `view` runs, for the self-test; `Err` says what keeps it from running.
#[cfg(feature = "selftest")]
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
#[cfg(feature = "selftest")]
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

#[cfg(feature = "selftest")]
pub use actions::selftest as code_action_menu;
#[cfg(feature = "selftest")]
pub use complete::selftest as completion;
pub use edits::apply_text_edits;
#[cfg(feature = "selftest")]
pub use hover::shown_text as hover_text;
#[cfg(feature = "selftest")]
pub use rename::rename_to;
#[cfg(feature = "selftest")]
pub use signature::selftest as signatures;
#[cfg(feature = "selftest")]
pub use {navigate::close_list, navigate::listed};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_server_is_offered_for_installation_unless_its_tool_is_missing_too() {
        let missing = |missing| NotFound {
            language: Language::Rust,
            program: "rust-analyzer".into(),
            install: Some(ide_lsp::Install::RustAnalyzer),
            missing,
            hint: None,
        };
        let (text, action) = missing_text(&missing(None));
        assert_eq!(action, status::Action::Install);
        assert!(text.contains("rust-analyzer"));
        let (text, action) = missing_text(&missing(Some("go")));
        assert_eq!(action, status::Action::None, "nothing to offer without go");
        assert!(text.contains("go"));
    }

    #[test]
    fn a_server_the_app_cannot_install_names_how_to_get_it() {
        let missing = NotFound {
            language: Language::Lua,
            program: "lua-language-server".into(),
            install: None,
            missing: None,
            hint: Some("brew install lua-language-server"),
        };
        let (text, action) = missing_text(&missing);
        assert_eq!(action, status::Action::None);
        assert!(text.contains("brew install lua-language-server"), "{text}");
        let custom = NotFound { hint: None, ..missing };
        assert!(missing_text(&custom).0.contains("lua-language-server"));
    }

    #[test]
    fn progress_reads_as_title_message_and_share() {
        assert_eq!(
            progress_text("Indexing", Some("3/10"), Some(30)),
            "Indexing – 3/10 (30 %)"
        );
        assert_eq!(progress_text("Fetching", Some(""), None), "Fetching");
        assert_eq!(
            progress_text("Indexing", Some("4/21 (core) 19%"), Some(19)),
            "Indexing – 4/21 (core) 19%"
        );
        assert_eq!(progress_text("Loading 40 %", None, Some(40)), "Loading 40 %");
    }
}
