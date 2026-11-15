//! What the self-test does with the panes' commands, kept out of their production code.

use super::{Side, TabId, WindowController};
use crate::window::documents::Target;

impl WindowController {
    /// The panes of the tab in front in a line, as `Panes::shape` writes them.
    pub(crate) fn pane_shape(&self) -> String {
        self.current_tab().map(|tab| tab.panes.shape()).unwrap_or_default()
    }

    /// Whether each pane of the tab in front shows its bar and shows at all.
    pub(crate) fn pane_bars(&self) -> Vec<(bool, bool)> {
        self.current_tab()
            .map(|tab| tab.panes.bars_and_shown())
            .unwrap_or_default()
    }

    /// Splits the active pane, as the menu's command does.
    pub(crate) fn split_pane(&self, side: Side) {
        self.split_active(side);
    }

    /// Fills the tab with the active pane, or shows the others again, as the menu's command does.
    pub(crate) fn zoom_pane(&self) {
        self.toggle_zoom();
    }

    /// Closes the active pane, as the menu's command does.
    pub(crate) fn close_pane(&self) {
        self.close_active_pane();
    }

    /// Lets go of the tab of `id` dragged to `x` and `y` of the documents of the pane `number`.
    pub(crate) fn drop_tab_on_pane(&self, id: TabId, number: usize, x: f64, y: f64) {
        let point = self.current_tab().and_then(|tab| tab.panes.point_in(number, x, y));
        if let Some(point) = point {
            self.drag_pane_tab(id, point);
            self.drop_pane_tab(id, point);
        }
    }

    /// The number of the pane of the tab in front that holds `id`.
    pub(crate) fn pane_of(&self, id: TabId) -> Option<usize> {
        self.current_tab()?.panes.of_document(id).map(|pane| pane.number())
    }

    /// The titles of the menu an Option-Shift click on a file in the tree opens now.
    pub(crate) fn file_pane_menu(&self) -> Vec<String> {
        let choices = self.pane_choices();
        self.with_file_tree(|files| files.pane_menu_titles(&choices))
            .unwrap_or_default()
    }

    /// Runs `open`, which opens a document, so that the document shows in a new pane right of the document `anchor`.
    pub(crate) fn open_beside(&self, anchor: TabId, open: impl FnOnce()) {
        self.open_at(
            Target::Beside {
                anchor,
                side: Side::Right,
                companion: false,
            },
            open,
        );
        self.settle_split();
    }
}
