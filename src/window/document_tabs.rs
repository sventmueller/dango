//! The documents of a window and the native tabs they stand in: adding, placing beside another, selecting, keeping a
//! preview, closing, and which tab is in front.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSView, NSWindow, NSWindowOrderingMode, NSWindowTabGroup};
use objc2_foundation::NSNotification;

use super::tabs::{Document, TabWindow};
use super::{TabKind, WindowController, layout, show_tab_bar};

impl WindowController {
    /// Shows `view` as a document of `kind` and brings it to the front: in a window tab of its own, or in the tab
    /// in front if that one is empty, or beside a document for `open_beside`. `key` names
    /// what it shows, so `select_tab` finds it again. The document's number.
    pub fn add_tab(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> isize {
        let document = self.new_document(kind, key, title, view);
        let id = document.id;
        self.place(document);
        id
    }

    /// Shows `view` as the preview document, which takes the place of the one before, as a single click opens a
    /// file; an ordinary document when the user turned preview tabs off.
    pub fn add_preview_tab(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> isize {
        if !ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::PREVIEW_TABS) {
            return self.add_tab(kind, key, title, view);
        }
        let old = self.documents().into_iter().find(|doc| doc.is_preview());
        if let Some(old) = old {
            let tab = self.tab_of_document(old.id);
            match tab {
                Some(tab) if crate::editor::may_close(self, old.id) => {
                    let index = tab.documents().iter().position(|doc| doc.id == old.id);
                    self.forget_document(&tab, old.id);
                    let document = self.new_document(kind, key, title, view);
                    document.set_preview(true);
                    let id = document.id;
                    tab.insert(document, index);
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
    pub fn keep_tab(&self, id: isize) {
        if let Some((tab, document)) = self.find_document(id)
            && document.is_preview()
        {
            document.set_preview(false);
            tab.refresh();
        }
    }

    /// The document showing `key`.
    pub fn tab_with_key(&self, key: &str) -> Option<isize> {
        self.documents()
            .into_iter()
            .find(|doc| doc.key.as_deref() == Some(key))
            .map(|doc| doc.id)
    }

    /// Brings the document showing `key` to the front; whether there is one.
    pub fn select_tab(&self, key: &str) -> bool {
        match self.tab_with_key(key) {
            Some(id) => {
                self.select_tab_by_id(id);
                true
            }
            None => false,
        }
    }

    pub fn set_tab_title(&self, id: isize, title: &str) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_title(title);
            tab.refresh();
        }
    }

    /// Shows a dot of `color` on the tab of the document `id`, or none.
    pub fn set_tab_status(&self, id: isize, color: Option<&NSColor>) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_status(color);
            tab.refresh();
        }
    }

    /// Tints the symbol of the document `id`.
    pub fn set_tab_tint(&self, id: isize, color: &NSColor) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_tint(color);
            tab.refresh();
        }
    }

    /// Shows the symbol of the document `id` in four colours, `colors` at its left, top, right and bottom point.
    pub fn set_tab_four_colours(&self, id: isize, colors: [Retained<NSColor>; 4]) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_four_colours(colors);
            tab.refresh();
        }
    }

    /// Runs `open`, which opens a document, so that the document shows beside the document `anchor` in its tab.
    pub fn open_beside(&self, anchor: isize, open: impl FnOnce()) {
        let before = self.ivars().beside.replace(Some(anchor));
        open();
        self.ivars().beside.set(before);
    }

    /// Opens `path` beside the document in front, as the folder tree's "Daneben öffnen" does; a file already open
    /// comes to the front where it is.
    pub fn open_file_beside(&self, path: &Path) {
        if let Some(id) = self.tab_with_key(&path.to_string_lossy()) {
            self.select_tab_by_id(id);
            return;
        }
        match self.selected_tab() {
            Some(anchor) => self.open_beside(anchor, || self.open_file(path)),
            None => self.open_file(path),
        }
    }

    /// Opens `path` in an editor tab, or selects the tab that has it and keeps it if it is the preview tab; a PDF,
    /// a picture, a media file or an office document opens in a tab that shows it.
    pub fn open_file(&self, path: &Path) {
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
    pub fn preview_file(&self, path: &Path) {
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
    pub fn set_tab_dirty(&self, id: isize, dirty: bool) {
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
    pub fn close_tab(&self, id: isize) {
        if crate::editor::may_close(self, id) {
            self.close_tab_now(id);
        }
    }

    /// Closes the document `id` without asking; its window tab goes with it when it was the tab's only one, and the
    /// window with its last tab.
    fn close_tab_now(&self, id: isize) {
        let Some(tab) = self.tab_of_document(id) else { return };
        self.forget_document(&tab, id);
        if tab.is_empty() {
            // A closing window has left its tab group already when AppKit tells its delegate.
            let next = self.neighbour(&tab);
            tab.window.close();
            let open = |next: &Rc<TabWindow>| self.ivars().tabs.borrow().iter().any(|t| Rc::ptr_eq(t, next));
            if let Some(next) = next.filter(open) {
                self.select(&next);
            }
        }
        crate::session::changed();
    }

    /// Takes the document `id` out of `tab` and tells what showed it that it closes.
    fn forget_document(&self, tab: &TabWindow, id: isize) {
        crate::editor::closing(self, id);
        crate::preview::closing(self, id);
        crate::debug::closing(self, id);
        tab.remove(id);
    }

    /// Brings the document `id` to the front; the keyboard focus stays where it is in its tab.
    pub(crate) fn select_tab_by_id(&self, id: isize) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.mark_focused(id);
            self.select(&tab);
            ide_ui_agent::show_pending(self);
        }
    }

    /// The documents' numbers, tab by tab in the order of the tab bar, left to right in a tab.
    pub fn tab_ids(&self) -> Vec<isize> {
        self.documents().iter().map(|doc| doc.id).collect()
    }

    /// Closes every document without asking, for a window that is about to get the documents of the last run or of
    /// another folder; the window keeps the tab in front, empty, and closes the others.
    pub(crate) fn clear_tabs(&self) {
        let tabs = self.ivars().tabs.borrow().clone();
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

    /// Whether `documents` may close: no editor holds unsaved changes the user wants to keep. When one refuses, the
    /// tab stays, and editors whose text the user already gave up keep it after all, with their copies back.
    pub(super) fn documents_may_close(&self, documents: &[Rc<Document>]) -> bool {
        let all = documents
            .iter()
            .all(|document| crate::editor::may_close(self, document.id));
        if !all {
            crate::editor::store_copies(self);
        }
        all
    }

    /// The document commands act on: the focused one of the tab in front.
    pub fn selected_tab(&self) -> Option<isize> {
        self.current_tab()?.focused().map(|doc| doc.id)
    }

    /// Brings the next or the previous tab of the window in front to the front.
    pub(super) fn select_relative(&self, next: bool) {
        let Some(current) = self.current_tab() else { return };
        let Some(tabbed) = current.window.tabbedWindows() else {
            return;
        };
        let windows: Vec<Retained<NSWindow>> = tabbed.iter().collect();
        let Some(index) = windows.iter().position(|w| std::ptr::eq(&**w, &*current.window)) else {
            return;
        };
        let count = windows.len();
        let step = if next { 1 } else { count - 1 };
        let target = windows.get((index + step) % count).and_then(|w| self.tab_of_window(w));
        if let Some(target) = target {
            self.select(&target);
        }
    }

    pub(super) fn current_tab(&self) -> Option<Rc<TabWindow>> {
        self.ivars().current.borrow().clone()
    }

    pub(super) fn tab_of_window(&self, window: &NSWindow) -> Option<Rc<TabWindow>> {
        self.ivars()
            .tabs
            .borrow()
            .iter()
            .find(|tab| std::ptr::eq(&*tab.window, window))
            .cloned()
    }

    pub(super) fn tab_of_notification(&self, notification: &NSNotification) -> Option<Rc<TabWindow>> {
        let object = notification.object()?;
        let window = object.downcast_ref::<NSWindow>()?;
        self.tab_of_window(window)
    }

    pub(super) fn tab_of_document(&self, id: isize) -> Option<Rc<TabWindow>> {
        self.ivars().tabs.borrow().iter().find(|tab| tab.holds(id)).cloned()
    }

    fn find_document(&self, id: isize) -> Option<(Rc<TabWindow>, Rc<Document>)> {
        let tab = self.tab_of_document(id)?;
        let document = tab.document(id)?;
        Some((tab, document))
    }

    /// The window tabs in the order of their tab bars, tab group by tab group.
    pub(super) fn ordered_tabs(&self) -> Vec<Rc<TabWindow>> {
        let tabs = self.ivars().tabs.borrow().clone();
        let by_window: HashMap<*const NSWindow, &Rc<TabWindow>> =
            tabs.iter().map(|tab| (Retained::as_ptr(&tab.window), tab)).collect();
        let mut placed: HashSet<*const NSWindow> = HashSet::with_capacity(tabs.len());
        let mut ordered: Vec<Rc<TabWindow>> = Vec::with_capacity(tabs.len());
        for tab in &tabs {
            if placed.contains(&Retained::as_ptr(&tab.window)) {
                continue;
            }
            let group: Vec<Retained<NSWindow>> = tab
                .window
                .tabbedWindows()
                .map(|tabbed| tabbed.iter().collect())
                .unwrap_or_else(|| vec![tab.window.clone()]);
            for window in group {
                let at = Retained::as_ptr(&window);
                if let Some(found) = by_window.get(&at)
                    && placed.insert(at)
                {
                    ordered.push((*found).clone());
                }
            }
            if placed.insert(Retained::as_ptr(&tab.window)) {
                ordered.push(tab.clone());
            }
        }
        ordered
    }

    /// The documents, tab by tab in the order of the tab bar.
    pub(super) fn documents(&self) -> Vec<Rc<Document>> {
        self.ordered_tabs().iter().flat_map(|tab| tab.documents()).collect()
    }

    fn new_document(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> Rc<Document> {
        let id = self.ivars().next_id.get();
        self.ivars().next_id.set(id + 1);
        Document::new(self, id, kind, key, title, view)
    }

    /// A new window tab of this window, not yet in its tab group.
    pub(super) fn new_tab_window(&self) -> Rc<TabWindow> {
        let tab = TabWindow::new(self, &self.tabbing_identifier(), None);
        self.update_title_of(&tab.window);
        self.ivars().tabs.borrow_mut().push(tab.clone());
        tab
    }

    /// Shows `document` where `add_tab` says.
    fn place(&self, document: Rc<Document>) {
        let anchor = self.ivars().beside.get();
        if let Some((tab, _)) = anchor.and_then(|anchor| self.find_document(anchor)) {
            self.put_beside(&tab, anchor.unwrap_or_default(), document);
            crate::session::changed();
            return;
        }
        let current = self.current_tab();
        if let Some(tab) = &current
            && tab.is_empty()
        {
            tab.insert(document, None);
            self.select(tab);
            crate::session::changed();
            return;
        }
        let tab = self.new_tab_window();
        tab.insert(document, None);
        if let Some(current) = &current {
            current
                .window
                .addTabbedWindow_ordered(&tab.window, NSWindowOrderingMode::Above);
        }
        self.select(&tab);
        crate::session::changed();
    }

    /// Puts `document` right of the document `anchor` in `tab`; a document there before gets a tab of its own.
    fn put_beside(&self, tab: &Rc<TabWindow>, anchor: isize, document: Rc<Document>) {
        let others: Vec<Rc<Document>> = tab.documents().into_iter().filter(|doc| doc.id != anchor).collect();
        for other in others {
            tab.remove(other.id);
            let own = self.new_tab_window();
            own.insert(other, None);
            tab.window
                .addTabbedWindow_ordered(&own.window, NSWindowOrderingMode::Above);
            own.refresh();
        }
        tab.insert(document, None);
        tab.mark_focused(anchor);
        self.select(tab);
    }

    /// Brings `tab` to the front of its tab group and gives it the sidebar.
    fn select(&self, tab: &Rc<TabWindow>) {
        if let Some(group) = tab.window.tabGroup() {
            group.setSelectedWindow(Some(&tab.window));
        }
        // A tab draws the icon of its attributed title only when it gets the title in its tab group.
        tab.refresh();
        if self.current_tab().is_some_and(|current| current.window.isVisible()) || tab.window.isVisible() {
            tab.window.makeKeyAndOrderFront(None);
            show_tab_bar(&tab.window);
        }
        self.make_current(tab);
        crate::session::changed();
    }

    /// Makes the tab the group shows the one in front, after AppKit picked another, as a closed tab leaves.
    fn follow_selection(&self) {
        let selected = self
            .current_tab()
            .and_then(|tab| tab.window.tabGroup()?.selectedWindow());
        if let Some(tab) = selected.and_then(|window| self.tab_of_window(&window)) {
            self.make_current(&tab);
        }
    }

    /// Makes `tab` the one in front: the sidebar moves into it at the width it had, with only the page of the panel
    /// that shows. The tab it leaves keeps a picture of it while its window stays on screen beside; a tab that comes
    /// on screen later without coming to the front gets its picture then.
    pub(super) fn make_current(&self, tab: &Rc<TabWindow>) {
        let previous = self.current_tab();
        if previous.as_ref().is_some_and(|previous| Rc::ptr_eq(previous, tab)) {
            self.list_front_tabs();
            return;
        }
        let mut collapsed = false;
        if let Some(previous) = &previous {
            collapsed = previous.sidebar_collapsed();
            self.remember_sidebar_width(previous);
            self.remember_inspector(previous);
            match self.ivars().sidebar.borrow().as_ref() {
                Some(view) if stays_on_screen(previous, tab) => previous.keep_picture_of(view),
                _ => previous.forget_picture(),
            }
            if let Some(panels) = self.ivars().panels.borrow().as_ref() {
                panels.leave_out_hidden();
            }
        }
        *self.ivars().current.borrow_mut() = Some(tab.clone());
        if let Some(view) = self.ivars().sidebar.borrow().as_ref() {
            tab.adopt_sidebar(view);
        }
        self.fit_sidebar(tab, collapsed);
        self.inspector_follows(tab);
        self.list_front_tabs();
    }

    /// A window tab came on screen or left it: one on screen beside the tab in front shows a picture of the sidebar.
    pub(super) fn tab_visibility_changed(&self, tab: &Rc<TabWindow>) {
        let current = self.current_tab();
        if current.is_some_and(|current| Rc::ptr_eq(&current, tab)) || tab.has_picture() {
            return;
        }
        let visible = tab
            .window
            .occlusionState()
            .contains(objc2_app_kit::NSWindowOcclusionState::Visible);
        if visible && let Some(view) = self.ivars().sidebar.borrow().as_ref() {
            tab.keep_picture_of(view);
        }
    }

    /// Lists only the tab in front of each tab group in the Window menu and the Dock's menu, so a folder appears once
    /// there however many tabs it has; a tab dragged out into a window of its own stays listed.
    fn list_front_tabs(&self) {
        let current = self.current_tab().map(|current| Retained::as_ptr(&current.window));
        // Each group's size and the window in front of it, asked of AppKit once per group.
        let mut fronts: HashMap<*const NSWindowTabGroup, (usize, Option<*const NSWindow>)> = HashMap::new();
        for tab in self.ivars().tabs.borrow().iter() {
            let hidden = tab.window.tabGroup().is_some_and(|group| {
                let (count, front) = *fronts.entry(Retained::as_ptr(&group)).or_insert_with(|| {
                    let windows = group.windows();
                    let front = current
                        .filter(|current| windows.iter().any(|w| Retained::as_ptr(&w) == *current))
                        .or_else(|| group.selectedWindow().map(|w| Retained::as_ptr(&w)));
                    (windows.len(), front)
                });
                count > 1 && front.is_some_and(|front| front != Retained::as_ptr(&tab.window))
            });
            tab.window.setExcludedFromWindowsMenu(hidden);
        }
    }

    /// A window tab closes: its documents go with it; the window goes with its last tab.
    pub(super) fn tab_closes(&self, tab: &Rc<TabWindow>) {
        let last = self.ivars().tabs.borrow().len() == 1;
        if last {
            self.window_closes();
        } else if !self.ivars().closing.get() {
            // The close button closes a tab group's windows one by one in the same pass, so the layout is kept before
            // the first goes; a single tab's close is written down again once the record follows.
            layout::save(self);
            self.ivars().closing.set(true);
            self.later(|controller| controller.ivars().closing.set(false));
        }
        for document in tab.documents() {
            self.forget_document(tab, document.id);
        }
        let neighbour = self.neighbour(tab);
        self.ivars().tabs.borrow_mut().retain(|other| !Rc::ptr_eq(other, tab));
        if self.current_tab().is_some_and(|current| Rc::ptr_eq(&current, tab)) {
            *self.ivars().current.borrow_mut() = None;
            let next = neighbour.or_else(|| self.ivars().tabs.borrow().first().cloned());
            if let Some(next) = next {
                self.make_current(&next);
                self.later(WindowController::follow_selection);
            }
        }
        crate::session::changed();
    }

    /// The tab that comes to the front when `tab` closes, as AppKit picks it: the next one in its tab bar, else the one
    /// before.
    fn neighbour(&self, tab: &TabWindow) -> Option<Rc<TabWindow>> {
        let windows: Vec<Retained<NSWindow>> = tab.window.tabbedWindows()?.iter().collect();
        let index = windows.iter().position(|w| std::ptr::eq(&**w, &*tab.window))?;
        windows
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|before| windows.get(before)))
            .and_then(|window| self.tab_of_window(window))
    }
}

/// Whether the window of `previous` stays on screen once `next` comes to the front: it is visible and not in the tab
/// group of `next`, whose selected window is the only one of the group on screen.
fn stays_on_screen(previous: &TabWindow, next: &TabWindow) -> bool {
    let group = |tab: &TabWindow| tab.window.tabGroup().map(|group| Retained::as_ptr(&group));
    previous.window.isVisible() && (group(previous).is_none() || group(previous) != group(next))
}
