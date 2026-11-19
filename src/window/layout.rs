//! Saving a folder's window tabs, their panes and their documents, and opening them again when the folder opens (E24,
//! E60, E145).
//!
//! Documents come back by opening what they showed: files, a new shell in the terminal's last folder, a page, an AI
//! session; a CLI thread of an older version comes back as a terminal. Terminals and AI sessions run what the folder
//! configures, so they come back only in a trusted folder (E22); a document whose file is gone stays away, and a tab left without documents with it. Editors open through
//! the caller, so a launch can put the copies a crash left in their places (`session::restore`).

use std::cell::RefCell;
use std::path::Path;

use ide_core::layout::{Editor, Layout, Tab, Untitled, Welcome, WindowTab};

use objc2::DefinedClass;

use super::{TabId, TabKind, WindowController};

/// A document an editor of a layout shows.
pub(crate) enum Document<'a> {
    File(&'a Path),
    /// An untitled one, by the name of its copy.
    Untitled(&'a str),
}

/// Opens a document's editor where the window's open target says; whether it did.
pub(crate) type Opener<'a> = dyn FnMut(&WindowController, Document) -> bool + 'a;

/// Saves the window tabs of `controller`'s window for its folder; a window without a folder keeps nothing.
pub(crate) fn save(controller: &WindowController) {
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
            Some(path) => Tab::new(&Editor { path: path.into() }),
            None => crate::editor::tab_document(controller, id).and_then(|(_, recovery)| {
                Tab::new(&Untitled {
                    recovery: recovery.as_str().to_string(),
                })
            }),
        },
        TabKind::Welcome => Tab::new(&Welcome {}),
        TabKind::Terminal => crate::terminal::folder_of_tab(controller, id)
            .and_then(|folder| Tab::new(&crate::terminal::KeptTerminal { folder })),
        TabKind::Browser => {
            crate::browser::url_of_tab(controller, id).and_then(|url| {
                match crate::site_preview::page_of(objc2::MainThreadOnly::mtm(controller), &url) {
                    Some(path) => Tab::new(&crate::site_preview::KeptSite { path }),
                    None => Tab::new(&crate::browser::KeptPage { url }),
                }
            })
        }
        TabKind::Agent => {
            let key = key?;
            ide_ui_agent::of_window(controller)
                .into_iter()
                .find(|tab| tab.tab_key() == key)
                .and_then(|tab| tab.session_id())
                .and_then(|session| Tab::new(&ide_ui_agent::KeptSession { session }))
        }
        TabKind::Console => crate::debug::console_of_tab(controller, id)
            .and_then(|(title, history)| Tab::new(&crate::debug::KeptConsole { title, history })),
        TabKind::Leitstand => Tab::new(&ide_ui_factory::KeptLeitstand {}),
        TabKind::Order => ide_ui_factory::order_of_tab_key(key?).and_then(|order| {
            Tab::new(&ide_ui_factory::KeptOrder {
                order: order.to_string(),
            })
        }),
        TabKind::Diff | TabKind::Log | TabKind::Plan | TabKind::Preview => None,
    }
}

/// Opens the window tabs saved for `controller`'s folder, each editor's document through `editor`; the welcome page
/// the window started with gives way once anything opened, unless the layout has one.
pub(crate) fn restore(controller: &WindowController, editor: &mut Opener) {
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
        && !layout
            .tabs
            .iter()
            .flat_map(WindowTab::documents)
            .any(Tab::is::<Welcome>)
    {
        controller.close_tab(welcome);
    }
    controller.settle_restored();
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
            let before = tab.panes.count(pane);
            controller.open_into(&tab, pane, || {
                opener.open(controller, document);
            });
            if index == saved.selected && tab.panes.count(pane) > before {
                front = tab.panes.front(pane).map(|doc| doc.id);
            }
        }
        if let Some(front) = front {
            tab.panes.select(front);
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
        && let Some(id) = first
    {
        controller.allow_restored_preview(id);
        if let Some(view) = crate::editor::editor_of_tab(controller, id) {
            crate::markdown::opened(&view);
        }
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
    /// Opens `tab`; whether it opened. A kind no feature of this version reads opens nothing.
    fn open(&self, controller: &WindowController, tab: &Tab) -> bool {
        let (root, trusted) = (self.root, self.trusted);
        if tab.is::<Welcome>() {
            controller.open_welcome();
            return true;
        }
        if let Some(Editor { path }) = tab.read() {
            return (self.editor.borrow_mut())(controller, Document::File(&path));
        }
        if let Some(Untitled { recovery }) = tab.read() {
            return (self.editor.borrow_mut())(controller, Document::Untitled(&recovery));
        }
        if let Some(crate::browser::KeptPage { url }) = tab.read() {
            crate::browser::open_tab(controller, Some(&url));
            return true;
        }
        // The Leitstand's tabs only show what the factory knows; orders come back paused on their own (E167).
        if tab.is::<ide_ui_factory::KeptLeitstand>() {
            return crate::leitstand_ui::reopen_leitstand(controller);
        }
        if let Some(ide_ui_factory::KeptOrder { order }) = tab.read() {
            return crate::leitstand_ui::reopen_order(controller, &order);
        }
        // Only the history comes back; no session starts or attaches (Review C18).
        if let Some(crate::debug::KeptConsole { title, history }) = tab.read() {
            return crate::debug::reopen_console(controller, &title, history);
        }
        trusted && self.open_trusted(controller, root, tab)
    }

    /// Opens `tab` of a kind that runs what the folder configures; whether it opened.
    fn open_trusted(&self, controller: &WindowController, root: &Path, tab: &Tab) -> bool {
        if let Some(crate::terminal::KeptTerminal { folder }) = tab.read() {
            crate::terminal::open_tab(controller, Some(folder.as_path()).filter(|f| f.is_dir()));
            return true;
        }
        if let Some(crate::site_preview::KeptSite { path }) = tab.read()
            && path.is_file()
        {
            crate::site_preview::open(controller, &path);
            return true;
        }
        if let Some(ide_ui_agent::KeptSession { session }) = tab.read() {
            return resume_agent(controller, root, &session);
        }
        if tab.read::<crate::terminal::KeptCli>().is_some() {
            crate::terminal::open_tab(controller, Some(root));
            return true;
        }
        false
    }
}

/// Opens the AI tab of the session `session` again with the agent that ran it, which resumes it once the tab is in
/// front or gets a prompt; a self-test plays a recording instead, since it runs no agent.
fn resume_agent(controller: &WindowController, root: &Path, session: &str) -> bool {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return crate::selftest::replay_agent(controller, session);
    }
    let open = ide_ui_agent::of_window(controller)
        .iter()
        .any(|tab| tab.session_id().as_deref() == Some(session));
    open || past_session(root, session)
        .is_some_and(|(agent, resume)| ide_ui_agent::restore_agent(controller, agent, resume).is_some())
}

/// The session `session` of the folder `root` and the agent that ran it. Read at once, unlike the AI panel's: the
/// restore opens the tabs in their saved order, each in its pane.
pub(crate) fn past_session(root: &Path, session: &str) -> Option<(ide_ui_agent::Agent, ide_ui_agent::Resume)> {
    ide_ui_agent::agent_panel::past_session(root, session)
}

impl WindowController {
    /// Opens the tabs the folder's window had when it closed, as opening a folder does.
    pub(crate) fn restore_layout(&self) {
        let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
        self.restore_editor_tabs(|| {
            restore(self, &mut |controller, document| match document {
                Document::File(path) => controller.restore_editor(path, size).is_some(),
                Document::Untitled(_) => false,
            });
        });
    }

    /// `restore_layout` with `editor` opening each editor's document, and saying whether it did, so the launch can put
    /// the copies a crash left in their places (`session::restore`).
    pub(crate) fn restore_layout_with(&self, editor: &mut Opener) {
        restore(self, editor);
    }

    /// What the window shows, for the next launch: its folder, its editors and welcome pages in every tab and which
    /// one is in front. The tabs of the other documents are the layout's, which `layout::restore` opens again.
    pub(crate) fn record(&self) -> crate::session::WindowRecord {
        use crate::session::TabRecord;
        let selected = self.selected_tab();
        let mut record = crate::session::WindowRecord {
            project: self.project(),
            ..Default::default()
        };
        for document in self.documents() {
            let tab = match document.kind {
                TabKind::Welcome => TabRecord::Welcome,
                TabKind::Editor => match crate::editor::tab_document(self, document.id)
                    .or_else(|| self.restored_editor_record(document.id))
                {
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

#[cfg(test)]
mod tests;
