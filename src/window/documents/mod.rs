//! The documents of a window and the native tabs they stand in (E60, E62, E145): adding, placing beside another or in a
//! pane, keeping a preview, closing. Which tab is in front is in `front`, a split window in `split_mode`, what the
//! user does with the panes in `pane_commands`.

mod front;
mod pane_commands;
mod restored;
mod split_mode;

pub(super) use pane_commands::side_with_option;

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSView, NSWindowOrderingMode};

use crate::editor::{Again, Answer};

use super::panes::{PaneId, Place, Side};
use super::tabs::{Document, TabWindow};
use super::{TabId, TabKind, WindowController};

/// What closes once the documents of a close agreed after a save they asked for.
pub(super) type Close = Rc<dyn Fn(&WindowController)>;

/// The documents' numbers.
pub(super) fn ids_of(documents: &[Rc<Document>]) -> Vec<TabId> {
    documents.iter().map(|document| document.id).collect()
}

fn no_again() -> Again {
    Rc::new(|_| {})
}

/// The window tabs of a window and where the next document goes.
pub(super) struct State {
    /// The window tabs, in the order they opened.
    tabs: RefCell<Vec<Rc<TabWindow>>>,
    /// The window tab in front, which has the sidebar.
    current: RefCell<Option<Rc<TabWindow>>>,
    /// The number of the next document.
    next_id: Cell<isize>,
    /// Where a document about to open goes instead of where it would by default (E145).
    target: RefCell<Option<Target>>,
    /// Whether a window tab about to open goes to the end of the tab bar instead of after the one in front.
    at_end: Cell<bool>,
    restored: restored::Editors,
    /// The tabs of a layout that open after the window's first frame.
    pub(in crate::window) later: crate::window::layout::Waiting,
}

impl Default for State {
    fn default() -> State {
        State {
            tabs: RefCell::default(),
            current: RefCell::default(),
            next_id: Cell::new(1),
            target: RefCell::default(),
            at_end: Cell::new(false),
            restored: restored::Editors::default(),
            later: crate::window::layout::Waiting::default(),
        }
    }
}

/// Where a document about to open goes instead of where it would by default (E145).
#[derive(Clone)]
pub(super) enum Target {
    /// Into a pane of a window tab, after its document in front.
    Into(Rc<TabWindow>, PaneId),
    /// Into a new pane at `side` of the pane that holds `anchor`; `companion` for a preview that follows it (E59).
    Beside { anchor: TabId, side: Side, companion: bool },
}

impl WindowController {
    /// Shows `view` as a document of `kind` and brings it to the front: where `open_beside` or `open_into` say, else
    /// in the active pane of the tab in front if that one is split or empty, else in a window tab of its own (E145).
    /// `key` names what it shows, so `select_tab` finds it again. The document's number.
    pub(crate) fn add_tab(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> TabId {
        let document = self.new_document(kind, key, title, view);
        let id = document.id;
        self.place(document);
        id
    }

    /// Shows `view` as the preview document, which takes the place of the one before, as a single click opens a
    /// file; an ordinary document when the user turned preview tabs off.
    pub(crate) fn add_preview_tab(
        &self,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
    ) -> TabId {
        if !ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::PREVIEW_TABS) {
            return self.add_tab(kind, key, title, view);
        }
        let old = self.documents().into_iter().find(|doc| doc.is_preview());
        if let Some(old) = old {
            let tab = self.tab_of_document(old.id);
            let pane = tab.as_ref().and_then(|tab| tab.panes.of_document(old.id));
            match (tab, pane) {
                (Some(tab), Some(pane)) if crate::editor::may_close(self, old.id, &no_again()) == Answer::Yes => {
                    let index = tab.panes.documents_in(pane).iter().position(|doc| doc.id == old.id);
                    let document = self.new_document(kind, key, title, view);
                    document.set_preview(true);
                    let id = document.id;
                    tab.insert(document, Place::Into(pane, index));
                    self.forget_document(&tab, old.id);
                    self.select(&tab);
                    crate::session::changed();
                    return id;
                }
                _ => self.keep_tab(old.id),
            }
        }
        let id = self.add_tab(kind, key, title, view);
        if let Some((tab, document)) = self.find_document(id) {
            document.set_preview(true);
            tab.refresh();
        }
        id
    }

    /// Makes the document `id` an ordinary one if it is the preview document.
    pub(crate) fn keep_tab(&self, id: TabId) {
        if let Some((tab, document)) = self.find_document(id)
            && document.is_preview()
        {
            document.set_preview(false);
            tab.refresh();
        }
    }

    /// The document showing `key`.
    pub(crate) fn tab_with_key(&self, key: &str) -> Option<TabId> {
        self.documents()
            .into_iter()
            .find(|doc| doc.key.as_deref() == Some(key))
            .map(|doc| doc.id)
    }

    /// Brings the document showing `key` to the front; whether there is one.
    pub(crate) fn select_tab(&self, key: &str) -> bool {
        match self.tab_with_key(key) {
            Some(id) => {
                self.select_tab_by_id(id);
                true
            }
            None => false,
        }
    }

    pub(crate) fn set_tab_title(&self, id: TabId, title: &str) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_title(title);
            tab.refresh();
        }
    }

    /// Shows a dot of `color` on the tab of the document `id`, or none.
    pub(crate) fn set_tab_status(&self, id: TabId, color: Option<&NSColor>) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_status(color);
            tab.refresh();
        }
    }

    /// Shows `ring` on the tab of the document `id` while no document of the tab has a dot, or no ring.
    pub(crate) fn set_tab_progress(&self, id: TabId, ring: Option<crate::terminal::Ring>) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_progress(ring);
            tab.refresh_accessory();
        }
    }

    /// Tints the symbol of the document `id`.
    pub(crate) fn set_tab_tint(&self, id: TabId, color: &NSColor) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_tint(color);
            tab.refresh();
        }
    }

    /// Shows the symbol of the document `id` in four colours, `colors` at its left, top, right and bottom point.
    pub(crate) fn set_tab_four_colours(&self, id: TabId, colors: [Retained<NSColor>; 4]) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_four_colours(colors);
            tab.refresh();
        }
    }

    /// Runs `open`, which opens the preview of the document `anchor`, so that it shows in a new pane right of it,
    /// which follows that document and does not count as a split (E59).
    pub(crate) fn open_companion(&self, anchor: TabId, open: impl FnOnce()) {
        self.open_at(
            Target::Beside {
                anchor,
                side: Side::Right,
                companion: true,
            },
            open,
        );
    }

    /// Runs `open`, which opens a document, so that the document shows in `pane` of `tab`.
    pub(super) fn open_into(&self, tab: &Rc<TabWindow>, pane: PaneId, open: impl FnOnce()) {
        self.open_at(Target::Into(tab.clone(), pane), open);
    }

    pub(super) fn open_at(&self, target: Target, open: impl FnOnce()) {
        let before = self.ivars().documents.target.replace(Some(target));
        open();
        *self.ivars().documents.target.borrow_mut() = before;
    }

    /// Opens `path` in a new pane at `side` of the document in front, as the folder tree's "Daneben öffnen" and
    /// "Darunter öffnen" do; a file already open comes to the front where it is.
    pub(crate) fn open_file_beside(&self, path: &Path, side: Side) {
        if let Some(id) = self.tab_with_key(&path.to_string_lossy()) {
            self.select_tab_by_id(id);
            return;
        }
        match self.selected_tab() {
            Some(anchor) => self.open_at(
                Target::Beside {
                    anchor,
                    side,
                    companion: false,
                },
                || self.open_file(path),
            ),
            None => self.open_file(path),
        }
        self.settle_split();
    }

    /// Opens `path` in the pane numbered `number` of the tab in front, or in a window tab of its own without one.
    pub(crate) fn open_file_in_pane(&self, path: &Path, number: Option<usize>) {
        if let Some(id) = self.tab_with_key(&path.to_string_lossy()) {
            self.select_tab_by_id(id);
            return;
        }
        let current = self.current_tab();
        let pane = current
            .as_ref()
            .and_then(|tab| Some((tab, tab.panes.by_number(number?)?)));
        match pane {
            Some((tab, pane)) => self.open_into(tab, pane, || self.open_file(path)),
            None => self.in_window_tab_of_its_own(|| self.open_file(path)),
        }
        self.settle_split();
    }

    /// Opens `path` in an editor tab, or selects the tab that has it and keeps it if it is the preview tab; a PDF,
    /// a picture, a media file or an office document opens in a tab that shows it.
    pub(crate) fn open_file(&self, path: &Path) {
        if crate::preview::is_document(path) && crate::preview::open_document(self, path, false) {
            return;
        }
        if let Some(id) = self.tab_with_key(&path.to_string_lossy()) {
            self.keep_tab(id);
            self.select_tab_by_id(id);
            return;
        }
        let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
        crate::editor::open_tab(self, path, size, false);
    }

    /// Opens `path` in the preview tab, as a single click in the folder tree does, or selects the tab that has it.
    pub(crate) fn preview_file(&self, path: &Path) {
        if crate::preview::is_document(path) && crate::preview::open_document(self, path, true) {
            return;
        }
        if self.select_tab(&path.to_string_lossy()) {
            return;
        }
        let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
        crate::editor::open_tab(self, path, size, true);
    }

    /// Marks the document `id` as having unsaved changes, or not; the first change keeps a preview document.
    pub(crate) fn set_tab_dirty(&self, id: TabId, dirty: bool) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_dirty(dirty);
            if dirty {
                document.set_preview(false);
            }
            tab.refresh();
        }
    }

    /// Closes the document `id` once it agrees, such as an editor with unsaved changes or a pop-up whose page closed
    /// itself; a window tab left without documents closes with it.
    pub(crate) fn close_tab(&self, id: TabId) {
        let (cx, window) = (Rc::clone(self.context()), self.id());
        let again: Again = Rc::new(move |written| {
            if let Some(controller) = cx.windows().get(window.get())
                && written
            {
                controller.close_tab(id);
            }
        });
        if crate::editor::may_close(self, id, &again) == Answer::Yes {
            self.close_tab_now(id);
        }
    }

    /// Closes the document `id` without asking; its window tab goes with it when it was the tab's only one, and the
    /// window with its last tab.
    pub(crate) fn close_tab_now(&self, id: TabId) {
        let Some(tab) = self.tab_of_document(id) else { return };
        let split = tab.panes.is_split();
        self.forget_document(&tab, id);
        if split && !tab.panes.is_split() && self.current_tab().is_some_and(|current| Rc::ptr_eq(&current, &tab)) {
            self.settle_split();
        }
        if tab.is_empty() {
            // A closing window has left its tab group already when AppKit tells its delegate.
            let next = self.neighbour(&tab);
            tab.window.close();
            let open = |next: &Rc<TabWindow>| self.ivars().documents.tabs.borrow().iter().any(|t| Rc::ptr_eq(t, next));
            if let Some(next) = next.filter(open) {
                self.select(&next);
            }
        } else if self.current_tab().is_some_and(|current| Rc::ptr_eq(&current, &tab)) {
            self.load_restored_editors(&tab);
        }
        crate::session::changed();
    }

    /// Takes the document `id` out of `tab` and tells what showed it that it closes.
    pub(super) fn forget_document(&self, tab: &TabWindow, id: TabId) {
        self.ivars().documents.restored.pending.borrow_mut().remove(&id);
        crate::editor::closing(self, id);
        crate::preview::closing(self, id);
        crate::debug::closing(self, id);
        crate::terminal::closing(self, id);
        tab.remove(id);
    }

    /// Brings the document `id` to the front; the keyboard focus stays where it is in its tab.
    pub(crate) fn select_tab_by_id(&self, id: TabId) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.mark_focused(id);
            self.select(&tab);
            ide_ui_agent::show_pending(self);
            self.tabs_came_forward();
        }
    }

    /// The documents' numbers, tab by tab in the order of the tab bar, pane by pane in a tab.
    pub(crate) fn tab_ids(&self) -> Vec<TabId> {
        self.documents().iter().map(|doc| doc.id).collect()
    }

    /// Closes every document without asking, for a window that is about to get the documents of the last run or of
    /// another folder; the window keeps the tab in front, empty, and closes the others.
    pub(crate) fn clear_tabs(&self) {
        self.cancel_restoring();
        let tabs = self.ivars().documents.tabs.borrow().clone();
        let keep = self.current_tab().or_else(|| tabs.first().cloned());
        for tab in &tabs {
            for document in tab.documents() {
                self.forget_document(tab, document.id);
            }
        }
        for tab in tabs {
            if keep.as_ref().is_some_and(|keep| !Rc::ptr_eq(keep, &tab)) {
                tab.window.close();
            }
        }
    }

    /// Whether the documents `ids` may close now: no editor holds unsaved changes the user wants to keep. When one
    /// refuses, the tab stays, and editors whose text the user already gave up keep it after all, with their copies
    /// back; when one saves first, they are asked again once it is written, and `close` runs if they all agree.
    pub(super) fn documents_may_close(&self, ids: &[TabId], close: Close) -> bool {
        let again = self.close_again(ids.to_vec(), close);
        for &id in ids {
            match crate::editor::may_close(self, id, &again) {
                Answer::Yes => {}
                Answer::No => {
                    crate::editor::store_copies(self);
                    return false;
                }
                Answer::Later => return false,
            }
        }
        true
    }

    /// What a close of the documents `ids` that waits for a save does once the file is written.
    fn close_again(&self, ids: Vec<TabId>, close: Close) -> Again {
        let (cx, window) = (Rc::clone(self.context()), self.id());
        Rc::new(move |written| {
            let Some(controller) = cx.windows().get(window.get()) else {
                return;
            };
            if !written {
                crate::editor::store_copies(&controller);
                return;
            }
            let open: Vec<TabId> = ids
                .iter()
                .copied()
                .filter(|id| controller.find_document(*id).is_some())
                .collect();
            if controller.documents_may_close(&open, Rc::clone(&close)) {
                close(&controller);
            }
        })
    }

    fn new_document(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> Rc<Document> {
        let id = self.ivars().documents.next_id.get();
        self.ivars().documents.next_id.set(id + 1);
        Document::new(self, TabId::new(id), kind, key, title, view)
    }

    /// A new window tab of this window, not yet in its tab group.
    pub(super) fn new_tab_window(&self) -> Rc<TabWindow> {
        let tab = TabWindow::new(self, &self.tabbing_identifier(), None);
        self.update_title_of(&tab.window);
        self.ivars().documents.tabs.borrow_mut().push(tab.clone());
        tab
    }

    /// Shows `document` where `add_tab` says.
    fn place(&self, document: Rc<Document>) {
        let target = self.ivars().documents.target.borrow().clone();
        let found = match target {
            Some(Target::Beside {
                anchor,
                side,
                companion,
            }) => self.tab_of_document(anchor).map(|tab| {
                let place = Place::Beside {
                    anchor,
                    side,
                    companion,
                };
                (tab, place, Some(anchor))
            }),
            Some(Target::Into(tab, pane)) => {
                let open = self
                    .ivars()
                    .documents
                    .tabs
                    .borrow()
                    .iter()
                    .any(|own| Rc::ptr_eq(own, &tab));
                (open && tab.panes.has(pane)).then_some((tab, Place::Into(pane, None), None))
            }
            None => None,
        };
        if let Some((tab, place, anchor)) = found {
            tab.insert(document, place);
            if let Some(anchor) = anchor {
                tab.mark_focused(anchor);
            }
            self.select(&tab);
            crate::session::changed();
            return;
        }
        let current = self.current_tab();
        if let Some(tab) = &current
            && (tab.is_empty() || tab.panes.is_split())
        {
            tab.insert(document, Place::Active);
            self.select(tab);
            crate::session::changed();
            return;
        }
        let tab = self.new_tab_window();
        tab.insert(document, Place::Active);
        self.attach(&tab);
        self.select(&tab);
        crate::session::changed();
    }

    /// Runs `open`, which opens a document, so that it gets a window tab of its own also while the tab in front is
    /// split.
    pub(super) fn in_window_tab_of_its_own(&self, open: impl FnOnce()) {
        let tab = self.new_tab_after_current();
        let first = tab.panes.active();
        self.open_into(&tab, first, open);
        if tab.is_empty() {
            tab.window.close();
        }
    }

    /// A new window tab after the one in front, in its tab group, for the layout to fill.
    pub(super) fn new_tab_after_current(&self) -> Rc<TabWindow> {
        let tab = self.new_tab_window();
        self.attach(&tab);
        tab
    }

    /// Runs `open`, which makes something new as the "+" menu does, so that a window tab it gets goes to the end of
    /// the tab bar, as Safari's "+" does.
    pub(super) fn at_end<R>(&self, open: impl FnOnce() -> R) -> R {
        let before = self.ivars().documents.at_end.replace(true);
        let opened = open();
        self.ivars().documents.at_end.set(before);
        opened
    }

    /// Puts `tab` into the tab group of the tab in front, after it or at the end as `at_end` says.
    fn attach(&self, tab: &TabWindow) {
        let Some(current) = self.current_tab() else { return };
        let last = if self.ivars().documents.at_end.get() {
            current.window.tabbedWindows().and_then(|tabbed| tabbed.iter().last())
        } else {
            None
        };
        last.as_ref()
            .unwrap_or(&current.window)
            .addTabbedWindow_ordered(&tab.window, NSWindowOrderingMode::Above);
    }
}
