//! When the Leitstand tells the user about an order (E175): once it begins to wait for the user, fails, or is ready
//! for acceptance; not again while it stays so, and not for what it already was when the folder's list first showed.

use std::collections::BTreeMap;

use ide_factory::OrderId;
use ide_factory::panel::Panel;
use ide_factory::station::{Condition, WaitReason};
use splat_i18n::t;

use super::words;

/// What an order was last seen as, by the kinds that notify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seen {
    Quiet,
    Waits,
    Failed,
    Acceptance,
}

/// One notification, for the host to post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    order: OrderId,
    title: String,
    body: String,
}

impl Notice {
    /// The order.
    pub fn order(&self) -> &OrderId {
        &self.order
    }

    /// The order's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// What happened.
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// What the orders of one folder were seen as last.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notices {
    seen: BTreeMap<OrderId, Seen>,
    started: bool,
}

impl Notices {
    /// The notifications `panel` calls for since it was seen last; the first look only takes in what is.
    pub fn update(&mut self, panel: &Panel) -> Vec<Notice> {
        let mut notices = Vec::new();
        let mut seen = BTreeMap::new();
        for row in panel.groups().iter().flat_map(|(_, rows)| rows) {
            let now = match row.condition() {
                Condition::Waiting(WaitReason::Acceptance) => Seen::Acceptance,
                Condition::Waiting(_) => Seen::Waits,
                Condition::Failed(_) => Seen::Failed,
                _ => Seen::Quiet,
            };
            let before = self.seen.get(row.id()).copied().unwrap_or(Seen::Quiet);
            if self.started && now != Seen::Quiet && now != before {
                let body = match now {
                    Seen::Acceptance => t("Bereit zur Abnahme").to_string(),
                    _ => words::condition(row.station(), row.condition()),
                };
                notices.push(Notice {
                    order: row.id().clone(),
                    title: row.title().to_string(),
                    body,
                });
            }
            seen.insert(row.id().clone(), now);
        }
        self.seen = seen;
        self.started = true;
        notices
    }
}
