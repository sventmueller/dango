//! Reading the planner's specification (E178): Markdown with the sections Criteria, Path scope and Risk.

use crate::station::RiskClass;

/// What the factory takes from a specification.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecParts {
    /// The criteria, one per list item of the section "Criteria".
    pub criteria: Vec<String>,
    /// The path scope, one pattern per list item of the section "Path scope".
    pub scope: Vec<String>,
    /// The proposed risk class from the section "Risk".
    pub risk: Option<RiskClass>,
}

/// The parts of `markdown` the factory uses; sections are told by their heading, whatever its level.
pub fn read_spec(markdown: &str) -> SpecParts {
    let mut parts = SpecParts::default();
    let mut section = String::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix('#') {
            section = heading.trim_start_matches('#').trim().to_ascii_lowercase();
            continue;
        }
        let item = trimmed
            .strip_prefix("- [ ]")
            .or_else(|| trimmed.strip_prefix("- "))
            .or_else(|| trimmed.strip_prefix("* "))
            .map(|s| s.trim().trim_matches('`').to_string());
        match section.as_str() {
            s if s.starts_with("criteria") || s.starts_with("acceptance") => {
                if let Some(item) = item.filter(|i| !i.is_empty()) {
                    parts.criteria.push(item);
                }
            }
            s if s.starts_with("path scope") || s == "scope" => {
                if let Some(item) = item.filter(|i| !i.is_empty()) {
                    parts.scope.push(item);
                }
            }
            s if s.starts_with("risk") && parts.risk.is_none() => {
                let lower = trimmed.to_ascii_lowercase();
                parts.risk = ["high", "medium", "low"]
                    .into_iter()
                    .find(|word| lower.split(|c: char| !c.is_ascii_alphabetic()).any(|w| w == *word))
                    .and_then(RiskClass::from_key);
            }
            _ => {}
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn criteria_scope_and_risk_come_from_their_sections() {
        let spec = "# CSV export\n\n## Goal\nExport.\n\n## Criteria\n- exports CSV\n- keeps the header\n\n\
                    ## Test plan\n- unit tests\n\n## Path scope\n- `src/export/**`\n\n## Risk\nMedium, it touches IO.\n";
        let parts = read_spec(spec);
        assert_eq!(parts.criteria, ["exports CSV", "keeps the header"]);
        assert_eq!(parts.scope, ["src/export/**"]);
        assert_eq!(parts.risk, Some(RiskClass::Medium));
        assert_eq!(read_spec("no sections"), SpecParts::default());
    }
}
