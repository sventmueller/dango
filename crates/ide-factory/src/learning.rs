//! The learning loop of B47: findings that come back across a folder's orders, from which a scout proposes a change
//! to `AGENTS.md` or `CLAUDE.md` once an order ended escalated or failed. The proposal is an order of its own: the
//! scout's specification waits at G1, and nothing of it reaches the working copy without G2.

use std::collections::BTreeMap;

use crate::gates::GateStep;
use crate::metrics::MIN_ORDERS;
use crate::options::NewOrder;
use crate::order::Order;
use crate::prompt::untrusted;
use crate::station::{Condition, Station, WaitReason};

/// The paths a rule proposal may change: the agents' instructions (E172).
pub const PROPOSAL_SCOPE: [&str; 4] = ["AGENTS.md", "CLAUDE.md", "**/AGENTS.md", "**/CLAUDE.md"];

/// How many recurring findings a proposal lists at most.
const LISTED: usize = 10;

/// How many words of a finding's text tell findings apart.
const KEY_WORDS: usize = 16;

/// A finding that came back in more than one order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recurring {
    text: String,
    orders: u32,
}

impl Recurring {
    /// The finding as it was first seen: the reviewer's file and words, or the gate that failed.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// In how many orders it came back.
    pub fn orders(&self) -> u32 {
        self.orders
    }
}

/// Whether `order` ended escalated or failed: a station failed, the review rounds ran out, or the reviewer could not
/// decide (E168, E179).
pub fn escalated(order: &Order) -> bool {
    matches!(
        order.condition(),
        Condition::Failed(_) | Condition::Waiting(WaitReason::ReviewRoundsSpent | WaitReason::Unclear)
    )
}

/// The findings of `orders` that came back in more than one of them, most frequent first: the reviewer's findings by
/// their words, and gate commands that failed by their name and the kind of failure.
pub fn recurring<'a>(orders: impl IntoIterator<Item = &'a Order>) -> Vec<Recurring> {
    let mut seen: BTreeMap<String, Recurring> = BTreeMap::new();
    for order in orders {
        for (key, text) in findings_of(order) {
            seen.entry(key).or_insert(Recurring { text, orders: 0 }).orders += 1;
        }
    }
    let mut out: Vec<Recurring> = seen.into_values().filter(|r| r.orders > 1).collect();
    out.sort_by(|a, b| b.orders.cmp(&a.orders).then_with(|| a.text.cmp(&b.text)));
    out
}

/// What `order` found, once per kind: key and the text first seen.
fn findings_of(order: &Order) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    for finding in order.review_log().iter().flat_map(|round| &round.findings) {
        let key = words(&finding.text);
        if !key.is_empty() {
            let text = format!("{}: {}", finding.file, finding.text.trim());
            found.entry(format!("finding:{key}")).or_insert(text);
        }
    }
    let records = order
        .gates()
        .iter()
        .chain(order.evidence().into_iter().flat_map(|e| e.gates()));
    for record in records {
        let Some(failure) = &record.failure else { continue };
        if !matches!(record.step, GateStep::Command(_)) {
            continue;
        }
        let kind = serde_json::to_value(failure)
            .ok()
            .and_then(|v| v["failure"].as_str().map(str::to_string))
            .unwrap_or_default();
        let text = format!("the gate command \"{}\" failed: {kind}", record.name);
        found.entry(format!("gate:{}:{kind}", record.name)).or_insert(text);
    }
    found
}

/// The first words of `text` in lower case, without punctuation, so findings told in the same words match.
fn words(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(KEY_WORDS)
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The recurring findings a scout should turn into a rule proposal now that `ended` ended escalated or failed, if
/// it is time for one: the folder's `orders` hold at least [`MIN_ORDERS`] that reached the review, none of them is a
/// rule proposal still open, and some finding came back.
pub fn proposal_due<'a>(orders: impl IntoIterator<Item = &'a Order>, ended: &Order) -> Option<Vec<Recurring>> {
    if !escalated(ended) || ended.rule_proposal() {
        return None;
    }
    let orders: Vec<&Order> = orders.into_iter().collect();
    if orders.iter().any(|o| o.rule_proposal() && !o.finished()) {
        return None;
    }
    let reviewed: Vec<&Order> = orders
        .into_iter()
        .filter(|o| !o.rule_proposal() && o.reached() >= Station::Review)
        .collect();
    if u32::try_from(reviewed.len()).unwrap_or(u32::MAX) < MIN_ORDERS {
        return None;
    }
    let found = recurring(reviewed);
    (!found.is_empty()).then_some(found)
}

/// The order a scout's rule proposal comes as: `title` and the user's `intro` as given, the recurring findings
/// marked as untrusted text below it, confined to the agents' instructions; its criteria come from the scout's
/// specification.
pub fn proposal_order(title: &str, intro: &str, found: &[Recurring]) -> NewOrder {
    let list: Vec<String> = found
        .iter()
        .take(LISTED)
        .map(|r| format!("- in {} orders: {}", r.orders, r.text))
        .collect();
    let findings = untrusted("recurring findings of earlier orders", &list.join("\n"));
    NewOrder {
        title: title.to_string(),
        goal: format!("{intro}\n\n{findings}"),
        scope: PROPOSAL_SCOPE.iter().map(|p| p.to_string()).collect(),
        rule_proposal: true,
        ..NewOrder::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn findings_told_in_the_same_words_match() {
        assert_eq!(
            words("Missing error handling, in `parse()`!"),
            "missing error handling in parse"
        );
        assert_eq!(words("  "), "");
    }

    #[test]
    fn a_proposal_order_is_confined_to_the_instructions_and_marks_the_findings() {
        let found = [Recurring {
            text: "src/a.rs: unwrap on user input".into(),
            orders: 3,
        }];
        let order = proposal_order("Regelvorschlag: 1", "Propose a rule.", &found);
        assert!(order.rule_proposal && order.criteria.is_empty());
        assert_eq!(order.scope, PROPOSAL_SCOPE);
        assert!(order.goal.starts_with("Propose a rule."));
        assert!(order.goal.contains("in 3 orders: src/a.rs: unwrap on user input"));
        assert!(order.goal.contains("untrusted"));
    }
}
