//! Acceptance criteria with stable ids (E179, C63): the reviewer judges each id exactly once, and a changed list keeps
//! the ids of criteria whose text stayed.

use serde::{Deserialize, Serialize};

/// One acceptance criterion.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Criterion {
    id: String,
    text: String,
}

impl Criterion {
    /// A criterion with `id` and `text`.
    pub fn new(id: &str, text: &str) -> Criterion {
        Criterion {
            id: id.to_string(),
            text: text.to_string(),
        }
    }

    /// Its stable id, such as `K3`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What must hold.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// The criteria of `text`, one per non-empty line, with list markers removed and ids `K1`, `K2`, ….
pub fn criteria_from_lines(text: &str) -> Vec<Criterion> {
    renumber(&[], lines(text))
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| {
            let line = line.trim();
            let line = line
                .strip_prefix("- [ ]")
                .or_else(|| line.strip_prefix("- "))
                .or_else(|| line.strip_prefix("* "))
                .unwrap_or(line);
            line.trim().to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

/// `texts` as criteria, keeping the id of each text `old` already had and giving new texts ids after the highest.
pub fn renumber(old: &[Criterion], texts: Vec<String>) -> Vec<Criterion> {
    let mut next = old
        .iter()
        .filter_map(|c| c.id.strip_prefix('K').and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);
    let mut used: Vec<&str> = Vec::new();
    texts
        .into_iter()
        .map(|text| {
            let kept = old.iter().find(|c| c.text == text && !used.contains(&c.id.as_str()));
            match kept {
                Some(c) => {
                    used.push(&c.id);
                    c.clone()
                }
                None => {
                    next += 1;
                    Criterion {
                        id: format!("K{next}"),
                        text,
                    }
                }
            }
        })
        .collect()
}

/// `text`'s lines as criteria over `old`, for "Kriterien ändern" (E184).
pub fn changed_criteria(old: &[Criterion], text: &str) -> Vec<Criterion> {
    renumber(old, lines(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_become_numbered_criteria() {
        let got = criteria_from_lines("- exports CSV\n\n* keeps the header \n- [ ] handles UTF-8\n");
        let ids: Vec<(&str, &str)> = got.iter().map(|c| (c.id(), c.text())).collect();
        assert_eq!(
            ids,
            [("K1", "exports CSV"), ("K2", "keeps the header"), ("K3", "handles UTF-8")]
        );
    }

    #[test]
    fn a_changed_list_keeps_the_ids_of_unchanged_texts() {
        let old = criteria_from_lines("a\nb\nc");
        let new = changed_criteria(&old, "c\nnew\na");
        let ids: Vec<&str> = new.iter().map(Criterion::id).collect();
        assert_eq!(ids, ["K3", "K4", "K1"]);
    }
}
