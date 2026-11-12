//! Saving a folder's window tabs and their documents, and opening them again when the folder opens (E24, E60).
//!
//! Documents come back by opening what they showed: files, a new shell in the terminal's last folder, a page, an AI
//! session; a CLI thread of an older version comes back as a terminal. Terminals and AI sessions run what the folder
//! configures, so they come back only in a trusted folder (E22); a document whose file is gone stays away, and a tab left without documents with it. Editors open through
//! the caller, so a launch can put the copies a crash left in their places (`session::restore`).

use std::cell::RefCell;
use std::path::Path;

use ide_core::layout::{Layout, Tab, WindowTab};

use objc2::DefinedClass;

use super::{TabKind, WindowController};

/// A document an editor of a layout shows.
pub enum Document<'a> {
    File(&'a Path),
    /// An untitled one, by the name of its copy.
    Untitled(&'a str),
}

/// Opens a document's editor in a tab, or beside the document `open_beside` names; whether it did.
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
        let documents = tab.documents();
        let reopened: Vec<Tab> = documents
            .iter()
            .filter_map(|document| reopenable(controller, document.id, document.kind, document.key.as_deref()))
            .collect();
        if reopened.is_empty() {
            continue;
        }
        if documents.iter().any(|document| Some(document.id) == selected) {
            layout.selected = Some(layout.tabs.len());
        }
        layout.tabs.push(WindowTab {
            share: tab.share().unwrap_or(0.5),
            documents: reopened,
        });
    }
    if let Err(error) = ide_core::layout::save(&root, &layout) {
        crate::diag::report("saving the window tabs", error);
    }
}

/// How the document `id` opens again, if it can.
fn reopenable(controller: &WindowController, id: isize, kind: TabKind, key: Option<&str>) -> Option<Tab> {
    match kind {
        TabKind::Editor => match key {
            Some(path) => Some(Tab::Editor { path: path.into() }),
            None => crate::editor::tab_document(controller, id).map(|(_, recovery)| Tab::Untitled {
                recovery: recovery.as_str().to_string(),
            }),
        },
        TabKind::Welcome => Some(Tab::Welcome),
        TabKind::Terminal => crate::terminal::folder_of_tab(controller, id).map(|folder| Tab::Terminal { folder }),
        TabKind::Browser => crate::browser::url_of_tab(controller, id).map(|url| Tab::Browser { url }),
        TabKind::Agent => {
            let key = key?;
            crate::agent::of_window(controller)
                .into_iter()
                .find(|tab| tab.tab_key() == key)
                .and_then(|tab| tab.session_id())
                .map(|session| Tab::Agent { session })
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
    let mut shares = Vec::new();
    for window_tab in &layout.tabs {
        let mut anchor: Option<isize> = None;
        for document in &window_tab.documents {
            match anchor {
                None => {
                    if opener.open(controller, document) {
                        anchor = controller.selected_tab();
                    }
                }
                Some(anchor) => {
                    controller.open_beside(anchor, || {
                        opener.open(controller, document);
                    });
                    shares.push((anchor, window_tab.share));
                }
            }
        }
        // A preview is not saved, as it opens with its Markdown editor when nothing else is beside it.
        if window_tab.documents.len() == 1
            && let Some(view) = anchor.and_then(|id| crate::editor::editor_of_tab(controller, id))
        {
            crate::markdown::opened(&view);
        }
        firsts.push(anchor);
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
    // A tab that is being built gets its size only once it shows.
    controller.later(move |controller| {
        for (anchor, share) in shares {
            if let Some(tab) = controller.tab_of_document(anchor) {
                tab.set_share(share);
            }
        }
    });
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
            Tab::Welcome => controller.open_welcome() >= 0,
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
            Tab::Agent { session } if trusted => resume_agent(controller, root, session),
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
        .is_some_and(|(agent, resume)| crate::agent::open_agent(controller, agent, Some(resume)).is_some())
}

/// The session `session` of the folder `root` and the agent that ran it: the agent its thread names, else Claude, whose
/// sessions started in a terminal have no thread.
pub(crate) fn past_session(root: &Path, session: &str) -> Option<(crate::agent::Agent, crate::agent::Resume)> {
    let (resume, _) = crate::agent_panel::past_sessions(root, 500)
        .into_iter()
        .find(|(resume, _)| resume.session == session)?;
    let index = ide_thread::ThreadIndex::open(&ide_thread::ThreadIndex::default_dir());
    let agent = resume
        .thread
        .as_ref()
        .and_then(|id| index.get(id))
        .map_or(crate::agent::Agent::Claude, |entry| {
            crate::agent::Agent::of_thread(&entry.agent)
        });
    Some((agent, resume))
}
