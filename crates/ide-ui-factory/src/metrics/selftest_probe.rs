//! The self-test's hooks of `metrics.rs`.

use super::*;

use crate::selftest::MetricsSeen;

impl MetricsSection {
    /// Opens the section, as a click on its disclosure does.
    pub(crate) fn open(&self) {
        if !self.view().open() {
            self.ivars().view.borrow_mut().toggle();
            self.show();
        }
    }

    /// What the section shows.
    pub(crate) fn seen(&self) -> MetricsSeen {
        let ui = self.ui();
        let view = self.view();
        MetricsSeen {
            title: ui.title.stringValue().to_string(),
            lines: view.lines().to_vec(),
            rows: usize::try_from(ui.table.numberOfRows()).unwrap_or(0),
            cells: view
                .rows()
                .iter()
                .map(|row| {
                    METRICS_COLUMNS
                        .iter()
                        .map(|(column, _)| row.cell(*column).to_string())
                        .collect()
                })
                .collect(),
            columns: ui
                .table
                .tableColumns()
                .iter()
                .map(|column| column.title().to_string())
                .collect(),
            table_shown: !ui.scroll.isHidden(),
        }
    }
}
