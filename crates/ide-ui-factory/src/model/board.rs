//! The Leitstand tab (E175, the Kanban board struck by the owner): the fleet line, the station strip with a count per
//! station that filters the table, and the sortable table whose filter and sort survive each new board.

use ide_factory::OrderId;
use ide_factory::panel::{Board, Column, Fleet, Row};
use ide_factory::station::Station;
use splat_i18n::{t, tf};

use super::list::agent;
use super::words::{self, Tone};

/// The table's columns in their order, with the key a column's identifier and sort descriptor carry.
pub const COLUMNS: [(Column, &str); 6] = [
    (Column::Title, "title"),
    (Column::Station, "station"),
    (Column::Agent, "agent"),
    (Column::Round, "round"),
    (Column::Spend, "spend"),
    (Column::WaitingSince, "waiting"),
];

/// The title of `column`'s header.
pub fn column_title(column: Column) -> &'static str {
    match column {
        Column::Title => t("Titel"),
        Column::Station => t("Station"),
        Column::Agent => t("Agent"),
        Column::Round => t("Prüfrunde"),
        Column::Spend => t("Verbrauch"),
        Column::WaitingSince => t("Wartet seit"),
    }
}

/// The column of `key`.
pub fn column_of(key: &str) -> Option<Column> {
    COLUMNS.iter().find(|(_, k)| *k == key).map(|(column, _)| *column)
}

/// The key of `column`.
pub fn key_of(column: Column) -> &'static str {
    COLUMNS
        .iter()
        .find(|(c, _)| *c == column)
        .map_or("title", |(_, key)| key)
}

/// One station of the strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    station: Station,
    count: usize,
    active: bool,
}

impl Stop {
    /// The station.
    pub fn station(&self) -> Station {
        self.station
    }

    /// How many orders stand there.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Whether the table shows only this station.
    pub fn active(&self) -> bool {
        self.active
    }

    /// "Bau 2".
    pub fn title(&self) -> String {
        tf("{} {}", &[words::station(self.station), &self.count.to_string()])
    }

    /// What VoiceOver reads: "Bau, 2 Aufträge, gefiltert".
    pub fn spoken(&self) -> String {
        let count = match self.count {
            1 => t("1 Auftrag").to_string(),
            n => tf("{} Aufträge", &[&n.to_string()]),
        };
        if self.active {
            tf("{}, {}, gefiltert", &[words::station(self.station), &count])
        } else {
            tf("{}, {}", &[words::station(self.station), &count])
        }
    }
}

/// One row of the table, as text per column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    id: OrderId,
    tone: Tone,
    cells: [String; 6],
}

impl TableRow {
    fn of(row: &Row, now: u64) -> TableRow {
        let waiting = row
            .waiting_since()
            .map(|since| words::duration(now.saturating_sub(since)))
            .unwrap_or_default();
        let round = match row.round() {
            0 => String::new(),
            n => n.to_string(),
        };
        TableRow {
            id: row.id().clone(),
            tone: Tone::of(row.station(), row.condition()),
            cells: [
                row.title().to_string(),
                tf(
                    "{} – {}",
                    &[
                        words::station(row.station()),
                        &words::condition(row.station(), row.condition()),
                    ],
                ),
                agent(row.agent(), row.model()),
                round,
                words::spend(row.spend()),
                waiting,
            ],
        }
    }

    /// The order.
    pub fn id(&self) -> &OrderId {
        &self.id
    }

    /// Its condition's shape and word, beside the station.
    pub fn tone(&self) -> Tone {
        self.tone
    }

    /// The text of `column`.
    pub fn cell(&self, column: Column) -> &str {
        let index = COLUMNS.iter().position(|(c, _)| *c == column).unwrap_or(0);
        &self.cells[index]
    }
}

/// The tab's strip and table, with the filter and sort the user chose.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardView {
    board: Option<Board>,
    filter: Option<Station>,
    sort: Column,
    ascending: bool,
    now: u64,
}

impl Default for BoardView {
    fn default() -> Self {
        BoardView {
            board: None,
            filter: None,
            sort: Column::WaitingSince,
            ascending: true,
            now: 0,
        }
    }
}

impl BoardView {
    /// Takes `board`, read at `now` (wall clock), with the filter and sort kept.
    pub fn set(&mut self, board: Option<Board>, now: u64) {
        self.now = now;
        self.board = board;
        self.apply();
    }

    fn apply(&mut self) {
        let Some(board) = self.board.as_mut() else { return };
        // `Board::filter` turns a filter off when it is given the active one again, so it hears only a change.
        if board.active_filter() != self.filter {
            board.filter(self.filter.or(board.active_filter()));
        }
        for _ in 0..2 {
            if board.sorting() != (self.sort, self.ascending) {
                board.sort_by(self.sort);
            }
        }
    }

    /// A click on `station` in the strip: it filters the table, or clears the filter if it was the one shown.
    pub fn click(&mut self, station: Station) {
        self.filter = if self.filter == Some(station) {
            None
        } else {
            Some(station)
        };
        self.apply();
    }

    /// Sorts by `column` in `ascending` order, as the table's header asks.
    pub fn sort(&mut self, column: Column, ascending: bool) {
        self.sort = column;
        self.ascending = ascending;
        self.apply();
    }

    /// The column and direction sorted by.
    pub fn sorting(&self) -> (Column, bool) {
        (self.sort, self.ascending)
    }

    /// The station the table shows alone.
    pub fn filter(&self) -> Option<Station> {
        self.filter
    }

    /// The strip, every station with its count.
    pub fn strip(&self) -> Vec<Stop> {
        let counts = self
            .board
            .as_ref()
            .map(Board::strip)
            .unwrap_or_else(|| Station::ALL.into_iter().map(|station| (station, 0)).collect());
        counts
            .into_iter()
            .map(|(station, count)| Stop {
                station,
                count,
                active: self.filter == Some(station),
            })
            .collect()
    }

    /// The rows the filter lets through, in the order sorted.
    pub fn rows(&self) -> Vec<TableRow> {
        self.board
            .as_ref()
            .map(|board| {
                board
                    .rows()
                    .into_iter()
                    .map(|row| TableRow::of(row, self.now))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether the folder has no order at all.
    pub fn is_empty(&self) -> bool {
        self.board
            .as_ref()
            .is_none_or(|board| board.strip().iter().all(|(_, n)| *n == 0))
    }

    /// What the table says when it has no rows.
    pub fn empty_words(&self) -> &'static str {
        if self.is_empty() {
            t(
                "Noch kein Auftrag. Ein Auftrag ist ein Ziel mit prüfbaren Kriterien, das Dango in einem eigenen Worktree baut und prüft.",
            )
        } else {
            t("Kein Auftrag an dieser Station.")
        }
    }
}

/// The fleet line's parts (E175).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FleetWords {
    orders: String,
    quotas: Vec<String>,
    day: String,
    disk: String,
}

impl FleetWords {
    /// The words of `fleet` at `now` (wall clock); a folder without a factory shows zeros.
    pub fn new(fleet: Option<&Fleet>, now: u64) -> FleetWords {
        let Some(fleet) = fleet else {
            return FleetWords {
                orders: tf("{} laufen · {} warten · {} gescheitert", &["0", "0", "0"]),
                ..FleetWords::default()
            };
        };
        let orders = tf(
            "{} laufen · {} warten · {} gescheitert · höchstens {} gleichzeitig",
            &[
                &fleet.running().to_string(),
                &fleet.waiting().to_string(),
                &fleet.failed().to_string(),
                &fleet.parallel().to_string(),
            ],
        );
        let quotas = fleet
            .quotas()
            .iter()
            .map(|quota| {
                let used = match (quota.current(), quota.utilization()) {
                    (true, Some(share)) => tf("{} %", &[&format!("{:.0}", share * 100.0)]),
                    _ => t("unbekannt").into(),
                };
                let seen = words::duration(now.saturating_sub(quota.observed_at()));
                tf(
                    "{} ({}), {}: {}, gesehen vor {}",
                    &[quota.account(), quota.service(), quota.window(), &used, &seen],
                )
            })
            .collect();
        let wait = fleet
            .median_wait_ms()
            .map_or_else(|| t("keine").to_string(), words::duration);
        let (spent, done) = (words::spend(fleet.day_spent()), fleet.done_today().to_string());
        let day = match fleet.day_budget().dollars() {
            Some(budget) => tf(
                "Heute: {} von {} · {} erledigt · Wartezeit im Median: {}",
                &[&spent, &words::dollars(budget), &done, &wait],
            ),
            None => tf(
                "Heute: {} verbraucht, ohne Tagesgrenze · {} erledigt · Wartezeit im Median: {}",
                &[&spent, &done, &wait],
            ),
        };
        let disk = tf("Worktrees: {}", &[&words::bytes(fleet.worktree_bytes())]);
        FleetWords {
            orders,
            quotas,
            day,
            disk,
        }
    }

    /// Running, waiting, failed and how many may run at once.
    pub fn orders(&self) -> &str {
        &self.orders
    }

    /// Use per account and service against its window, with when it was seen (C65).
    pub fn quotas(&self) -> &[String] {
        &self.quotas
    }

    /// The day's spending against its budget, orders done today and the median wait.
    pub fn day(&self) -> &str {
        &self.day
    }

    /// The space the worktrees take (R68).
    pub fn disk(&self) -> &str {
        &self.disk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_folder_shows_every_station_with_none() {
        let view = BoardView::default();
        let strip = view.strip();
        assert_eq!(strip.len(), Station::ALL.len());
        assert!(strip.iter().all(|stop| stop.count() == 0 && !stop.active()));
        assert!(view.rows().is_empty() && view.is_empty());
    }

    #[test]
    fn a_second_click_on_the_station_clears_the_filter() {
        let mut view = BoardView::default();
        view.click(Station::Build);
        assert_eq!(view.filter(), Some(Station::Build));
        view.click(Station::Build);
        assert_eq!(view.filter(), None);
    }

    #[test]
    fn every_column_has_its_key() {
        for (column, key) in COLUMNS {
            assert_eq!(column_of(key), Some(column));
            assert_eq!(key_of(column), key);
        }
    }
}
