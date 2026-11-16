//! The Leitstand tab (E175, the Kanban board struck by the owner): the fleet line, the station strip with a count per
//! station that filters the table, and the sortable table whose filter and sort survive each new board.

use ide_factory::OrderId;
use ide_factory::budget::Spend;
use ide_factory::panel::{Board, Column, Fleet, Row};
use ide_factory::station::Station;
use ide_factory::workflow::rules::Loosening;
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
                        &words::status(row.station(), row.condition(), row.held()),
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

    /// Whether any order of the folder waits for the user, whatever the filter shows: the tab's red dot (E71).
    pub fn waits(&self) -> bool {
        self.board.as_ref().is_some_and(|board| {
            let mut all = board.clone();
            all.filter(None);
            all.rows()
                .iter()
                .any(|row| Tone::of(row.station(), row.condition()) == Tone::Waiting)
        })
    }

    /// Whether the folder has no order at all.
    pub fn is_empty(&self) -> bool {
        self.board
            .as_ref()
            .is_none_or(|board| board.strip().iter().all(|(_, n)| *n == 0))
    }

    /// What the table says when it has no rows.
    pub fn empty_words(&self) -> String {
        if self.is_empty() {
            words::no_order_yet()
        } else {
            t("Kein Auftrag an dieser Station.").to_string()
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
    /// The words of `fleet` at `now` (wall clock) on the local day `today`, such as `2026-11-16`; a folder without a
    /// factory shows zeros.
    pub fn new(fleet: Option<&Fleet>, now: u64, today: &str) -> FleetWords {
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
        // The account starts over only when spending comes in, so after midnight it may still hold yesterday's.
        let spent = if fleet.day() == today {
            words::spend(fleet.day_spent())
        } else {
            words::spend(&Spend::default())
        };
        let done = fleet.done_today().to_string();
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

/// The loosening among `loosenings` whose hash is `hash`, so a button confirms the loosening it stood beside even
/// when the list changed since it was shown.
pub fn loosening_of<'a>(loosenings: &'a [Loosening], hash: &str) -> Option<&'a Loosening> {
    loosenings.iter().find(|loosening| loosening.hash() == hash)
}

/// What VoiceOver reads for the button that confirms `loosening`.
pub fn confirm_label(loosening: &Loosening) -> String {
    tf("Bestätigen: {}", &[&words::loosening(loosening)])
}

#[cfg(test)]
mod tests {
    use ide_factory::budget::DailyAccount;
    use ide_factory::quota::QuotaBook;
    use ide_factory::workflow::rules::Rules;
    use ide_factory::{Factory, Input, InputKind, Stage, Stamp};

    use super::super::testing::waiting;
    use super::*;

    #[test]
    fn the_red_dot_stays_while_the_filter_hides_the_order_that_waits() {
        let factory = waiting();
        let mut view = BoardView::default();
        view.set(Some(Board::new(&factory, factory.now())), factory.now().wall);
        assert!(view.waits(), "the order waits: {:?}", view.rows());
        view.click(Station::Done);
        assert!(view.rows().is_empty());
        assert!(view.waits(), "the filter hides the order, not its wait");
    }

    #[test]
    fn yesterdays_spending_does_not_count_for_today() {
        let mut factory = Factory::new(Stage::One, Rules::default());
        let mut account = DailyAccount::new("2026-11-15");
        account.add("2026-11-15", None, &Spend::new(3.5, 0, 0.0));
        factory.handle(Input::new(Stamp::new(1, 1), InputKind::Day(account)));
        let fleet = Fleet::new(&factory, &QuotaBook::default(), Stamp::new(2, 2), 0, 0);
        let yesterday = FleetWords::new(Some(&fleet), 2, "2026-11-15");
        let today = FleetWords::new(Some(&fleet), 2, "2026-11-16");
        assert!(yesterday.day().contains(&words::dollars(3.5)), "{}", yesterday.day());
        assert!(!today.day().contains(&words::dollars(3.5)), "{}", today.day());
        assert!(
            today.day().contains(&words::spend(&Spend::default())),
            "{}",
            today.day()
        );
    }

    #[test]
    fn a_loosening_is_found_by_its_hash_wherever_it_stands() {
        let skip = Loosening::SkipSpecApproval;
        let copy = Loosening::CopyCredentials {
            paths: vec![".env".into()],
        };
        let shown = [skip.clone(), copy.clone()];
        let now = [copy.clone()];
        assert_eq!(loosening_of(&now, &shown[1].hash()), Some(&copy));
        assert_eq!(loosening_of(&now, &shown[0].hash()), None);
        assert_ne!(confirm_label(&skip), confirm_label(&copy));
    }

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
