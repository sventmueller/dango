//! What the user does with the panes of a window tab (E145): splitting, closing a pane, filling the tab with one pane,
//! moving through a pane's documents, and what a pane's tabs do.

use objc2_foundation::NSPoint;

use crate::window::panes::Side;
use crate::window::{TabId, WindowController};

#[cfg(feature = "selftest")]
mod selftest_probe;

impl WindowController {
    /// A click on the tab of the document `id` in a pane's bar; a double click keeps a preview.
    pub(in crate::window) fn pane_tab_clicked(&self, id: TabId, count: isize) {
        if count >= 2 {
            self.keep_tab(id);
        }
        if let Some(tab) = self.tab_of_document(id) {
            tab.activate(id);
            crate::session::changed();
        }
    }

    pub(in crate::window) fn drag_pane_tab(&self, id: TabId, point: NSPoint) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.panes.drag(id, point);
        }
    }

    pub(in crate::window) fn drop_pane_tab(&self, id: TabId, point: NSPoint) {
        let Some(tab) = self.tab_of_document(id) else { return };
        if tab.panes.drop(id, point) {
            self.settle_split();
            if let Some(tab) = self.tab_of_document(id) {
                tab.activate(id);
            }
            crate::session::changed();
        }
    }

    /// Whether the pane of the document `id` holds another one.
    pub(in crate::window) fn pane_holds_more(&self, id: TabId) -> bool {
        self.tab_of_document(id)
            .is_some_and(|tab| tab.panes.of_document(id).is_some_and(|pane| tab.panes.count(pane) > 1))
    }

    /// Closes the other documents of the pane of `id`, each once it agrees.
    pub(in crate::window) fn close_other_pane_tabs(&self, id: TabId) {
        let Some(tab) = self.tab_of_document(id) else { return };
        let Some(pane) = tab.panes.of_document(id) else { return };
        for other in tab.panes.documents_in(pane).iter().filter(|doc| doc.id != id) {
            self.close_tab(other.id);
        }
    }

    /// Moves the document `id` into a new pane at `side` of its own, which keeps its other documents.
    pub(in crate::window) fn split_off(&self, id: TabId, side: Side) {
        let Some(tab) = self.tab_of_document(id) else { return };
        if tab.panes.split_off(id, side) {
            self.settle_split();
            tab.activate(id);
            crate::session::changed();
        }
    }

    /// Splits the active pane of the tab in front at `side`: its document in front moves into the new pane while it
    /// has others, else the new pane opens empty and active, for the next document.
    pub(in crate::window) fn split_active(&self, side: Side) {
        let Some(tab) = self.current_tab() else { return };
        let Some((new, moved)) = tab.panes.split_active(side) else {
            return;
        };
        self.settle_split();
        match moved {
            Some(moved) => tab.activate(moved),
            None => tab.activate_pane(new),
        }
        crate::session::changed();
    }

    /// Closes the active pane of the tab in front, as `close_pane_by_tag` does.
    pub(in crate::window) fn close_active_pane(&self) {
        self.close_pane_by_tag(None);
    }

    /// Closes the pane numbered `number` in the tab in front, or its active pane: its documents move into the pane
    /// before it, else the one after; a preview's pane and the last pane close with their documents once they all
    /// agree, and an empty pane goes unless it is the last.
    pub(in crate::window) fn close_pane_by_tag(&self, number: Option<isize>) {
        let Some(tab) = self.current_tab() else { return };
        let pane = number
            .and_then(|number| usize::try_from(number).ok())
            .filter(|number| *number > 0)
            .and_then(|number| tab.panes.by_number(number))
            .unwrap_or_else(|| tab.panes.active());
        let documents = tab.panes.documents_in(pane);
        if documents.is_empty() {
            tab.panes.remove_pane(pane);
            self.settle_split();
            tab.activate_pane(tab.panes.active());
            return;
        }
        if let Some(heir) = tab.panes.close_pane(pane) {
            let front = tab.panes.front(heir).map(|document| document.id);
            self.settle_split();
            if let Some(front) = front
                && let Some(tab) = self.tab_of_document(front)
            {
                tab.activate(front);
            }
            crate::session::changed();
            return;
        }
        if !self.documents_may_close(&documents) {
            return;
        }
        for document in documents {
            self.close_tab_now(document.id);
        }
    }

    /// Makes the pane numbered `number` the active one and opens the menu of new tabs below its "+" `button`, so what
    /// the user picks opens in that pane.
    pub(in crate::window) fn new_tab_in_pane(&self, number: isize, button: Option<&objc2_app_kit::NSView>) {
        let Some(tab) = self.current_tab() else { return };
        let pane = usize::try_from(number)
            .ok()
            .and_then(|number| tab.panes.by_number(number));
        if let Some(pane) = pane {
            tab.activate_pane(pane);
        }
        match button {
            Some(button) => crate::window::new_tab_button::open_below(self, button),
            None => crate::window::new_tab_button::open(self),
        }
    }

    /// Fills the tab in front with its active pane alone, or shows the other panes again.
    pub(in crate::window) fn toggle_zoom(&self) {
        let Some(tab) = self.current_tab() else { return };
        let pane = tab.panes.active();
        tab.panes.toggle_zoom(pane);
        tab.activate_pane(pane);
    }

    /// Brings the document `step` places from the one in front of the active pane to the front.
    pub(in crate::window) fn select_pane_tab_relative(&self, step: isize) {
        let Some(tab) = self.current_tab() else { return };
        let pane = tab.panes.active();
        tab.panes.select_relative(step);
        if let Some(front) = tab.panes.front(pane) {
            tab.activate(front.id);
        }
    }

    /// Whether the tab in front has panes to move through, to close or to fill it with.
    pub(in crate::window) fn pane_commands(&self) -> Option<(usize, usize, bool)> {
        let tab = self.current_tab()?;
        let pane = tab.panes.active();
        Some((tab.panes.all().len(), tab.panes.count(pane), tab.panes.is_zoomed()))
    }
}

impl WindowController {
    /// The panes of the tab in front, each by its number and the title of its document in front.
    pub(crate) fn pane_choices(&self) -> Vec<(usize, Option<String>)> {
        let Some(tab) = self.current_tab() else {
            return Vec::new();
        };
        tab.panes
            .all()
            .into_iter()
            .map(|pane| (pane.number(), tab.panes.front(pane).map(|doc| doc.title())))
            .collect()
    }
}

/// The side a split from the toolbar or the menu goes to: below with Option held, else right, as Xcode's "Add Editor"
/// does.
pub(in crate::window) fn side_with_option() -> Side {
    let mtm = objc2::MainThreadMarker::new();
    let held = mtm
        .and_then(|mtm| objc2_app_kit::NSApplication::sharedApplication(mtm).currentEvent())
        .is_some_and(|event| {
            event
                .modifierFlags()
                .contains(objc2_app_kit::NSEventModifierFlags::Option)
        });
    if held { Side::Below } else { Side::Right }
}
