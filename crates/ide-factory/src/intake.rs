//! Orders from issues (E176, R63): what an order keeps of the issue it came from, the issue's open task list as
//! suggested criteria, the sheet's prefill, and which issues automatic intake takes. An issue's text is foreign text:
//! it is frozen with its hash when the order comes in, goes into prompts marked as untrusted, and such an order never
//! skips G1. Comments are never taken. A later change of the issue shows as a [`line_diff`] and changes the order only
//! when the user takes it over, its task list then updating the criteria ([`updated_criteria`]).

use serde::{Deserialize, Serialize};

use crate::criteria::Criterion;
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

/// The criteria of an order over `criteria` once its issue changed from `before` to `after` (E176): criteria that
/// were tasks the issue dropped go, tasks it gained come after the rest, and criteria the planner or the user wrote
/// stay as they are, their ids kept.
pub fn updated_criteria(criteria: &[Criterion], before: &str, after: &str) -> Vec<Criterion> {
    let (old, new) = (task_list(before), task_list(after));
    let mut texts: Vec<String> = criteria
        .iter()
        .map(|c| c.text().to_string())
        .filter(|text| new.contains(text) || !old.contains(text))
        .collect();
    for task in new {
        if !old.contains(&task) && !texts.contains(&task) {
            texts.push(task);
        }
    }
    crate::criteria::renumber(criteria, texts)
}

/// A line of the difference between two texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// In both.
    Kept(String),
    /// Only in the old text.
    Removed(String),
    /// Only in the new text.
    Added(String),
}

/// The lines of `before` and `after` with what was removed and added, by their longest common sequence; texts too long
/// for that come out as wholly removed and added.
pub fn line_diff(before: &str, after: &str) -> Vec<DiffLine> {
    const MOST: usize = 2_000;
    let (old, new): (Vec<&str>, Vec<&str>) = (before.lines().collect(), after.lines().collect());
    if old.len() > MOST || new.len() > MOST {
        let removed = old.iter().map(|l| DiffLine::Removed((*l).to_string()));
        return removed
            .chain(new.iter().map(|l| DiffLine::Added((*l).to_string())))
            .collect();
    }
    let mut common = vec![vec![0u32; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            common[i][j] = if old[i] == new[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            out.push(DiffLine::Kept(old[i].to_string()));
            (i, j) = (i + 1, j + 1);
        } else if i < old.len() && (j == new.len() || common[i + 1][j] >= common[i][j + 1]) {
            out.push(DiffLine::Removed(old[i].to_string()));
            i += 1;
        } else {
            out.push(DiffLine::Added(new[j].to_string()));
            j += 1;
        }
    }
    out
}

/// The specification of an order from an issue in stage 1, where no planner writes one: its text for G1, which such
/// an order never skips (E176).
pub(crate) fn spec_of(title: &str, goal: &str, criteria: &[Criterion], origin: &IssueOrigin) -> String {
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
    fn a_changed_task_list_updates_only_the_criteria_that_were_its_tasks() {
        let before = "Fix it.\n\n- [ ] it opens\n- [ ] it saves\n";
        let after = "Fix it now.\n\n- [ ] it opens\n- [ ] it exports\n";
        let criteria = crate::criteria::criteria_from_lines("it opens\nit saves\nno panic on empty files");
        let got = updated_criteria(&criteria, before, after);
        let ids: Vec<(&str, &str)> = got.iter().map(|c| (c.id(), c.text())).collect();
        assert_eq!(
            ids,
            [
                ("K1", "it opens"),
                ("K3", "no panic on empty files"),
                ("K4", "it exports")
            ]
        );
        assert_eq!(updated_criteria(&criteria, before, before), criteria);
    }

    #[test]
    fn the_diff_keeps_common_lines_and_marks_the_rest() {
        let got = line_diff("a\nb\nc", "a\nx\nc\nd");
        assert_eq!(
            got,
            [
                DiffLine::Kept("a".into()),
                DiffLine::Removed("b".into()),
                DiffLine::Added("x".into()),
                DiffLine::Kept("c".into()),
                DiffLine::Added("d".into()),
            ]
        );
        assert_eq!(line_diff("", "new"), [DiffLine::Added("new".into())]);
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
