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

    /// Clicks the file `path` in the tree with Option; whether the tree took the click as one on a file.
    pub(crate) fn option_click_file(&self, path: &std::path::Path) -> bool {
        self.with_file_tree(|files| files.option_click(path)).unwrap_or(false)
    }

    /// Picks the pane numbered `number` from the tree's Option-Shift menu of the file `path`.
    pub(crate) fn choose_pane_for_file(&self, path: &std::path::Path, number: usize) {
        let number = isize::try_from(number).unwrap_or(0);
        self.with_file_tree(|files| files.choose_pane(path, number));
    }

    /// Clicks the tab of the document `id` in its pane's bar, `count` times in a row.
    pub(crate) fn click_pane_tab(&self, id: TabId, count: isize) {
        self.pane_tab_clicked(id, count);
    }

    /// Whether the pane of `id` holds other documents too, as the bar's context menu asks.
    pub(crate) fn pane_has_more(&self, id: TabId) -> bool {
        self.pane_holds_more(id)
    }

    /// Closes the other documents of the pane of `id`, as the bar's context menu does.
    pub(crate) fn close_others_in_pane(&self, id: TabId) {
        self.close_other_pane_tabs(id);
    }

    /// Moves `id` into a new pane at `side`, as the bar's context menu does.
    pub(crate) fn split_off_pane(&self, id: TabId, side: Side) {
        self.split_off(id, side);
    }

    /// Closes the pane numbered `number`, as its bar's close button does.
    pub(crate) fn close_pane_numbered(&self, number: usize) {
        self.close_pane_by_tag(isize::try_from(number).ok());
    }

    /// Brings the document `step` places away in the active pane to the front, as the menu's command does.
    pub(crate) fn step_pane_tab(&self, step: isize) {
        self.select_pane_tab_relative(step);
    }

    /// The panes of the tab in front, the documents of the active one and whether it fills the tab, as the menu asks.
    pub(crate) fn pane_menu_state(&self) -> Option<(usize, usize, bool)> {
        self.pane_commands()
    }

    /// The side a split from the toolbar goes to with the keys held now.
    pub(crate) fn split_side(&self) -> Side {
        super::side_with_option()
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
