//! The builder's word on the criteria (E178, the "sprint contract" after Anthropic's harness design): before it builds,
//! the builder confirms the criteria or objects to them in a fixed form, and an objection makes the order wait for the
//! user, who changes the criteria, overrules the objection or aborts.

use serde::{Deserialize, Serialize};

/// The line that confirms the criteria.
pub const CONFIRMED: &str = "CRITERIA CONFIRMED";

/// The line that begins an objection; the reasons follow it.
pub const OBJECTION: &str = "CRITERIA OBJECTION";

/// What the builder's prompt asks while the criteria wait for its word.
pub fn instructions() -> String {
    format!(
        "Before you change anything, check the acceptance criteria. If you can meet every one of them, carry out the \
         order and end your final message with the line `{CONFIRMED}`. If you object to a criterion, because it \
         contradicts another, cannot be checked or asks for more than the goal, change nothing: answer with the line \
         `{OBJECTION}`, then one line per objection such as `- [K2] why`, and end your turn."
    )
}

/// What the builder said about the criteria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reply", content = "text")]
pub enum CriteriaReply {
    /// It confirmed them.
    Confirmed,
    /// It objected, with its reasons.
    Objection(String),
    /// It said neither; the order goes on as if it had confirmed, and the log says so.
    Missing,
}

/// The builder's word in `answer`, its last message: the last line that confirms or objects counts, an objection with
/// everything after it.
pub fn read_reply(answer: Option<&str>) -> CriteriaReply {
    let Some(answer) = answer else {
        return CriteriaReply::Missing;
    };
    let lines: Vec<&str> = answer.lines().collect();
    for (index, line) in lines.iter().enumerate().rev() {
        let bare = line
            .trim()
            .trim_matches(|c: char| matches!(c, '`' | '*' | '_' | '#' | '>'))
            .trim();
        let upper = bare.to_ascii_uppercase();
        if let Some(rest) = upper.strip_prefix(OBJECTION) {
            let first = bare[bare.len() - rest.len()..]
                .trim_start_matches([':', '-', ' '])
                .trim_matches(|c: char| matches!(c, '`' | '*' | '_'))
                .trim();
            let mut text = vec![first];
            text.extend(lines[index + 1..].iter().map(|l| l.trim_end()));
            return CriteriaReply::Objection(text.join("\n").trim().to_string());
        }
        if upper.trim_end_matches(['.', ':']) == CONFIRMED {
            return CriteriaReply::Confirmed;
        }
    }
    CriteriaReply::Missing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_marker_line_counts() {
        assert_eq!(
            read_reply(Some("Done.\n\nCRITERIA CONFIRMED")),
            CriteriaReply::Confirmed
        );
        assert_eq!(
            read_reply(Some("All good.\n**Criteria confirmed.**\n")),
            CriteriaReply::Confirmed
        );
        assert_eq!(read_reply(Some("I built it.")), CriteriaReply::Missing);
        assert_eq!(read_reply(None), CriteriaReply::Missing);
        assert_eq!(
            read_reply(Some(
                "CRITERIA CONFIRMED\nOn second thought:\n`CRITERIA OBJECTION`\n- [K2] untestable\n"
            )),
            CriteriaReply::Objection("- [K2] untestable".into())
        );
    }

    #[test]
    fn an_objection_keeps_its_reasons_and_text_on_its_line() {
        assert_eq!(
            read_reply(Some("Criteria objection: K1 contradicts K3\n- [K2] needs a network\n")),
            CriteriaReply::Objection("K1 contradicts K3\n- [K2] needs a network".into())
        );
        assert_eq!(
            read_reply(Some("CRITERIA OBJECTION")),
            CriteriaReply::Objection(String::new())
        );
    }

    #[test]
    fn the_instructions_name_both_lines() {
        let text = instructions();
        assert!(text.contains(CONFIRMED) && text.contains(OBJECTION));
    }
}
