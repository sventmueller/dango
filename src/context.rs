//! The app context (E97): the open objects, the work that runs and the app's state, which the app delegate creates and
//! hands to the window controllers by `Rc`. docs/thread-locals.md lists what stays outside it and why.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::MainThreadMarker;
use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2_app_kit::NSApplication;

use crate::app::AppDelegate;
use crate::editor::EditorTextView;
use crate::window::WindowController;

/// The app's open objects, running work and state.
#[derive(Default)]
pub struct AppContext {
    pub windows: Registry<Retained<WindowController>>,
    pub session: crate::session::Saving,
    pub navigation: crate::navigation::Navigation,
    pub new_tab_menu: crate::window::NewTabMenu,
    pub activities: crate::activity::Activities,
    pub settings: crate::settings::SettingsState,
    /// The editors of every window.
    pub editors: RefCell<Vec<Weak<EditorTextView>>>,
    pub editor: crate::editor::EditorState,
    pub previews: crate::preview::Previews,
    pub exports: crate::markdown::Exports,
    pub search_panels: crate::window::SearchPanels,
    pub hierarchy_panels: crate::window::HierarchyPanels,
    pub terminal_views: crate::terminal::TerminalViews,
    pub lsp_workspaces: crate::lsp::Workspaces,
    pub server_leases: crate::lsp::Leases,
    pub inlay_requests: crate::lsp::inlay::InlayRequests,
    pub formatting: crate::lsp::Formatting,
    pub completion: crate::lsp::Completion,
    pub signature_help: crate::lsp::ShownSignature,
    pub code_actions: crate::lsp::CodeActions,
    pub location_sheet: crate::lsp::LocationSheets,
    pub agent: crate::agent::AgentState,
    pub agent_panels: crate::agent_panel::AgentPanels,
    pub git: crate::git_panel::Open,
    pub ci_panels: crate::ci_panel::CiPanels,
    pub ci_log_tabs: crate::ci_panel::LogTabs,
    pub browser_tabs: crate::browser::BrowserTabs,
    pub downloads: crate::browser::Downloading,
    pub certificates: crate::browser::Certificates,
    #[cfg(feature = "selftest")]
    pub browser_dialogs: crate::browser::Dialogs,
    #[cfg(feature = "dictation")]
    pub dictation: crate::dictation::DictationState,
}

impl AppContext {
    /// The context of the running app, for code that reaches the main thread without one at hand, such as an answer
    /// from a worker thread, which carries only ids (Review C11).
    pub fn get(mtm: MainThreadMarker) -> Rc<AppContext> {
        Self::of_delegate(mtm).expect("the app delegate exists before anything asks for the app context")
    }

    fn of_delegate(mtm: MainThreadMarker) -> Option<Rc<AppContext>> {
        let delegate = NSApplication::sharedApplication(mtm).delegate()?;
        let object: &AnyObject = delegate.as_ref();
        Some(object.downcast_ref::<AppDelegate>()?.context())
    }
}

/// `AppContext::get` from code that runs on the main thread but holds no marker for it.
pub fn app() -> Rc<AppContext> {
    AppContext::get(MainThreadMarker::new().expect("the app context lives on the main thread"))
}

/// The app context unless the app has none (yet), for code such as `Drop` that must not fail.
pub fn try_app() -> Option<Rc<AppContext>> {
    AppContext::of_delegate(MainThreadMarker::new()?)
}

/// Open objects by an id of their own, which answers from other threads carry instead of the object.
pub struct Registry<T> {
    next: Cell<u64>,
    entries: RefCell<Vec<(u64, T)>>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Registry {
            next: Cell::new(1),
            entries: RefCell::new(Vec::new()),
        }
    }
}

impl<T: Clone> Registry<T> {
    /// The id the next object gets, which no object had before.
    pub fn next_id(&self) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        id
    }

    pub fn insert(&self, id: u64, object: T) {
        self.entries.borrow_mut().push((id, object));
    }

    pub fn remove(&self, id: u64) -> Option<T> {
        let mut entries = self.entries.borrow_mut();
        let index = entries.iter().position(|(other, _)| *other == id)?;
        Some(entries.remove(index).1)
    }

    /// The object with `id`, unless it closed in the meantime.
    pub fn get(&self, id: u64) -> Option<T> {
        self.entries
            .borrow()
            .iter()
            .find(|(other, _)| *other == id)
            .map(|(_, object)| object.clone())
    }

    /// The open objects in the order they opened.
    pub fn all(&self) -> Vec<T> {
        self.entries.borrow().iter().map(|(_, object)| object.clone()).collect()
    }

    pub fn find(&self, mut f: impl FnMut(&T) -> bool) -> Option<T> {
        self.entries
            .borrow()
            .iter()
            .find(|(_, object)| f(object))
            .map(|(_, object)| object.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_for_an_object_that_closed_finds_nothing() {
        let open: Registry<Rc<&str>> = Registry::default();
        let first = open.next_id();
        open.insert(first, Rc::new("first"));
        let second = open.next_id();
        open.insert(second, Rc::new("second"));
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || tx.send((first, "answer")).unwrap());
        open.remove(first);
        worker.join().unwrap();
        let (id, _answer) = rx.recv().unwrap();
        assert!(open.get(id).is_none());
        assert_eq!(open.get(second).as_deref(), Some(&"second"));
    }

    #[test]
    fn ids_are_not_given_twice() {
        let open: Registry<u8> = Registry::default();
        let first = open.next_id();
        open.insert(first, 1);
        open.remove(first);
        assert_ne!(open.next_id(), first);
    }
}
