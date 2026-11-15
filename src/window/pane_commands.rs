//! What the user does with the panes of a window tab (E145): splitting, closing a pane, filling the tab with one pane,
//! moving through a pane's documents, and what a pane's tabs do.

use objc2_foundation::NSPoint;

use super::panes::{Place, Side};
use super::{TabId, WindowController};

#[cfg(feature = "selftest")]
mod selftest_probe;

impl WindowController {
    /// A click on the tab of the document `id` in a pane's bar; a double click keeps a preview.
    pub(super) fn pane_tab_clicked(&self, id: TabId, count: isize) {
        if count >= 2 {
            self.keep_tab(id);
        }
        if let Some(tab) = self.tab_of_document(id) {
            tab.activate(id);
            crate::session::changed();
        }
    }

    pub(super) fn drag_pane_tab(&self, id: TabId, point: NSPoint) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.panes.drag(id, point);
        }
    }

    pub(super) fn drop_pane_tab(&self, id: TabId, point: NSPoint) {
        let Some(tab) = self.tab_of_document(id) else { return };
        if tab.panes.drop(id, point) {
            tab.activate(id);
            crate::session::changed();
        }
    }

    /// Whether the pane of the document `id` holds another one.
    pub(super) fn pane_holds_more(&self, id: TabId) -> bool {
        self.tab_of_document(id)
            .and_then(|tab| tab.panes.of_document(id))
            .is_some_and(|pane| pane.count() > 1)
    }

    /// Closes the other documents of the pane of `id`, each once it agrees.
    pub(super) fn close_other_pane_tabs(&self, id: TabId) {
        let Some(pane) = self.tab_of_document(id).and_then(|tab| tab.panes.of_document(id)) else {
            return;
        };
        for other in pane.documents().iter().filter(|doc| doc.id != id) {
            self.close_tab(other.id);
        }
    }

    /// Moves the document `id` into a new pane at `side` of its own, which keeps its other documents.
    pub(super) fn split_off(&self, id: TabId, side: Side) {
        let Some(tab) = self.tab_of_document(id) else { return };
        if tab.panes.split_off(id, side) {
            tab.activate(id);
            crate::session::changed();
        }
    }

    /// Moves the document `id` out of its pane into a window tab of its own.
    pub(super) fn move_to_window_tab(&self, id: TabId) {
        let Some(from) = self.tab_of_document(id) else { return };
        let Some(document) = from.remove(id) else { return };
        let tab = self.new_tab_after_current();
        tab.insert(document, Place::Active);
        if from.is_empty() {
            from.window.close();
        }
        self.select_tab_by_id(id);
        if let Some(tab) = self.tab_of_document(id) {
            tab.activate(id);
        }
        crate::session::changed();
    }

    /// Splits the active pane of the tab in front at `side`: its document in front moves into the new pane while it
    /// has others, else the new pane opens empty and active, for the next document.
    pub(super) fn split_active(&self, side: Side) {
        let Some(tab) = self.current_tab() else { return };
        let pane = tab.panes.active();
        match pane.front() {
            Some(front) if pane.count() > 1 => self.split_off(front.id, side),
            _ => {
                let new = tab.panes.add_beside(&pane, side);
                tab.activate_pane(&new);
                crate::session::changed();
            }
        }
    }

    /// Closes the active pane of the tab in front with its documents, each once it agrees.
    pub(super) fn close_active_pane(&self) {
        self.close_pane_by_tag(None);
    }

    /// Closes the pane numbered `number` in the tab in front, or its active pane, with its documents once they all
    /// agree; an empty pane goes unless it is the last.
    pub(super) fn close_pane_by_tag(&self, number: Option<isize>) {
        let Some(tab) = self.current_tab() else { return };
        let pane = number
            .and_then(|number| usize::try_from(number).ok())
            .filter(|number| *number > 0)
            .and_then(|number| tab.panes.by_number(number))
            .unwrap_or_else(|| tab.panes.active());
        let documents = pane.documents();
        if documents.is_empty() {
            tab.panes.remove_pane(&pane);
            let active = tab.panes.active();
            tab.activate_pane(&active);
            return;
        }
        if !self.documents_may_close(&documents) {
            return;
        }
        for document in documents {
            self.close_tab_now(document.id);
        }
    }

    /// Fills the tab in front with its active pane alone, or shows the other panes again.
    pub(super) fn toggle_zoom(&self) {
        let Some(tab) = self.current_tab() else { return };
        let pane = tab.panes.active();
        tab.panes.toggle_zoom(&pane);
        tab.activate_pane(&pane);
    }

    /// Brings the document `step` places from the one in front of the active pane to the front.
    pub(super) fn select_pane_tab_relative(&self, step: isize) {
        let Some(tab) = self.current_tab() else { return };
        let pane = tab.panes.active();
        pane.select_relative(step);
        if let Some(front) = pane.front() {
            tab.activate(front.id);
        }
    }

    /// Whether the tab in front has panes to move through, to close or to fill it with.
    pub(super) fn pane_commands(&self) -> Option<(usize, usize, bool)> {
        let tab = self.current_tab()?;
        let pane = tab.panes.active();
        Some((tab.panes.all().len(), pane.count(), tab.panes.is_zoomed()))
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
            .iter()
            .map(|pane| (pane.number, pane.front().map(|doc| doc.title())))
            .collect()
    }
}
