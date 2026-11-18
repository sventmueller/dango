//! The Leitstand tab's figures (E180): a sortable table of first-pass rate, review rounds, cost and runtime per role,
//! agent, model and risk class, and the lines that say how many orders they rest on, how long the user was waited for
//! and when the rate breaks a tie in routing. Every state is words, none a colour alone (E101).

use std::cmp::Ordering;

use ide_factory::metrics::{Figures, Key, MIN_ORDERS, Metrics};
use splat_i18n::{t, tf};

use super::list::agent;
use super::words;

/// A column of the figures' table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricsColumn {
    /// The role.
    Role,
    /// Agent and model.
    Agent,
    /// The risk class.
    Risk,
    /// The orders the key took part in.
    Orders,
    /// The first-pass rate and the orders it rests on.
    FirstPass,
    /// The review rounds that did not pass.
    Rounds,
    /// What the role spent.
    Spend,
    /// How long its turns ran.
    Runtime,
}

/// The columns in their order, with the key a column's identifier and sort descriptor carry.
pub const METRICS_COLUMNS: [(MetricsColumn, &str); 8] = [
    (MetricsColumn::Role, "metrics-role"),
    (MetricsColumn::Agent, "metrics-agent"),
    (MetricsColumn::Risk, "metrics-risk"),
    (MetricsColumn::Orders, "metrics-orders"),
    (MetricsColumn::FirstPass, "metrics-first-pass"),
    (MetricsColumn::Rounds, "metrics-rounds"),
    (MetricsColumn::Spend, "metrics-spend"),
    (MetricsColumn::Runtime, "metrics-runtime"),
];

/// The title of `column`'s header.
pub fn metrics_column_title(column: MetricsColumn) -> &'static str {
    match column {
        MetricsColumn::Role => t("Rolle"),
        MetricsColumn::Agent => t("Agent"),
        MetricsColumn::Risk => t("Risikoklasse"),
        MetricsColumn::Orders => t("Aufträge"),
        MetricsColumn::FirstPass => t("Erstdurchlauf"),
        MetricsColumn::Rounds => t("Prüfrunden"),
        MetricsColumn::Spend => t("Kosten"),
        MetricsColumn::Runtime => t("Laufzeit"),
    }
}

/// The column of `key`.
pub fn metrics_column_of(key: &str) -> Option<MetricsColumn> {
    METRICS_COLUMNS
        .iter()
        .find(|(_, k)| *k == key)
        .map(|(column, _)| *column)
}

/// The key of `column`.
pub fn metrics_key_of(column: MetricsColumn) -> &'static str {
    METRICS_COLUMNS
        .iter()
        .find(|(c, _)| *c == column)
        .map_or("metrics-role", |(_, key)| key)
}

/// One row of the table: the words per column and what it sorts by.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricsRow {
    key: Key,
    figures: Figures,
    cells: [String; 8],
}

impl MetricsRow {
    fn of(key: &Key, figures: &Figures) -> MetricsRow {
        let rate = match figures.first_pass_rate() {
            Some(rate) => tf(
                "{} % aus {}",
                &[&percent(rate).to_string(), &figures.eligible().to_string()],
            ),
            None => t("ohne Quote").to_string(),
        };
        MetricsRow {
            key: key.clone(),
            figures: figures.clone(),
            cells: [
                words::role(key.role()).to_string(),
                agent(Some(&key.agent().to_string()), key.model()),
                words::risk(key.risk()).to_string(),
                figures.orders().to_string(),
                rate,
                figures.rounds().to_string(),
                words::spend(figures.spend()),
                words::duration(figures.runtime_ms()),
            ],
        }
    }

    /// The text of `column`.
    pub fn cell(&self, column: MetricsColumn) -> &str {
        let index = METRICS_COLUMNS.iter().position(|(c, _)| *c == column).unwrap_or(0);
        &self.cells[index]
    }

    fn compare(&self, other: &MetricsRow, column: MetricsColumn) -> Ordering {
        let (a, b) = (&self.figures, &other.figures);
        let by = match column {
            MetricsColumn::Role => self.key.role().cmp(&other.key.role()),
            MetricsColumn::Agent => self.cell(column).cmp(other.cell(column)),
            MetricsColumn::Risk => self.key.risk().cmp(&other.key.risk()),
            MetricsColumn::Orders => a.orders().cmp(&b.orders()),
            MetricsColumn::FirstPass => a
                .first_pass_rate()
                .partial_cmp(&b.first_pass_rate())
                .unwrap_or(Ordering::Equal)
                .then(a.eligible().cmp(&b.eligible())),
            MetricsColumn::Rounds => a.rounds().cmp(&b.rounds()),
            MetricsColumn::Spend => a
                .spend()
                .dollars()
                .total_cmp(&b.spend().dollars())
                .then(a.spend().tokens().cmp(&b.spend().tokens())),
            MetricsColumn::Runtime => a.runtime_ms().cmp(&b.runtime_ms()),
        };
        by.then_with(|| self.key.cmp(&other.key))
    }
}

/// A share from 0 to 1 in whole percent.
fn percent(share: f64) -> u32 {
    let rounded = (share.clamp(0.0, 1.0) * 100.0).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=100"
    )]
    let percent = rounded as u32;
    percent
}

/// The figures' section of the tab: whether it is open, its lines, and its rows in the order sorted.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricsView {
    rows: Vec<MetricsRow>,
    lines: Vec<String>,
    sort: MetricsColumn,
    ascending: bool,
    open: bool,
}

impl Default for MetricsView {
    fn default() -> Self {
        MetricsView {
            rows: Vec::new(),
            lines: vec![t("Noch keine Kennzahlen.").to_string()],
            sort: MetricsColumn::Role,
            ascending: true,
            open: false,
        }
    }
}

impl MetricsView {
    /// Takes `metrics`, with the sort and whether the section is open kept; `None` while the folder has no factory.
    pub fn set(&mut self, metrics: Option<&Metrics>) {
        let Some(metrics) = metrics.filter(|m| m.orders() > 0) else {
            *self = MetricsView {
                sort: self.sort,
                ascending: self.ascending,
                open: self.open,
                ..MetricsView::default()
            };
            return;
        };
        self.rows = metrics.figures().iter().map(|(k, f)| MetricsRow::of(k, f)).collect();
        let wait = metrics
            .median_wait_ms()
            .map_or_else(|| t("keine").to_string(), words::duration);
        let eligible = metrics.eligible().to_string();
        let rests = match metrics.orders() {
            1 => tf(
                "Die Kennzahlen beruhen auf 1 Auftrag; {} davon zählen für die Erstdurchlaufquote, ohne Eingriffe und ohne Agentenwechsel.",
                &[&eligible],
            ),
            orders => tf(
                "Die Kennzahlen beruhen auf {} Aufträgen; {} davon zählen für die Erstdurchlaufquote, ohne Eingriffe und ohne Agentenwechsel.",
                &[&orders.to_string(), &eligible],
            ),
        };
        self.lines = vec![
            rests,
            tf("Wartezeit auf dich im Median: {}", &[&wait]),
            tf(
                "Bei Gleichstand im Routing entscheidet die Quote, sobald beide Einträge auf mindestens {} Aufträgen beruhen.",
                &[&MIN_ORDERS.to_string()],
            ),
        ];
        self.apply();
    }

    fn apply(&mut self) {
        let (column, ascending) = (self.sort, self.ascending);
        self.rows.sort_by(|a, b| {
            let order = a.compare(b, column);
            if ascending { order } else { order.reverse() }
        });
    }

    /// Sorts by `column` in `ascending` order, as the table's header asks.
    pub fn sort(&mut self, column: MetricsColumn, ascending: bool) {
        self.sort = column;
        self.ascending = ascending;
        self.apply();
    }

    /// The column and direction sorted by.
    pub fn sorting(&self) -> (MetricsColumn, bool) {
        (self.sort, self.ascending)
    }

    /// The rows in the order sorted.
    pub fn rows(&self) -> &[MetricsRow] {
        &self.rows
    }

    /// What the section says above its table.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Opens or closes the section.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Whether the section shows its table.
    pub fn open(&self) -> bool {
        self.open
    }

    /// What the disclosure says, for VoiceOver too: "Kennzahlen, 4 Zeilen".
    pub fn title(&self) -> String {
        match self.rows.len() {
            0 => t("Kennzahlen").to_string(),
            1 => t("Kennzahlen, 1 Zeile").to_string(),
            n => tf("Kennzahlen, {} Zeilen", &[&n.to_string()]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_orders_the_section_says_so_and_keeps_its_sort() {
        let mut view = MetricsView::default();
        view.sort(MetricsColumn::FirstPass, false);
        view.toggle();
        view.set(Some(&Metrics::default()));
        assert!(view.rows().is_empty());
        assert_eq!(view.lines(), [t("Noch keine Kennzahlen.")]);
        assert_eq!(view.sorting(), (MetricsColumn::FirstPass, false));
        assert!(view.open());
    }

    #[test]
    fn every_column_has_its_key() {
        for (column, key) in METRICS_COLUMNS {
            assert_eq!(metrics_column_of(key), Some(column));
            assert_eq!(metrics_key_of(column), key);
        }
    }

    #[test]
    fn shares_read_in_whole_percent() {
        assert_eq!((percent(0.0), percent(0.666), percent(1.0)), (0, 67, 100));
    }
}
