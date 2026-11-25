//! The window tabs of a layout behind the one in front, opened one per frame once the window showed its first, so the
//! window appears before all its tabs are built.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ide_core::layout::{Editor, Tab, Welcome, WindowTab};
use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::NSWindowOrderingMode;

use super::{Document, Documents, restore_tab};
use crate::context::AppContext;
use crate::util::FlagGuard;
use crate::window::documents::ids_of;
use crate::window::tabs::TabWindow;
use crate::window::{TabId, WindowController, WindowId};

/// The saved tabs of a window that wait for a frame of their own.
#[derive(Default)]
pub(in crate::window) struct Waiting {
    /// Which restore the scheduled steps belong to; a later restore or a cancel leaves theirs behind.
    round: Cell<u64>,
    /// A tab is being opened, which a save or a finish on demand reaches in the middle of.
    busy: Cell<bool>,
    later: RefCell<Option<Later>>,
}

/// The documents each opened tab of a layout started with, by the tab's index in the layout.
pub(super) type Started = Vec<(usize, Vec<TabId>)>;

/// What opening the waiting tabs needs.
pub(super) struct Later {
    pub(super) root: PathBuf,
    pub(super) trusted: bool,
    pub(super) font_size: f64,
    /// The tabs still to open, by their index in the layout, in its order.
    pub(super) tabs: VecDeque<(usize, WindowTab)>,
    pub(super) opened: Started,
}

/// Keeps `later` for `controller`'s window and opens its first tab once the window committed its next frame.
pub(super) fn wait(controller: &WindowController, later: Later) {
    let waiting = &controller.ivars().documents.later;
    let round = waiting.round.get() + 1;
    waiting.round.set(round);
    *waiting.later.borrow_mut() = Some(later);
    let (cx, window) = (Rc::clone(controller.context()), controller.id());
    crate::util::after_frame(move || step(&cx, window, round));
}

fn step(cx: &Rc<AppContext>, window: WindowId, round: u64) {
    let Some(controller) = cx.windows().get(window.get()) else {
        return;
    };
    let waiting = &controller.ivars().documents.later;
    if waiting.round.get() != round {
        return;
    }
    if controller.ivars().closed.get() {
        controller.cancel_restoring();
        return;
    }
    let cx = Rc::clone(cx);
    if open_next(&controller) {
        crate::util::after_frame(move || step(&cx, window, round));
    } else {
        // The tab group settles the sizes of its windows once more after the last one joined it.
        crate::util::after_frame(move || {
            if let Some(controller) = cx.windows().get(window.get()) {
                controller.reapply_saved_frame();
            }
        });
    }
}

/// Opens the next waiting tab where the layout had it, behind the tab in front; whether tabs still wait.
fn open_next(controller: &WindowController) -> bool {
    let waiting = &controller.ivars().documents.later;
    if waiting.busy.get() || controller.restoring_editor_tabs() {
        return controller.restoring_later();
    }
    let next = {
        let mut later = waiting.later.borrow_mut();
        let Some(state) = later.as_mut() else { return false };
        match state.tabs.pop_front() {
            Some((index, saved)) => (index, saved, state.root.clone(), state.trusted, state.font_size),
            None => {
                *later = None;
                return false;
            }
        }
    };
    let (index, saved, root, trusted, font_size) = next;
    let _busy = FlagGuard::raise(&waiting.busy);
    let front = controller.current_tab();
    let mut editor = |controller: &WindowController, document: Document| match document {
        // The user may have opened the file in the meantime.
        Document::File(path) => {
            controller.tab_with_key(&path.to_string_lossy()).is_none()
                && controller.restore_editor(path, font_size).is_some()
        }
        Document::Untitled(_) => false,
    };
    let editor: &mut super::Opener = &mut editor;
    let opener = Documents {
        root: &root,
        trusted,
        editor: RefCell::new(editor),
    };
    let mut built = None;
    // A window added to the tab group takes on its own content's size, and the group follows it.
    let frame = controller.window().map(|window| window.frame());
    controller.restore_behind(|| {
        let tab = new_tab_at(controller, index);
        if restore_tab(controller, &opener, &saved, &tab).is_some() {
            built = Some(tab);
        }
    });
    keep_in_front(front.as_ref());
    if let Some(tab) = built {
        let ids = ids_of(&tab.documents());
        if let Some(state) = waiting.later.borrow_mut().as_mut() {
            state.opened.push((index, ids));
        }
        controller.settle_restored_tab(&tab);
        controller.list_front_tabs();
    }
    if let (Some(frame), Some(window)) = (frame, controller.window())
        && window.frame() != frame
    {
        window.setFrame_display(frame, true);
    }
    controller.restoring_later()
}

/// AppKit may bring a tab added to the group to the front, which the one the user sees stays.
fn keep_in_front(front: Option<&Rc<TabWindow>>) {
    let Some(front) = front else { return };
    if let Some(group) = front.window.tabGroup()
        && group
            .selectedWindow()
            .is_none_or(|selected| Retained::as_ptr(&selected) != Retained::as_ptr(&front.window))
    {
        group.setSelectedWindow(Some(&front.window));
    }
}

/// A new window tab in the tab group, after the tabs of the layout's earlier tabs, else before those of its later ones.
fn new_tab_at(controller: &WindowController, index: usize) -> Rc<TabWindow> {
    let opened = controller
        .ivars()
        .documents
        .later
        .later
        .borrow()
        .as_ref()
        .map(|state| state.opened.clone())
        .unwrap_or_default();
    let ordered = controller.ordered_tabs();
    let holding = |ids: &[TabId]| {
        ordered
            .iter()
            .filter(|tab| tab.documents().iter().any(|document| ids.contains(&document.id)))
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut before: Vec<&(usize, Vec<TabId>)> = opened.iter().filter(|(at, _)| *at < index).collect();
    before.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    let mut after: Vec<&(usize, Vec<TabId>)> = opened.iter().filter(|(at, _)| *at > index).collect();
    after.sort_by_key(|(at, _)| *at);
    let anchor = before
        .iter()
        .find_map(|(_, ids)| {
            holding(ids)
                .last()
                .cloned()
                .map(|tab| (tab, NSWindowOrderingMode::Above))
        })
        .or_else(|| {
            after.iter().find_map(|(_, ids)| {
                holding(ids)
                    .first()
                    .cloned()
                    .map(|tab| (tab, NSWindowOrderingMode::Below))
            })
        })
        .or_else(|| controller.current_tab().map(|tab| (tab, NSWindowOrderingMode::Above)));
    let tab = controller.new_tab_window();
    if let Some((anchor, ordering)) = anchor {
        anchor.window.addTabbedWindow_ordered(&tab.window, ordering);
    }
    tab
}

impl WindowController {
    /// Whether saved tabs of the window still wait to open.
    pub(crate) fn restoring_later(&self) -> bool {
        self.ivars()
            .documents
            .later
            .later
            .borrow()
            .as_ref()
            .is_some_and(|state| !state.tabs.is_empty())
    }

    /// Whether a waiting tab shows the file at `path`, which it opens with.
    pub(crate) fn restores_later(&self, path: &Path) -> bool {
        self.waits_with(|document| document.read::<Editor>().is_some_and(|editor| editor.path == path))
    }

    /// Whether a waiting tab shows the welcome page, which it opens with in its place.
    pub(crate) fn restores_welcome_later(&self) -> bool {
        self.waits_with(Tab::is::<Welcome>)
    }

    fn waits_with(&self, shows: impl Fn(&Tab) -> bool) -> bool {
        self.ivars()
            .documents
            .later
            .later
            .borrow()
            .as_ref()
            .is_some_and(|state| state.tabs.iter().flat_map(|(_, saved)| saved.documents()).any(shows))
    }

    /// Opens every waiting tab now, for a command that needs all of them.
    pub(crate) fn finish_restoring(&self) {
        let waiting = &self.ivars().documents.later;
        if waiting.busy.get() || self.restoring_editor_tabs() {
            return;
        }
        while open_next(self) {}
    }

    /// Forgets the waiting tabs, as the window closes or gets other documents.
    pub(in crate::window) fn cancel_restoring(&self) {
        let waiting = &self.ivars().documents.later;
        waiting.round.set(waiting.round.get() + 1);
        waiting.later.borrow_mut().take();
    }

    /// The waiting tabs by their index in the layout, and the documents each opened one started with, for a save.
    pub(super) fn waiting_tabs(&self) -> (Vec<(usize, WindowTab)>, Started) {
        match self.ivars().documents.later.later.borrow().as_ref() {
            Some(state) => (state.tabs.iter().cloned().collect(), state.opened.clone()),
            None => (Vec::new(), Vec::new()),
        }
    }
}
