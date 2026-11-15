//! A split window (E145): while the tab in front is split, it is the window's only tab, its panes hold all the
//! window's documents and show their own tab bars in place of the window's; once one pane is left, its documents get
//! native tabs of their own again (E60).

use std::rc::Rc;

use objc2_app_kit::NSWindowOrderingMode;

use super::WindowController;
use super::panes::{Place, Side};
use super::tabs::TabWindow;

impl WindowController {
    /// Brings the window in line with the panes of the tab in front: a split takes in the documents of the other tabs
    /// and hides the window's tab bar; a tab no longer split gives each of its documents a tab again and shows it.
    pub(super) fn settle_split(&self) {
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
    pub(super) fn settle_restored(&self) {
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

    /// Shows the window's tab bar while `tab` is not split, which then shows every document as a tab (E60), and hides
    /// it while the panes' bars take its place.
    pub(super) fn sync_tab_bar(&self, tab: &TabWindow) {
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
        let front = main.front().map(|doc| doc.id);
        let mut before = 0;
        for (index, other) in ordered.iter().enumerate() {
            if Rc::ptr_eq(other, tab) {
                continue;
            }
            for document in other.documents() {
                let preview = other
                    .panes
                    .of_document(document.id)
                    .is_some_and(|pane| pane.companion_of.get().is_some());
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
                    main.count()
                };
                main.insert(document, Some(at));
            }
            other.window.close();
        }
        if let Some(front) = front {
            main.select(front);
        }
    }

    /// Gives each document of the one pane of `tab` but the one in front a native tab of its own, in the pane's order
    /// around it, its preview with it.
    fn spread_out(&self, tab: &Rc<TabWindow>) {
        let main = tab.panes.main();
        let documents = main.documents();
        let Some(front) = main.front() else { return };
        if documents.len() < 2 {
            return;
        }
        let at_front = documents.iter().position(|doc| doc.id == front.id).unwrap_or(0);
        let mut last = tab.clone();
        for (index, document) in documents.iter().enumerate() {
            if index == at_front {
                continue;
            }
            let preview = tab
                .panes
                .companion_of(document.id)
                .and_then(|pane| pane.front())
                .map(|doc| doc.id);
            let Some(moved) = tab.remove(document.id) else { continue };
            let own = self.new_tab_window();
            own.insert(moved, Place::Active);
            if let Some(preview) = preview.and_then(|id| tab.remove(id)) {
                let beside = Place::Beside {
                    anchor: document.id,
                    side: Side::Right,
                    companion: true,
                };
                own.insert(preview, beside);
            }
            if index < at_front {
                tab.window
                    .addTabbedWindow_ordered(&own.window, NSWindowOrderingMode::Below);
            } else {
                last.window
                    .addTabbedWindow_ordered(&own.window, NSWindowOrderingMode::Above);
                last = own.clone();
            }
            own.refresh();
        }
        main.select(front.id);
    }
}
