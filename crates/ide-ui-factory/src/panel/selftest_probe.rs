//! The self-test's hooks of `panel.rs`.

use super::*;

use crate::selftest::PanelSeen;

impl FactoryPanel {
    /// What the panel shows.
    pub(crate) fn seen(&self) -> PanelSeen {
        let list = self.list();
        PanelSeen {
            empty: !self.ui().empty.isHidden(),
            lines: list
                .lines()
                .iter()
                .map(|line| match line {
                    Line::Station(station, count) => format!("{} ({count})", crate::model::words::station(*station)),
                    Line::Order(order) => order.title().to_string(),
                })
                .collect(),
        }
    }
}
