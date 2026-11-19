//! The self-test's hooks of `board.rs`.

use super::*;

use crate::selftest::BoardSeen;

impl BoardTab {
    /// What the tab shows.
    pub(crate) fn seen(&self) -> BoardSeen {
        if self.ivars().stale.get() {
            self.show_changes(crate::now());
        }
        let ui = self.ui();
        let view = self.view();
        BoardSeen {
            strip: view.strip().iter().map(|stop| stop.title()).collect(),
            rows: usize::try_from(ui.table.numberOfRows()).unwrap_or(0),
            columns: ui
                .table
                .tableColumns()
                .iter()
                .map(|column| column.title().to_string())
                .collect(),
            empty: if ui.empty.isHidden() {
                String::new()
            } else {
                ui.empty.stringValue().to_string()
            },
            metrics: ui.metrics.seen(),
        }
    }

    /// Opens the figures' section under the fleet line, as a click on its disclosure does.
    pub(crate) fn open_metrics(&self) {
        self.ui().metrics.open();
    }
}
