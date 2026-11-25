//! The panel "Leitstand" as lines (E175): the folder's orders grouped by station, each with title, station, agent and
//! model and runtime, a red dot while it waits for the user; and the words of the panel's symbol (E69).

use std::hash::{DefaultHasher, Hash, Hasher};

use ide_factory::OrderId;
use ide_factory::panel::{Panel, Row};
use ide_factory::station::Station;
use splat_i18n::{t, tf};

use super::words::{self, Tone};

/// One order's line in the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLine {
    id: OrderId,
    title: String,
    tone: Tone,
    agent: String,
    /// How long it ran, which changes with every tick of a running order.
    runtime: String,
    /// What VoiceOver reads before the detail: title, station and condition.
    said: String,
}

impl OrderLine {
    fn of(row: &Row) -> OrderLine {
        let tone = Tone::of(row.station(), row.condition());
        let agent = agent(row.agent(), row.model());
        let condition = words::status(row.station(), row.condition(), row.held());
        let runtime = words::duration(row.runtime_ms());
        let said = [words::station(row.station()), condition.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .fold(row.title().to_string(), |spoken, part| tf("{}, {}", &[&spoken, part]));
        OrderLine {
            id: row.id().clone(),
            title: row.title().to_string(),
            tone,
            agent,
            runtime,
            said,
        }
    }

    /// The order.
    pub fn id(&self) -> &OrderId {
        &self.id
    }

    /// Its title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Its condition's shape and word.
    pub fn tone(&self) -> Tone {
        self.tone
    }

    /// Agent and model, then runtime.
    pub fn detail(&self) -> String {
        [self.agent.as_str(), self.runtime.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// What VoiceOver reads for the line: title, station, condition with its reason, agent and runtime.
    pub fn spoken(&self) -> String {
        let detail = self.detail();
        if detail.is_empty() {
            self.said.clone()
        } else {
            tf("{}, {}", &[&self.said, &detail])
        }
    }

    /// Whether it waits for the user, which the red dot shows (E71).
    pub fn waits(&self) -> bool {
        self.tone == Tone::Waiting
    }
}

/// "claude · opus", "claude", or nothing before any agent worked.
pub fn agent(agent: Option<&str>, model: Option<&str>) -> String {
    match (agent, model) {
        (Some(agent), Some(model)) => format!("{agent} · {model}"),
        (Some(agent), None) => agent.to_string(),
        (None, _) => String::new(),
    }
}

/// A line of the panel's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// The heading of a station with how many orders stand there.
    Station(Station, usize),
    /// An order.
    Order(OrderLine),
}

impl Line {
    /// What tells the line apart from the others, so the list keeps it open and selected.
    pub fn key(&self) -> Key {
        match self {
            Line::Station(station, _) => Key::Station(*station),
            Line::Order(order) => Key::Order(order.id.clone()),
        }
    }

    /// Whether it tells the same as `other` but maybe for an order's runtime.
    pub fn same_but_runtime(&self, other: &Line) -> bool {
        match (self, other) {
            (Line::Order(a), Line::Order(b)) => {
                a.id == b.id && a.title == b.title && a.tone == b.tone && a.agent == b.agent && a.said == b.said
            }
            _ => self == other,
        }
    }

    /// A hash of what its row shows but for an order's runtime, which the row takes in place.
    pub fn look(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        match self {
            Line::Station(station, count) => (station, count).hash(&mut hasher),
            Line::Order(order) => (&order.id, &order.title, order.tone, &order.agent, &order.said).hash(&mut hasher),
        }
        hasher.finish()
    }
}

/// What a line stands for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// A station's heading.
    Station(Station),
    /// An order.
    Order(OrderId),
}

/// The panel's list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct List {
    lines: Vec<Line>,
    waiting: usize,
}

impl List {
    /// The list of `panel`: a heading per station that has orders, its orders below it.
    pub fn new(panel: &Panel) -> List {
        let mut lines = Vec::new();
        for (station, rows) in panel.groups() {
            lines.push(Line::Station(*station, rows.len()));
            lines.extend(rows.iter().map(|row| Line::Order(OrderLine::of(row))));
        }
        List {
            lines,
            waiting: panel.waiting(),
        }
    }

    /// Whether it lists the same as `other` but maybe for the orders' runtimes.
    pub fn same_but_runtimes(&self, other: &List) -> bool {
        self.waiting == other.waiting
            && self.lines.len() == other.lines.len()
            && self.lines.iter().zip(&other.lines).all(|(a, b)| a.same_but_runtime(b))
    }

    /// Whether it has the same lines in the same order as `other`, whatever they say.
    pub fn same_keys(&self, other: &List) -> bool {
        self.lines.len() == other.lines.len() && self.lines.iter().zip(&other.lines).all(|(a, b)| a.key() == b.key())
    }

    /// Every line, each heading before its orders.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The headings.
    pub fn stations(&self) -> Vec<Station> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                Line::Station(station, _) => Some(*station),
                Line::Order(_) => None,
            })
            .collect()
    }

    /// The orders below the heading of `station`.
    pub fn orders_in(&self, station: Station) -> Vec<&OrderLine> {
        let mut inside = false;
        let mut orders = Vec::new();
        for line in &self.lines {
            match line {
                Line::Station(s, _) => inside = *s == station,
                Line::Order(order) if inside => orders.push(order),
                Line::Order(_) => {}
            }
        }
        orders
    }

    /// The line of the order `id`.
    pub fn order(&self, id: &OrderId) -> Option<&OrderLine> {
        self.lines.iter().find_map(|line| match line {
            Line::Order(order) if order.id == *id => Some(order),
            _ => None,
        })
    }

    /// Whether the folder has no order that is still open.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// How many orders wait for the user.
    pub fn waiting(&self) -> usize {
        self.waiting
    }

    /// What the panel's symbol says beside its name: "1 Auftrag wartet"; none while no order waits (E69).
    pub fn symbol_words(&self) -> Option<String> {
        match self.waiting {
            0 => None,
            1 => Some(t("1 Auftrag wartet").into()),
            n => Some(tf("{} Aufträge warten", &[&n.to_string()])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_symbol_names_waiting_orders_only() {
        let list = List::default();
        assert_eq!(list.symbol_words(), None);
        let one = List {
            waiting: 1,
            ..List::default()
        };
        assert_eq!(one.symbol_words().as_deref(), Some(t("1 Auftrag wartet")));
    }

    #[test]
    fn a_line_without_an_agent_reads_without_an_empty_part() {
        let factory = super::super::testing::waiting();
        let list = List::new(&Panel::new(&factory, factory.now()));
        let line = list.order(&OrderId("o1".into())).expect("the order is listed");
        assert!(line.spoken().starts_with("CSV export"), "{}", line.spoken());
        assert!(
            !line.spoken().ends_with(", ") && !line.spoken().contains(", , "),
            "{}",
            line.spoken()
        );
    }

    #[test]
    fn a_runtime_alone_leaves_the_look_and_the_keys() {
        let factory = super::super::testing::waiting();
        let list = List::new(&Panel::new(&factory, factory.now()));
        let mut later = list.clone();
        for line in &mut later.lines {
            if let Line::Order(order) = line {
                order.runtime = "9 min".into();
            }
        }
        assert_ne!(list, later);
        assert!(list.same_but_runtimes(&later));
        assert!(list.same_keys(&later));
        assert!(
            list.lines()
                .iter()
                .zip(later.lines())
                .all(|(a, b)| a.look() == b.look())
        );
        let Some(Line::Order(order)) = later.lines.iter().find(|l| matches!(l, Line::Order(_))) else {
            panic!("the list has an order");
        };
        assert!(order.spoken().ends_with("9 min"), "{}", order.spoken());
    }

    #[test]
    fn agent_and_model_join_as_one() {
        assert_eq!(agent(Some("claude"), Some("opus")), "claude · opus");
        assert_eq!(agent(Some("codex"), None), "codex");
        assert_eq!(agent(None, Some("x")), "");
    }
}
