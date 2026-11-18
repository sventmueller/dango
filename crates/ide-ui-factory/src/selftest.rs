//! What the self-test reads from a window's Leitstand: its panel and its tab, through the views they show.

use std::rc::Rc;

use ide_ui_common::window::{Tabs, Window};

use crate::host::State;

/// What the Leitstand panel of `window` shows.
pub fn panel(window: Window, state: &Rc<State>) -> Option<PanelSeen> {
    state.panels.of(window).map(|panel| panel.seen())
}

/// What each row of the Leitstand panel of `window` shows, its view made as scrolling it into sight does.
pub fn panel_rows(window: Window, state: &Rc<State>) -> Option<Vec<PanelRowSeen>> {
    state.panels.of(window).map(|panel| panel.rows_seen())
}

/// What the sheet "Neuer Auftrag" on `window` shows, while it is open.
pub fn order_sheet(window: Window) -> Option<SheetSeen> {
    crate::sheet::OrderSheet::on(window).map(|sheet| sheet.seen())
}

/// Fills the fields of the sheet "Neuer Auftrag" on `window` with `typed`, as the user types; whether it is open.
pub fn type_into_order_sheet(window: Window, typed: &SheetTyped) -> bool {
    crate::sheet::OrderSheet::on(window)
        .inspect(|sheet| sheet.type_in(typed))
        .is_some()
}

/// Clicks `control` of the sheet "Neuer Auftrag" on `window`; whether the sheet is open and has it.
pub fn press_order_sheet(window: Window, control: SheetControl) -> bool {
    crate::sheet::OrderSheet::on(window).is_some_and(|sheet| sheet.press(control))
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

/// What the tab of the order `order` in `window` shows, while the window has it.
pub fn order(window: Window, state: &Rc<State>, order: &ide_factory::OrderId) -> Option<OrderSeen> {
    let key = crate::order::tab_key(order);
    let tab = window.tab_with_key(&key)?;
    state
        .orders
        .live()
        .into_iter()
        .find(|order_tab| order_tab.tab() == Some(tab))
        .map(|order_tab| order_tab.seen())
}

/// Runs `command` in the tab of the order `order` in `window` as its button does; whether the window has the tab.
pub fn press_order(
    window: Window,
    state: &Rc<State>,
    order: &ide_factory::OrderId,
    command: crate::model::detail::Command,
) -> bool {
    let key = crate::order::tab_key(order);
    let Some(tab) = window.tab_with_key(&key) else {
        return false;
    };
    state
        .orders
        .live()
        .into_iter()
        .find(|order_tab| order_tab.tab() == Some(tab))
        .inspect(|order_tab| order_tab.press(command))
        .is_some()
}

/// The dialog "Software Factory für <Ordner>" open on `window`.
fn project_sheet(window: Window, state: &Rc<State>) -> Option<objc2::rc::Retained<crate::project::ProjectSheet>> {
    state
        .projects
        .live()
        .into_iter()
        .find(|sheet| sheet.window() == window && sheet.open_now())
}

/// What each row of the dialog "Software Factory für <Ordner>" on `window` shows, while it is open.
pub fn project(window: Window, state: &Rc<State>) -> Option<Vec<ProjectRowSeen>> {
    project_sheet(window, state).map(|sheet| sheet.seen())
}

/// Presses "Auf Vorgabe zurücksetzen" of `value` in the dialog on `window`; whether the button could be pressed.
pub fn reset_project_value(window: Window, state: &Rc<State>, value: crate::model::settings::ProjectValue) -> bool {
    project_sheet(window, state).is_some_and(|sheet| sheet.press_reset(value))
}

/// Presses "Sichern" in the dialog on `window`; whether it was open.
pub fn save_project(window: Window, state: &Rc<State>) -> bool {
    project_sheet(window, state)
        .inspect(|sheet| sheet.press_save())
        .is_some()
}

/// What an order's tab shows, as the self-test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderSeen {
    /// The texts of its condition line and its body, in their order.
    pub texts: Vec<String>,
    /// Every button in its body and every command of its pull-down, with whether it can be pressed.
    pub buttons: Vec<(String, bool)>,
    /// The titles of the buttons in its command bar, the next step first.
    pub bar: Vec<String>,
}

/// What a row of the dialog "Software Factory für <Ordner>" shows, as the self-test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRowSeen {
    /// The row's title, such as "Prüfer:".
    pub title: String,
    /// The value as it shows.
    pub value: String,
    /// "Vorgabe" or "factory.toml".
    pub origin: String,
    /// Whether "Auf Vorgabe zurücksetzen" can be pressed.
    pub resettable: bool,
}

/// What a panel shows, as the self-test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelSeen {
    /// The empty state shows instead of the list.
    pub empty: bool,
    /// The list's lines: a station's heading or an order's title.
    pub lines: Vec<String>,
}

/// What a row of the panel's list shows, as the self-test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelRowSeen {
    /// Its texts: a station's heading, or an order's title and detail.
    pub texts: Vec<String>,
    /// How many symbols it shows: an order's condition, and the red dot while it waits.
    pub symbols: usize,
}

/// What the sheet "Neuer Auftrag" shows, as the self-test reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetSeen {
    /// The title's field.
    pub title: String,
    /// The goal's text.
    pub goal: String,
    /// Every label and note that shows, such as the note that the text is an issue's.
    pub labels: Vec<String>,
    /// The lines of what keeps the order back and its notes; empty while there are none.
    pub messages: Vec<String>,
    /// Whether "Starten" can be pressed.
    pub start: bool,
    /// Whether the options show.
    pub options: bool,
    /// Whether "Spezifikation ohne Freigabe" can be ticked.
    pub skip: bool,
    /// The issue's open tasks, each with whether it is ticked.
    pub suggestions: Vec<(String, bool)>,
}

/// What the self-test types into the sheet "Neuer Auftrag".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetTyped {
    /// The title.
    pub title: String,
    /// The goal.
    pub goal: String,
    /// The criteria, one per line.
    pub criteria: String,
    /// The path scope.
    pub scope: String,
    /// The budget in dollars.
    pub budget: String,
}

/// A control of the sheet "Neuer Auftrag" the self-test clicks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetControl {
    /// The disclosure of the options.
    Options,
    /// The risk choice at this index of the popup.
    Risk(usize),
    /// The box "Spezifikation ohne Freigabe".
    Skip,
    /// The box of the issue's open task at this index.
    Suggestion(usize),
    /// "Abbrechen".
    Cancel,
    /// "Starten".
    Start,
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
