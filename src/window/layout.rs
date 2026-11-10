//! Saving a project window's panels and their tabs, and opening them again when the folder opens (I10).
//!
//! Tabs come back by opening what they showed: files, a new shell in the terminal's last folder, a page, an AI
//! session. Terminals and AI sessions run what the folder configures, so they come back only in a trusted folder
//! (E22); a tab whose file is gone stays away, and a panel left without tabs with it. Editors open through the caller,
//! so a launch can put the copies a crash left in their places (`session::restore`).

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use ide_core::layout::{Node, Tab};

use super::panes::{Pane, Panes, Side};
use super::{TabKind, WindowController};

/// A document an editor tab of a layout shows.
pub enum Document<'a> {
    File(&'a Path),
    /// An untitled one, by the name of its copy.
    Untitled(&'a str),
}

/// Opens a document's editor tab in the active panel; whether it did.
pub type Opener<'a> = dyn FnMut(&WindowController, Document) -> bool + 'a;

/// Saves the panels of `controller`'s window for its folder; a window without a folder keeps nothing.
pub fn save(controller: &WindowController) {
    let Some(root) = controller.project_root() else { return };
    let Some(layout) = controller.with_panes(|panes| panes.layout(&|id| reopenable(controller, panes, id))) else {
        return;
    };
    if let Err(error) = ide_core::layout::save(&root, &layout) {
        crate::diag::report("saving the panel layout", error);
    }
}

/// How the tab `id` opens again, if it can.
fn reopenable(controller: &WindowController, panes: &Panes, id: isize) -> Option<Tab> {
    let pane = panes.of_tab(id)?;
    let (_, kind, key) = pane.tabs.entries().into_iter().find(|(tab, _, _)| *tab == id)?;
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
        TabKind::Diff | TabKind::Log | TabKind::Cli | TabKind::Plan | TabKind::Preview => None,
    }
}

/// Opens the panels and tabs saved for `controller`'s folder, each editor's document through `editor`; the welcome
/// tab the window started with gives way once anything opened, unless the layout has one.
pub fn restore(controller: &WindowController, editor: &mut Opener) {
    let Some(root) = controller.project_root() else { return };
    let Some(layout) = ide_core::layout::load(&root).filter(|layout| layout.tab_count() > 0) else {
        return;
    };
    let trusted = controller.is_trusted();
    let built = controller.with_panes(|panes| {
        let first = panes.all().into_iter().next()?;
        let welcome = first.tabs.find(TabKind::Welcome);
        let opener = Tabs {
            root: &root,
            trusted,
            editor: RefCell::new(editor),
        };
        build(controller, panes, &first, &layout, &opener);
        for pane in panes.all() {
            panes.remove_if_empty(&pane);
        }
        panes.set_shares(&layout);
        let total: usize = panes.all().iter().map(|pane| pane.tabs.count()).sum();
        Some((welcome, total))
    });
    let keeps_welcome = layout.has(&Tab::Welcome);
    if let Some(Some((Some(welcome), total))) = built
        && total > 1
        && !keeps_welcome
    {
        controller.close_tab(welcome);
    }
    // A window that is being built gets its size only once it shows.
    controller.later(move |controller| {
        controller.with_panes(|panes| panes.set_shares(&layout));
    });
}

/// How the tabs of a layout open.
struct Tabs<'a, 'b> {
    root: &'a Path,
    trusted: bool,
    editor: RefCell<&'a mut Opener<'b>>,
}

/// Lays out `node` in `pane`: opens a panel's tabs there, or puts a split's parts in `pane` and new panels beside it.
fn build(controller: &WindowController, panes: &Panes, pane: &Rc<Pane>, node: &Node, opener: &Tabs) {
    match node {
        Node::Panel { tabs, selected } => {
            panes.activate(pane);
            let mut opened = vec![];
            for tab in tabs {
                if opener.open(controller, tab) {
                    opened.push(pane.tabs.selected_id());
                } else {
                    opened.push(None);
                }
            }
            if let Some(id) = selected.and_then(|index| opened.get(index).copied().flatten()) {
                pane.tabs.select(id);
            }
        }
        Node::Split {
            side_by_side, children, ..
        } => {
            let side = if *side_by_side { Side::Right } else { Side::Below };
            let mut parts = vec![pane.clone()];
            for _ in 1..children.len() {
                let last = parts.last().cloned().expect("one part at least");
                match panes.add_beside(&last, side) {
                    Some(next) => parts.push(next),
                    None => break,
                }
            }
            for (part, child) in parts.iter().zip(children) {
                build(controller, panes, part, child, opener);
            }
        }
    }
}

impl Tabs<'_, '_> {
    /// Opens `tab` in the active panel; whether it opened.
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
            _ => false,
        }
    }
}

/// Opens the AI tab of the session `session` again, which resumes it with `claude`; a self-test plays a recording
/// instead, since it runs no agent.
fn resume_agent(controller: &WindowController, root: &Path, session: &str) -> bool {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return crate::selftest::replay_agent(controller, session);
    }
    crate::agent_panel::past_sessions(root, 500)
        .into_iter()
        .find(|(resume, _)| resume.session == session)
        .and_then(|(resume, _)| crate::agent::open_tab(controller, Some(resume)))
        .is_some()
}
