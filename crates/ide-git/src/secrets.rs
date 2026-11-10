//! The secrets check before a commit or push (I4), with gitleaks when the login shell's PATH has it.

use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::repo::Repository;
use crate::run::Call;

/// The exit code gitleaks is told to use for findings, so that 1 still means it failed.
const LEAKS_FOUND: i32 = 3;

/// The outcome of a secrets check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretScan {
    /// gitleaks is not on the login shell's PATH, so nothing was checked.
    NotInstalled,
    /// Nothing was found.
    Clean,
    /// Possible secrets; their values are not included.
    Found(Vec<Finding>),
}

/// A possible secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The gitleaks rule, such as `github-pat`.
    pub rule: String,
    /// What the rule looks for.
    pub description: String,
    /// The file, relative to the repository's root.
    pub file: PathBuf,
    /// The line it starts on.
    pub line: u32,
    /// The commit it is in; `None` for staged changes.
    pub commit: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Report {
    #[serde(rename = "RuleID")]
    rule_id: String,
    #[serde(default)]
    description: String,
    file: String,
    #[serde(default)]
    start_line: u32,
    #[serde(default)]
    commit: String,
}

impl Repository {
    /// Checks what is staged, before a commit.
    pub fn scan_staged(&self) -> Result<SecretScan> {
        self.scan(&["--staged"])
    }

    /// Checks the commits of HEAD that no remote-tracking branch has, before a push.
    pub fn scan_unpushed(&self) -> Result<SecretScan> {
        if self.head()?.is_none() {
            return Ok(SecretScan::Clean);
        }
        self.scan(&["--log-opts=HEAD --not --remotes"])
    }

    fn scan(&self, what: &[&str]) -> Result<SecretScan> {
        let Some(gitleaks) = self.git().find("gitleaks") else {
            return Ok(SecretScan::NotInstalled);
        };
        let call = Call::new(self.root(), ["git"]).args(what).args([
            "--no-banner",
            "--no-color",
            "--redact",
            "--report-format=json",
            "--report-path=-",
            "--log-level=error",
            &format!("--exit-code={LEAKS_FOUND}"),
            ".",
        ]);
        let output = self.git().run_program(&gitleaks, call)?;
        // gitleaks exits with 0 even when git failed under it, and says so only in its log.
        let failed = output
            .stderr
            .lines()
            .any(|l| l.contains(" ERR ") || l.contains(" FTL "));
        if !(output.code == 0 || output.code == LEAKS_FOUND) || (failed && output.code == 0) {
            return Err(Error::Failed {
                command: "gitleaks git".into(),
                code: Some(output.code),
                output: output.said(),
            });
        }
        let findings = parse(&output.stdout)?;
        Ok(if findings.is_empty() {
            SecretScan::Clean
        } else {
            SecretScan::Found(findings)
        })
    }
}

fn parse(report: &[u8]) -> Result<Vec<Finding>> {
    let text = String::from_utf8_lossy(report);
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let reports: Vec<Report> = serde_json::from_str(text.trim()).map_err(|e| Error::Unexpected {
        what: format!("gitleaks report: {e}"),
    })?;
    Ok(reports
        .into_iter()
        .map(|r| Finding {
            rule: r.rule_id,
            description: r.description,
            file: PathBuf::from(r.file),
            line: r.start_line,
            commit: (!r.commit.is_empty()).then_some(r.commit),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_gives_rule_file_and_line_without_the_secret() {
        let report = br#"[{"RuleID":"github-pat","Description":"GitHub token","StartLine":2,"EndLine":2,
            "Match":"REDACTED","Secret":"REDACTED","File":"s p.env","Commit":"","Tags":[],"Fingerprint":"x"}]"#;
        let findings = parse(report).unwrap();
        assert_eq!(
            findings,
            [Finding {
                rule: "github-pat".into(),
                description: "GitHub token".into(),
                file: PathBuf::from("s p.env"),
                line: 2,
                commit: None,
            }]
        );
        assert!(parse(b"[]\n").unwrap().is_empty());
        assert!(parse(b"").unwrap().is_empty());
        assert!(parse(b"{not json").is_err());
    }
}
