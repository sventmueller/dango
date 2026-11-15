//! Saving a folder's window tabs, their panes and their documents, and opening them again when the folder opens (E24,
//! E60, E145).
//!
//! Documents come back by opening what they showed: files, a new shell in the terminal's last folder, a page, an AI
//! session; a CLI thread of an older version comes back as a terminal. Terminals and AI sessions run what the folder
//! configures, so they come back only in a trusted folder (E22); a document whose file is gone stays away, and a tab left without documents with it. Editors open through
//! the caller, so a launch can put the copies a crash left in their places (`session::restore`).

use std::cell::RefCell;
use std::path::Path;

use ide_core::layout::{Layout, Tab, WindowTab};

use objc2::DefinedClass;

use super::{TabId, TabKind, WindowController};

/// A document an editor of a layout shows.
pub enum Document<'a> {
    File(&'a Path),
    /// An untitled one, by the name of its copy.
    Untitled(&'a str),
}

/// Opens a document's editor where the window's open target says; whether it did.
pub type Opener<'a> = dyn FnMut(&WindowController, Document) -> bool + 'a;

/// Saves the window tabs of `controller`'s window for its folder; a window without a folder keeps nothing.
pub fn save(controller: &WindowController) {
    let Some(root) = controller.project_root() else { return };
    if controller.ivars().closing.get() || crate::session::is_quitting() {
        return;
    }
    let selected = controller.selected_tab();
    let mut layout = Layout::default();
    for tab in controller.ordered_tabs() {
        let reopen = |document: &super::tabs::Document| {
            reopenable(controller, document.id, document.kind, document.key.as_deref())
        };
        let Some(root) = tab.panes.layout(&reopen) else {
            continue;
        };
        if tab.documents().iter().any(|document| Some(document.id) == selected) {
            layout.selected = Some(layout.tabs.len());
        }
        layout.tabs.push(WindowTab { root });
    }
    controller
        .context()
        .session()
        .write_soon("saving the window tabs", move || ide_core::layout::save(&root, &layout));
}

/// How the document `id` opens again, if it can.
fn reopenable(controller: &WindowController, id: TabId, kind: TabKind, key: Option<&str>) -> Option<Tab> {
    match kind {
        TabKind::Editor => match key {
            Some(path) => Some(Tab::Editor { path: path.into() }),
            None => crate::editor::tab_document(controller, id).map(|(_, recovery)| Tab::Untitled {
                recovery: recovery.as_str().to_string(),
            }),
        },
        TabKind::Welcome => Some(Tab::Welcome),
        TabKind::Terminal => crate::terminal::folder_of_tab(controller, id).map(|folder| Tab::Terminal { folder }),
        TabKind::Browser => crate::browser::url_of_tab(controller, id).map(|url| {
            match crate::site_preview::page_of(objc2::MainThreadOnly::mtm(controller), &url) {
                Some(path) => Tab::SitePreview { path },
                None => Tab::Browser { url },
            }
        }),
        TabKind::Agent => {
            let key = key?;
            ide_ui_agent::of_window(controller)
                .into_iter()
                .find(|tab| tab.tab_key() == key)
                .and_then(|tab| tab.session_id())
                .map(|session| Tab::Agent { session })
        }
        TabKind::Console => {
            crate::debug::console_of_tab(controller, id).map(|(title, history)| Tab::Debug { title, history })
        }
        TabKind::Diff | TabKind::Log | TabKind::Plan | TabKind::Preview => None,
    }
}

/// Opens the window tabs saved for `controller`'s folder, each editor's document through `editor`; the welcome page
/// the window started with gives way once anything opened, unless the layout has one.
pub fn restore(controller: &WindowController, editor: &mut Opener) {
    let Some(root) = controller.project_root() else { return };
    let Some(layout) = ide_core::layout::load(&root).filter(|layout| layout.document_count() > 0) else {
        return;
    };
    let welcome = controller
        .documents()
        .into_iter()
        .find(|document| document.kind == TabKind::Welcome)
        .map(|document| document.id);
    let opener = Documents {
        root: &root,
        trusted: controller.is_trusted(),
        editor: RefCell::new(editor),
    };
    let mut firsts = Vec::with_capacity(layout.tabs.len());
    for window_tab in &layout.tabs {
        firsts.push(restore_tab(controller, &opener, window_tab));
    }
    if let Some(id) = layout.selected.and_then(|index| firsts.get(index).copied().flatten()) {
        controller.select_tab_by_id(id);
    }
    if let Some(welcome) = welcome
        && controller.documents().len() > 1
        && !layout.has(&Tab::Welcome)
    {
        controller.close_tab(welcome);
    }
}

/// Opens the window tab `window_tab` with its panes, in the tab in front while that one is empty; the document in
/// front of its active pane, if any opened.
fn restore_tab(controller: &WindowController, opener: &Documents, window_tab: &WindowTab) -> Option<TabId> {
    let tab = match controller.current_tab() {
        Some(current) if current.is_empty() && current.panes.all().len() == 1 => current,
        _ => controller.new_tab_after_current(),
    };
    for (pane, saved) in tab.panes.build(&window_tab.root) {
        let mut front = None;
        for (index, document) in saved.documents.iter().enumerate() {
            let before = pane.count();
            controller.open_into(&pane, || {
                opener.open(controller, document);
            });
            if index == saved.selected && pane.count() > before {
                front = pane.front().map(|doc| doc.id);
            }
        }
        if let Some(front) = front {
            pane.select(front);
        }
    }
    tab.panes.prune();
    if tab.is_empty() {
        tab.window.close();
        return None;
    }
    tab.refresh();
    let first = tab.focused().map(|doc| doc.id);
    // A preview is not saved, as it opens with its Markdown editor when nothing else is beside it.
    if tab.documents().len() == 1
        && let Some(view) = first.and_then(|id| crate::editor::editor_of_tab(controller, id))
    {
        crate::markdown::opened(&view);
    }
    first
}

/// How the documents of a layout open.
struct Documents<'a, 'b> {
    root: &'a Path,
    trusted: bool,
    editor: RefCell<&'a mut Opener<'b>>,
}

impl Documents<'_, '_> {
    /// Opens `tab`; whether it opened.
    fn open(&self, controller: &WindowController, tab: &Tab) -> bool {
        let (root, trusted) = (self.root, self.trusted);
        match tab {
            Tab::Welcome => {
                controller.open_welcome();
                true
            }
            Tab::Editor { path } => (self.editor.borrow_mut())(controller, Document::File(path)),
            Tab::Untitled { recovery } => (self.editor.borrow_mut())(controller, Document::Untitled(recovery)),
            Tab::Terminal { folder } if trusted => {
                let folder = Some(folder.as_path()).filter(|f| f.is_dir());
                crate::terminal::open_tab(controller, folder);
                true
            }
            Tab::Browser { url } => {
                crate::browser::open_tab(controller, Some(url));
                true
            }
            Tab::SitePreview { path } if trusted && path.is_file() => {
                crate::site_preview::open(controller, path);
                true
            }
            Tab::Agent { session } if trusted => resume_agent(controller, root, session),
            // Only the history comes back; no session starts or attaches (Review C18).
            Tab::Debug { title, history } => crate::debug::reopen_console(controller, title, history.clone()),
            Tab::Cli { .. } if trusted => {
                crate::terminal::open_tab(controller, Some(root));
                true
            }
            _ => false,
        }
    }
}

/// Opens the AI tab of the session `session` again with the agent that ran it, which resumes it; a self-test plays a
/// recording instead, since it runs no agent.
fn resume_agent(controller: &WindowController, root: &Path, session: &str) -> bool {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return crate::selftest::replay_agent(controller, session);
    }
    past_session(root, session)
        .is_some_and(|(agent, resume)| ide_ui_agent::open_agent(controller, agent, Some(resume)).is_some())
}

/// The session `session` of the folder `root` and the agent that ran it. Read at once, unlike the AI panel's: the
/// restore opens the tabs in their saved order, each in its pane.
pub(crate) fn past_session(root: &Path, session: &str) -> Option<(ide_ui_agent::Agent, ide_ui_agent::Resume)> {
    ide_ui_agent::agent_panel::past_session(root, session)
}

impl WindowController {
    /// What the window shows, for the next launch: its folder, its editors and welcome pages in every tab and which
    /// one is in front. The tabs of the other documents are the layout's, which `layout::restore` opens again.
    pub fn record(&self) -> crate::session::WindowRecord {
        use crate::session::TabRecord;
        let selected = self.selected_tab();
        let mut record = crate::session::WindowRecord {
            project: self.project(),
            ..Default::default()
        };
        for document in self.documents() {
            let tab = match document.kind {
                TabKind::Welcome => TabRecord::Welcome,
                TabKind::Editor => match crate::editor::tab_document(self, document.id) {
                    Some((path, recovery)) => TabRecord::Editor {
                        path,
                        recovery: recovery.as_str().to_string(),
                    },
                    None => continue,
                },
                _ => continue,
            };
            if selected == Some(document.id) {
                record.selected = Some(record.tabs.len());
            }
            record.tabs.push(tab);
        }
        record
    }
}
