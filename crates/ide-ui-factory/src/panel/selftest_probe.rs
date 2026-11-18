//! The self-test's hooks of `panel.rs`.

use super::*;

use crate::selftest::{PanelRowSeen, PanelSeen};

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

    /// What each row of the outline shows, its view made as scrolling it into sight does.
    pub(crate) fn rows_seen(&self) -> Vec<PanelRowSeen> {
        let outline = &self.ui().outline;
        (0..outline.numberOfRows())
            .filter_map(|row| outline.viewAtColumn_row_makeIfNecessary(0, row, true))
            .map(|view| {
                let mut seen = PanelRowSeen {
                    texts: Vec::new(),
                    symbols: 0,
                };
                collect(&view, &mut seen);
                seen
            })
            .collect()
    }
}

fn collect(view: &NSView, seen: &mut PanelRowSeen) {
    if let Some(field) = view.downcast_ref::<NSTextField>() {
        seen.texts.push(field.stringValue().to_string());
        return;
    }
    if view.downcast_ref::<NSImageView>().is_some() {
        seen.symbols += 1;
        return;
    }
    for child in view.subviews().iter() {
        collect(&child, seen);
    }
}
