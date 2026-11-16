//! Orders from issues (E176, R63): what an order keeps of the issue it came from, the issue's open task list as
//! suggested criteria, the sheet's prefill, and which issues automatic intake takes. An issue's text is foreign text:
//! it is frozen with its hash when the order comes in, goes into prompts marked as untrusted, and such an order never
//! skips G1. Comments are never taken.

use serde::{Deserialize, Serialize};

use crate::options::NewOrder;
use crate::workflow::IssueIntake;

/// The issue an order came from, as it read when the order came in (E176, E177).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueOrigin {
    /// The issue's page, which also tells it apart across repositories.
    pub url: String,
    /// The number the project shows.
    pub number: u64,
    /// Who opened it.
    pub author: Option<String>,
    /// Its description as it read when the order came in.
    pub body: String,
    /// The SHA-256 of `body`, to tell a later change of the issue.
    pub hash: String,
}

impl IssueOrigin {
    /// The origin of the issue `number` at `url` by `author`, its description `body` frozen with its hash.
    pub fn freeze(url: &str, number: u64, author: Option<&str>, body: &str) -> IssueOrigin {
        IssueOrigin {
            url: url.to_string(),
            number,
            author: author.map(str::to_string),
            body: body.to_string(),
            hash: crate::hash::sha256_hex(body.as_bytes()),
        }
    }

    /// Whether `body` is still the text the order froze.
    pub fn unchanged(&self, body: &str) -> bool {
        crate::hash::sha256_hex(body.as_bytes()) == self.hash
    }
}

/// The open items of `body`'s task lists (`- [ ] …`), which the sheet offers as suggestions from the issue; done items
/// and items inside code blocks are left out.
pub fn task_list(body: &str) -> Vec<String> {
    let mut fenced = false;
    let mut items = Vec::new();
    for line in body.lines() {
        let line = line.trim_start();
        if line.starts_with("```") || line.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let item = ["- [ ]", "* [ ]", "+ [ ]"]
            .iter()
            .find_map(|marker| line.strip_prefix(marker))
            .map(str::trim);
        if let Some(item) = item.filter(|i| !i.is_empty()) {
            items.push(item.to_string());
        }
    }
    items
}

/// The order an issue makes before the user changed anything: its title, its description as the goal, its task list
/// as the criteria, and where it came from.
pub fn issue_order(origin: IssueOrigin, title: &str) -> NewOrder {
    let criteria: Vec<String> = task_list(&origin.body)
        .iter()
        .map(|item| format!("- {item}\n"))
        .collect();
    NewOrder {
        title: title.trim().to_string(),
        goal: origin.body.trim().to_string(),
        criteria: criteria.concat(),
        issue: Some(origin),
        ..NewOrder::default()
    }
}

/// Whether automatic intake takes an issue with `labels` by `author`: it carries the intake's label and its author is
/// one of the allowed ones, compared without case; an issue without an author is never taken.
pub fn wanted(intake: &IssueIntake, author: Option<&str>, labels: &[String]) -> bool {
    let Some(author) = author else { return false };
    labels.iter().any(|l| l.eq_ignore_ascii_case(&intake.label))
        && intake.authors.iter().any(|a| a.eq_ignore_ascii_case(author))
}

/// The specification of an order from an issue in stage 1, where no planner writes one: its text for G1, which such
/// an order never skips (E176).
pub(crate) fn spec_of(
    title: &str,
    goal: &str,
    criteria: &[crate::criteria::Criterion],
    origin: &IssueOrigin,
) -> String {
    let items: Vec<String> = criteria.iter().map(|c| format!("- {}", c.text())).collect();
    format!(
        "# {title}\n\nFrom issue #{} ({}).\n\n## Goal\n\n{goal}\n\n## Criteria\n\n{}\n",
        origin.number,
        origin.url,
        items.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_open_task_list_items_outside_code_become_suggestions() {
        let body = "Intro\n\n- [ ] an empty file opens\n- [x] done already\n* [ ]  trims  \n- [ ]\n```\n- [ ] in code\n```\n  + [ ] nested\n";
        assert_eq!(task_list(body), ["an empty file opens", "trims", "nested"]);
    }

    #[test]
    fn an_issue_order_takes_title_body_and_tasks_and_keeps_its_origin_frozen() {
        let origin = IssueOrigin::freeze("https://x/issues/3", 3, Some("octo"), "Fix it.\n\n- [ ] it works\n");
        let order = issue_order(origin.clone(), " Crash ");
        assert_eq!(order.title, "Crash");
        assert_eq!(order.criteria, "- it works\n");
        assert!(order.goal.starts_with("Fix it."));
        assert!(origin.unchanged("Fix it.\n\n- [ ] it works\n") && !origin.unchanged("Fix it."));
        assert_eq!(order.issue, Some(origin));
    }

    #[test]
    fn intake_wants_the_label_and_an_allowed_author() {
        let intake = IssueIntake {
            label: "factory".into(),
            authors: vec!["Octo".into()],
        };
        let labels = vec!["bug".to_string(), "Factory".to_string()];
        assert!(wanted(&intake, Some("octo"), &labels));
        assert!(!wanted(&intake, Some("mallory"), &labels));
        assert!(!wanted(&intake, None, &labels));
        assert!(!wanted(&intake, Some("octo"), &["bug".to_string()]));
    }
}
