//! A split window (E145): while the tab in front is split, it is the window's only tab, its panes hold all the
//! window's documents and show their own tab bars in place of the window's; once one pane is left, its documents get
//! native tabs of their own again (E60).

use std::rc::Rc;

use objc2_app_kit::NSWindowOrderingMode;

use crate::window::panes::{Place, Side};
use crate::window::tabs::TabWindow;
use crate::window::{TabId, WindowController};

impl WindowController {
    /// Brings the window in line with the panes of the tab in front: a split takes in the documents of the other tabs
    /// and hides the window's tab bar; a tab no longer split gives each of its documents a tab again and shows it.
    pub(in crate::window) fn settle_split(&self) {
        let Some(tab) = self.current_tab() else { return };
        if tab.panes.is_split() {
            self.take_in_tabs(&tab);
        } else {
            self.spread_out(&tab);
        }
        self.sync_tab_bar(&tab);
        tab.refresh();
    }

    /// Settles the tabs a layout opened: a split one comes to the front and takes in the others, as an older layout
    /// may keep tabs beside it; without one, every document of a tab gets a tab of its own.
    pub(in crate::window) fn settle_restored(&self) {
        let tabs = self.ordered_tabs();
        match tabs.iter().find(|tab| tab.panes.is_split()) {
            Some(split) => {
                if let Some(front) = split.focused().map(|doc| doc.id) {
                    self.select_tab_by_id(front);
                }
                self.settle_split();
            }
            None => {
                for tab in &tabs {
                    self.spread_out(tab);
                    self.sync_tab_bar(tab);
                }
            }
        }
    }

    /// Settles a tab a layout opened after the others: a split tab in front takes it in, else every document of it
    /// gets a tab of its own.
    pub(in crate::window) fn settle_restored_tab(&self, tab: &Rc<TabWindow>) {
        if self.current_tab().is_some_and(|current| current.panes.is_split()) {
            self.settle_split();
        } else {
            self.spread_out(tab);
            self.sync_tab_bar(tab);
        }
    }

    /// Shows the window's tab bar while `tab` is not split, which then shows every document as a tab (E60), and hides
    /// it while the panes' bars take its place.
    pub(in crate::window) fn sync_tab_bar(&self, tab: &TabWindow) {
        let visible = !tab.panes.is_split();
        if tab
            .window
            .tabGroup()
            .is_some_and(|group| group.isTabBarVisible() != visible)
        {
            tab.window.toggleTabBar(None);
        }
    }

    /// Moves the documents of every other tab into the first pane of `tab`, in the order of the tab bar around its
    /// own, and closes those tabs; a preview stays behind and closes with its tab.
    fn take_in_tabs(&self, tab: &Rc<TabWindow>) {
        let ordered = self.ordered_tabs();
        if ordered.len() < 2 {
            return;
        }
        let own = ordered.iter().position(|other| Rc::ptr_eq(other, tab)).unwrap_or(0);
        let main = tab.panes.main();
        let front = tab.panes.front(main).map(|doc| doc.id);
        let mut before = 0;
        for (index, other) in ordered.iter().enumerate() {
            if Rc::ptr_eq(other, tab) {
                continue;
            }
            for document in other.documents() {
                let preview = other
                    .panes
                    .of_document(document.id)
                    .is_some_and(|pane| other.panes.is_companion(pane));
                if preview {
                    continue;
                }
                let Some(document) = other.remove(document.id) else {
                    continue;
                };
                let at = if index < own {
                    before += 1;
                    before - 1
                } else {
                    tab.panes.count(main)
                };
                tab.panes.insert(document, Place::Into(main, Some(at)));
            }
            other.window.close();
        }
        if let Some(front) = front {
            tab.panes.select(front);
        }
    }

    /// Gives each document of the one pane of `tab` but the one in front a native tab of its own, in the pane's order
    /// around it, its preview with it.
    fn spread_out(&self, tab: &Rc<TabWindow>) {
        let Some(spread) = tab.panes.spread() else { return };
        for (document, preview) in spread.before {
            if let Some(own) = self.tab_of_its_own(tab, document, preview) {
                tab.window
                    .addTabbedWindow_ordered(&own.window, NSWindowOrderingMode::Below);
                own.refresh();
            }
        }
        let mut last = tab.clone();
        for (document, preview) in spread.after {
            if let Some(own) = self.tab_of_its_own(tab, document, preview) {
                last.window
                    .addTabbedWindow_ordered(&own.window, NSWindowOrderingMode::Above);
                own.refresh();
                last = own;
            }
        }
        tab.panes.select(spread.front);
    }

    /// Moves the document `document` of `tab` into a new window tab of its own, with `preview` beside it; that tab,
    /// not yet in the tab group.
    fn tab_of_its_own(&self, tab: &TabWindow, document: TabId, preview: Option<TabId>) -> Option<Rc<TabWindow>> {
        let moved = tab.remove(document)?;
        let own = self.new_tab_window();
        own.insert(moved, Place::Active);
        if let Some(preview) = preview.and_then(|id| tab.remove(id)) {
            let beside = Place::Beside {
                anchor: document,
                side: Side::Right,
                companion: true,
            };
            own.insert(preview, beside);
        }
        Some(own)
    }
}
