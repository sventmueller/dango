//! Which window tab of a window is in front, the order of its tabs and finding the tab of a window, a notification or
//! a document; the sidebar moving into the tab that comes to the front, and a tab closing.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{NSWindow, NSWindowTabGroup};
use objc2_foundation::NSNotification;

use crate::window::tab_titles::{self, Named};
use crate::window::tabs::{Document, TabWindow};
use crate::window::{TabId, WindowController, layout};

impl WindowController {
    /// The document commands act on: the focused one of the tab in front.
    pub(crate) fn selected_tab(&self) -> Option<TabId> {
        self.current_tab()?.focused().map(|doc| doc.id)
    }

    /// Brings the next or the previous tab of the window in front to the front.
    pub(in crate::window) fn select_relative(&self, next: bool) {
        self.finish_restoring();
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

    pub(in crate::window) fn current_tab(&self) -> Option<Rc<TabWindow>> {
        self.ivars().documents.current.borrow().clone()
    }

    /// The window tabs, in the order they opened.
    pub(in crate::window) fn open_tabs(&self) -> Vec<Rc<TabWindow>> {
        self.ivars().documents.tabs.borrow().clone()
    }

    /// The window tab whose window is at the address `window`, which a callback for later carries instead of the
    /// window.
    pub(in crate::window) fn tab_at(&self, window: usize) -> Option<Rc<TabWindow>> {
        self.ivars()
            .documents
            .tabs
            .borrow()
            .iter()
            .find(|tab| Retained::as_ptr(&tab.window) as usize == window)
            .cloned()
    }

    pub(in crate::window) fn tab_of_window(&self, window: &NSWindow) -> Option<Rc<TabWindow>> {
        self.ivars()
            .documents
            .tabs
            .borrow()
            .iter()
            .find(|tab| std::ptr::eq(&*tab.window, window))
            .cloned()
    }

    pub(in crate::window) fn tab_of_notification(&self, notification: &NSNotification) -> Option<Rc<TabWindow>> {
        let object = notification.object()?;
        let window = object.downcast_ref::<NSWindow>()?;
        self.tab_of_window(window)
    }

    pub(in crate::window) fn tab_of_document(&self, id: TabId) -> Option<Rc<TabWindow>> {
        self.ivars()
            .documents
            .tabs
            .borrow()
            .iter()
            .find(|tab| tab.holds(id))
            .cloned()
    }

    pub(super) fn find_document(&self, id: TabId) -> Option<(Rc<TabWindow>, Rc<Document>)> {
        let tab = self.tab_of_document(id)?;
        let document = tab.document(id)?;
        Some((tab, document))
    }

    /// The window tabs in the order of their tab bars, tab group by tab group.
    pub(in crate::window) fn ordered_tabs(&self) -> Vec<Rc<TabWindow>> {
        let tabs = self.open_tabs();
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
    pub(in crate::window) fn documents(&self) -> Vec<Rc<Document>> {
        self.ordered_tabs().iter().flat_map(|tab| tab.documents()).collect()
    }

    /// Brings `tab` to the front of its tab group and gives it the sidebar.
    pub(super) fn select(&self, tab: &Rc<TabWindow>) {
        let _interval = ide_process::signpost::begin(ide_process::signpost::name!("tab"));
        if self.restoring_editor_tabs() {
            *self.ivars().documents.current.borrow_mut() = Some(tab.clone());
            return;
        }
        if let Some(group) = tab.window.tabGroup() {
            group.setSelectedWindow(Some(&tab.window));
        }
        // A tab draws the icon of its attributed title only when it gets the title in its tab group.
        tab.refresh();
        if self.current_tab().is_some_and(|current| current.window.isVisible()) || tab.window.isVisible() {
            tab.window.makeKeyAndOrderFront(None);
            self.sync_tab_bar(tab);
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
    pub(in crate::window) fn make_current(&self, tab: &Rc<TabWindow>) {
        if self.restoring_editor_tabs() {
            return;
        }
        let previous = self.current_tab();
        if previous.as_ref().is_some_and(|previous| Rc::ptr_eq(previous, tab)) {
            self.load_restored_editors(tab);
            self.list_front_tabs();
            return;
        }
        let _interval = ide_process::signpost::begin(ide_process::signpost::name!("tab front"));
        self.mark_action();
        let collapsed = previous
            .as_ref()
            .is_some_and(|previous| self.sidebar_leaves(previous, tab));
        *self.ivars().documents.current.borrow_mut() = Some(tab.clone());
        self.sidebar_joins(tab, collapsed);
        self.load_restored_editors(tab);
        self.list_front_tabs();
    }

    /// Another tab may have come to the front or on screen: the Software Factory's tabs and the CI logs that left out
    /// a change while out of view catch up on it, and the terminals now in view lose their red dot.
    pub(crate) fn tabs_came_forward(&self) {
        ide_ui_factory::came_to_front(self.handle(), self.context().factory_ui());
        crate::ci_panel::came_to_front(self);
        crate::terminal::came_to_front(self);
    }

    /// A window tab came on screen or left it: one on screen beside the tab in front shows a picture of the sidebar.
    pub(in crate::window) fn tab_visibility_changed(&self, tab: &Rc<TabWindow>) {
        if self.restoring_editor_tabs() {
            return;
        }
        let current = self.current_tab();
        if current.is_some_and(|current| Rc::ptr_eq(&current, tab)) || tab.has_picture() {
            return;
        }
        let visible = tab
            .window
            .occlusionState()
            .contains(objc2_app_kit::NSWindowOcclusionState::Visible);
        if visible {
            self.picture_sidebar_in(tab);
        }
    }

    /// Lists only the tab in front of each tab group in the Window menu and the Dock's menu, so a folder appears once
    /// there however many tabs it has; a tab dragged out into a window of its own stays listed.
    pub(in crate::window) fn list_front_tabs(&self) {
        let current = self.current_tab().map(|current| Retained::as_ptr(&current.window));
        // Each group's size and the window in front of it, asked of AppKit once per group.
        let mut fronts: HashMap<*const NSWindowTabGroup, (usize, Option<*const NSWindow>)> = HashMap::new();
        for tab in self.ivars().documents.tabs.borrow().iter() {
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

    /// Whether the window has one tab left, whose closing closes the window.
    pub(in crate::window) fn is_last_tab(&self) -> bool {
        self.ivars().documents.tabs.borrow().len() <= 1
    }

    /// A window tab closes: its documents go with it; the window goes with its last tab.
    pub(in crate::window) fn tab_closes(&self, tab: &Rc<TabWindow>) {
        let last = self.ivars().documents.tabs.borrow().len() == 1;
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
        let documents = &self.ivars().documents;
        documents.tabs.borrow_mut().retain(|other| !Rc::ptr_eq(other, tab));
        if self.current_tab().is_some_and(|current| Rc::ptr_eq(&current, tab)) {
            *documents.current.borrow_mut() = None;
            let next = neighbour.or_else(|| documents.tabs.borrow().first().cloned());
            if let Some(next) = next {
                if self.restoring_editor_tabs() {
                    self.select(&next);
                } else {
                    self.make_current(&next);
                    self.later(WindowController::follow_selection);
                }
            }
        }
        self.retitle_tabs(None);
        crate::session::changed();
    }

    /// Names every window tab after its document in front, told apart from tabs of the same name; `own` gets its
    /// title again even when it reads the same, as its icon or style may have changed.
    pub(in crate::window) fn retitle_tabs(&self, own: Option<&TabWindow>) {
        let shown: Vec<(Rc<TabWindow>, Rc<Document>)> = self
            .open_tabs()
            .into_iter()
            .filter_map(|tab| tab.named_after().map(|doc| (tab, doc)))
            .collect();
        let titles: Vec<String> = shown.iter().map(|(_, doc)| doc.title()).collect();
        let named: Vec<Named> = shown
            .iter()
            .zip(&titles)
            .map(|((_, doc), title)| Named {
                title,
                kind: doc.kind,
                key: doc.key.as_deref(),
            })
            .collect();
        for ((tab, _), title) in shown.iter().zip(tab_titles::distinct(&named)) {
            let is_own = own.is_some_and(|own| std::ptr::eq(&**tab, own));
            if is_own || tab.shown_title() != title {
                tab.show_title(&title);
            }
        }
    }

    /// The tab that comes to the front when `tab` closes, as AppKit picks it: the next one in its tab bar, else the one
    /// before.
    pub(super) fn neighbour(&self, tab: &TabWindow) -> Option<Rc<TabWindow>> {
        let windows: Vec<Retained<NSWindow>> = tab.window.tabbedWindows()?.iter().collect();
        let index = windows.iter().position(|w| std::ptr::eq(&**w, &*tab.window))?;
        windows
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|before| windows.get(before)))
            .and_then(|window| self.tab_of_window(window))
    }
}
