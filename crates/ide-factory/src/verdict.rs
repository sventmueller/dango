//! The reviewer's structured verdict (E179, C63): per criterion met, not met or unclear with a reason, and findings
//! with file, line and severity. Every criterion must be judged exactly once; a missing one is unclear, a duplicate
//! makes the round invalid, and an answer that cannot be read makes the round unclear.

use serde::{Deserialize, Serialize};

use crate::criteria::Criterion;

/// The JSON schema of the verdict, for Codex's `outputSchema` and the prompts of the other paths (A19).
pub const VERDICT_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["criteria", "findings"],
  "properties": {
    "criteria": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "verdict", "reason"],
        "properties": {
          "id": { "type": "string" },
          "verdict": { "type": "string", "enum": ["met", "not_met", "unclear"] },
          "reason": { "type": "string" }
        }
      }
    },
    "findings": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["file", "line", "severity", "text"],
        "properties": {
          "file": { "type": "string" },
          "line": { "type": ["integer", "null"] },
          "severity": { "type": "string", "enum": ["low", "medium", "high"] },
          "text": { "type": "string" }
        }
      }
    }
  }
}"#;

/// How the reviewer judged a criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Judgement {
    /// Met.
    Met,
    /// Not met: back to the builder.
    NotMet,
    /// Unclear: to the user.
    Unclear,
}

/// The verdict on one criterion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionVerdict {
    /// The criterion's id.
    pub id: String,
    /// The judgement.
    pub verdict: Judgement,
    /// Why.
    pub reason: String,
}

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High: back to the builder whatever the criteria say.
    High,
}

/// A finding of the reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// The file.
    pub file: String,
    /// The line, when there is one.
    pub line: Option<u32>,
    /// How serious.
    pub severity: Severity,
    /// What is wrong.
    pub text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    criteria: Vec<CriterionVerdict>,
    #[serde(default)]
    findings: Vec<Finding>,
}

/// What a review round decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum RoundOutcome {
    /// Every criterion met, no finding of severity high.
    Passed,
    /// Back to the builder with these (E179's hard threshold).
    Rework {
        /// The criteria not met.
        not_met: Vec<String>,
    },
    /// To the user: these criteria are unclear, missing from the verdict, or the answer could not be read.
    Unclear {
        /// The unclear criteria; all of them when the answer could not be read.
        criteria: Vec<String>,
    },
    /// A criterion was judged twice; the round does not count as a verdict.
    Invalid {
        /// The criteria judged more than once.
        duplicates: Vec<String>,
    },
}

/// A review round: what it decided, and the verdicts and findings it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    /// What it decided.
    pub outcome: RoundOutcome,
    /// The verdict per criterion, in the order of the criteria; missing ones as unclear.
    pub verdicts: Vec<CriterionVerdict>,
    /// The findings.
    pub findings: Vec<Finding>,
}

/// The JSON in `answer`: the whole text, else its last fenced block, else from the first `{` to the last `}`.
fn json_in(answer: &str) -> Option<&str> {
    let trimmed = answer.trim();
    if trimmed.starts_with('{') {
        return Some(trimmed);
    }
    if let Some(start) = answer.rfind("```json").or_else(|| answer.rfind("```JSON")) {
        let body = &answer[start + 7..];
        if let Some(end) = body.find("```") {
            return Some(body[..end].trim());
        }
    }
    let (start, end) = (answer.find('{')?, answer.rfind('}')?);
    (start < end).then(|| &answer[start..=end])
}

/// Reads the reviewer's `answer` against `criteria`.
pub fn read_verdict(answer: &str, criteria: &[Criterion]) -> Round {
    let all = || criteria.iter().map(|c| c.id().to_string()).collect::<Vec<_>>();
    let Some(raw) = json_in(answer).and_then(|json| serde_json::from_str::<Raw>(json).ok()) else {
        return Round {
            outcome: RoundOutcome::Unclear { criteria: all() },
            verdicts: criteria.iter().map(|c| missing(c.id())).collect(),
            findings: Vec::new(),
        };
    };
    let mut duplicates = Vec::new();
    let mut verdicts = Vec::new();
    for criterion in criteria {
        let mut given = raw.criteria.iter().filter(|v| v.id == criterion.id());
        let first = given.next();
        if given.next().is_some() {
            duplicates.push(criterion.id().to_string());
        }
        verdicts.push(first.cloned().unwrap_or_else(|| missing(criterion.id())));
    }
    let with = |judgement| {
        verdicts
            .iter()
            .filter(|v| v.verdict == judgement)
            .map(|v| v.id.clone())
            .collect::<Vec<_>>()
    };
    let outcome = if !duplicates.is_empty() {
        RoundOutcome::Invalid { duplicates }
    } else if !with(Judgement::NotMet).is_empty() || raw.findings.iter().any(|f| f.severity == Severity::High) {
        RoundOutcome::Rework {
            not_met: with(Judgement::NotMet),
        }
    } else if !with(Judgement::Unclear).is_empty() {
        RoundOutcome::Unclear {
            criteria: with(Judgement::Unclear),
        }
    } else {
        RoundOutcome::Passed
    };
    Round {
        outcome,
        verdicts,
        findings: raw.findings,
    }
}

fn missing(id: &str) -> CriterionVerdict {
    CriterionVerdict {
        id: id.to_string(),
        verdict: Judgement::Unclear,
        reason: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn criteria() -> Vec<Criterion> {
        vec![
            Criterion::new("K1", "exports CSV"),
            Criterion::new("K2", "keeps the header"),
        ]
    }

    fn answer(criteria: &[(&str, &str)], findings: &str) -> String {
        let items: Vec<String> = criteria
            .iter()
            .map(|(id, v)| format!(r#"{{"id":"{id}","verdict":"{v}","reason":"r"}}"#))
            .collect();
        format!(r#"{{"criteria":[{}],"findings":[{findings}]}}"#, items.join(","))
    }

    #[test]
    fn the_schema_is_json() {
        let schema: serde_json::Value = serde_json::from_str(VERDICT_SCHEMA).unwrap();
        assert_eq!(schema["required"][0], "criteria");
    }

    #[test]
    fn verdict_table() {
        let high = r#"{"file":"src/a.rs","line":3,"severity":"high","text":"panics"}"#;
        let low = r#"{"file":"src/a.rs","line":null,"severity":"low","text":"naming"}"#;
        let cases = [
            (answer(&[("K1", "met"), ("K2", "met")], ""), "passed"),
            (answer(&[("K1", "met"), ("K2", "met")], low), "passed"),
            (answer(&[("K1", "met"), ("K2", "met")], high), "rework"),
            (answer(&[("K1", "not_met"), ("K2", "unclear")], ""), "rework"),
            (answer(&[("K1", "met"), ("K2", "unclear")], ""), "unclear"),
            (answer(&[("K1", "met")], ""), "unclear"),
            (answer(&[("K1", "met"), ("K1", "met"), ("K2", "met")], ""), "invalid"),
            ("I think it is fine.".to_string(), "unclear"),
            (
                r#"{"criteria":[{"id":"K1","verdict":"great","reason":""}]}"#.to_string(),
                "unclear",
            ),
            (
                format!(
                    "Here it is:\n```json\n{}\n```\n",
                    answer(&[("K1", "met"), ("K2", "met")], "")
                ),
                "passed",
            ),
        ];
        for (text, outcome) in cases {
            let round = read_verdict(&text, &criteria());
            let got = serde_json::to_value(&round.outcome).unwrap()["outcome"].clone();
            assert_eq!(got, outcome, "{text}");
            assert_eq!(round.verdicts.len(), 2);
        }
    }

    #[test]
    fn a_missing_criterion_is_named_unclear() {
        let round = read_verdict(&answer(&[("K2", "met")], ""), &criteria());
        assert_eq!(
            round.outcome,
            RoundOutcome::Unclear {
                criteria: vec!["K1".into()]
            }
        );
        assert_eq!(round.verdicts[0].verdict, Judgement::Unclear);
    }
}
