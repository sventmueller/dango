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
}
