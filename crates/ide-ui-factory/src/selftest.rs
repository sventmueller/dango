//! What the self-test reads from a window's Leitstand: its panel and its tab, through the views they show.

use std::rc::Rc;

use ide_ui_common::window::{Tabs, Window};

use crate::host::State;

/// What the Leitstand panel of `window` shows.
pub fn panel(window: Window, state: &Rc<State>) -> Option<PanelSeen> {
    state.panels.of(window).map(|panel| panel.seen())
}

/// What the Leitstand tab of `window` shows, while it has one.
pub fn board(window: Window, state: &Rc<State>) -> Option<BoardSeen> {
    let tab = window.tab_with_key(crate::board::TAB_KEY)?;
    state
        .boards
        .live()
        .into_iter()
        .find(|board| board.tab() == Some(tab))
        .map(|board| board.seen())
}

/// Opens the figures' section of the Leitstand tab of `window`; whether the window has the tab.
pub fn open_metrics(window: Window, state: &Rc<State>) -> bool {
    let Some(tab) = window.tab_with_key(crate::board::TAB_KEY) else {
        return false;
    };
    let board = state.boards.live().into_iter().find(|board| board.tab() == Some(tab));
    board.inspect(|board| board.open_metrics()).is_some()
}

/// What a panel shows, as the self-test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelSeen {
    /// The empty state shows instead of the list.
    pub empty: bool,
    /// The list's lines: a station's heading or an order's title.
    pub lines: Vec<String>,
}

/// What a Leitstand tab shows, as the self-test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSeen {
    /// The strip's buttons, such as "Bau 2".
    pub strip: Vec<String>,
    /// The table's rows.
    pub rows: usize,
    /// The table's columns, by their header.
    pub columns: Vec<String>,
    /// The note the tab shows without rows, empty while it has some.
    pub empty: String,
    /// The figures' section under the fleet line.
    pub metrics: MetricsSeen,
}

/// What the figures' section of a Leitstand tab shows, as the self-test reads it (E180).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsSeen {
    /// The disclosure's title, such as "Kennzahlen, 2 Zeilen".
    pub title: String,
    /// The lines above the table: what the figures rest on, the median wait, the tie-break's minimum.
    pub lines: Vec<String>,
    /// The table's rows.
    pub rows: usize,
    /// The text of each row's cells, in the order sorted.
    pub cells: Vec<Vec<String>>,
    /// The table's columns, by their header.
    pub columns: Vec<String>,
    /// Whether the table shows.
    pub table_shown: bool,
}
