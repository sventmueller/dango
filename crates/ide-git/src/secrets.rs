//! The secrets check before a commit or push (I4), with gitleaks when the login shell's PATH has it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::id::CommitId;
use crate::repo::Repository;
use crate::run::{Call, Git};

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
    /// The line it ends on.
    pub end_line: u32,
    /// The commit it is in; `None` for staged changes.
    pub commit: Option<CommitId>,
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
    end_line: u32,
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
        let mut args = vec!["git"];
        args.extend_from_slice(what);
        Ok(match self.gitleaks(&args, None)? {
            None => SecretScan::NotInstalled,
            Some(findings) if findings.is_empty() => SecretScan::Clean,
            Some(findings) => SecretScan::Found(findings),
        })
    }

    /// Runs `gitleaks <command>` on the repository, or on `input` for its `stdin` command; `None` when the login
    /// shell's PATH has no gitleaks.
    pub(crate) fn gitleaks(&self, command: &[&str], input: Option<Vec<u8>>) -> Result<Option<Vec<Finding>>> {
        gitleaks(self.git(), self.root(), &self.pinned(), command, input)
    }
}

/// The gitleaks rules a work order's diff is checked with (C52): frozen from the user's working copy, never read from
/// the order's worktree, so an allowlist the builder adds to `.gitleaks.toml` or `.gitleaksignore`, or a
/// `gitleaks:allow` comment it writes, lets nothing through.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrozenRules {
    /// The frozen `.gitleaks.toml`; `None` for gitleaks' own rules, or those `GITLEAKS_CONFIG` names in the login
    /// shell's environment.
    pub config: Option<PathBuf>,
    /// The frozen `.gitleaksignore`; `None` for none.
    pub ignore: Option<PathBuf>,
}

/// A folder of one check's own with nothing in it, removed when dropped, where gitleaks finds no configuration.
struct EmptyFolder(PathBuf);

impl EmptyFolder {
    fn new() -> std::io::Result<EmptyFolder> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ide-git-scan-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path)?;
        Ok(EmptyFolder(path))
    }
}

impl Drop for EmptyFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.0);
    }
}

impl Git {
    /// Checks `text`, such as a work order's diff, with the `rules` frozen from the user's working copy (C52):
    /// gitleaks runs in an empty folder of its own with `--config` and `--gitleaks-ignore-path` pointing at the frozen
    /// files, and with `--ignore-gitleaks-allow`, so nothing in the scanned text or its folder can allow a finding.
    /// Without gitleaks the answer is `SecretScan::NotInstalled`, which a proof lists first (F75).
    pub fn scan_text_frozen(&self, text: &str, rules: &FrozenRules) -> Result<SecretScan> {
        let empty = EmptyFolder::new()?;
        let ignore = rules.ignore.as_deref().unwrap_or(&empty.0);
        let mut args = vec![
            "stdin".to_string(),
            "--ignore-gitleaks-allow".to_string(),
            format!("--gitleaks-ignore-path={}", ignore.display()),
        ];
        if let Some(config) = &rules.config {
            args.push(format!("--config={}", config.display()));
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        Ok(
            match gitleaks(self, &empty.0, &[], &args, Some(text.as_bytes().to_vec()))? {
                None => SecretScan::NotInstalled,
                Some(findings) if findings.is_empty() => SecretScan::Clean,
                Some(findings) => SecretScan::Found(findings),
            },
        )
    }

    /// Checks `text` as gitleaks reads standard input, run in the folder `dir`, which needs no repository: for text
    /// that is about to leave the Mac outside a commit.
    pub fn scan_text(&self, dir: &Path, text: &str) -> Result<SecretScan> {
        Ok(
            match gitleaks(self, dir, &[], &["stdin"], Some(text.as_bytes().to_vec()))? {
                None => SecretScan::NotInstalled,
                Some(findings) if findings.is_empty() => SecretScan::Clean,
                Some(findings) => SecretScan::Found(findings),
            },
        )
    }
}

/// Runs `gitleaks <command>` in `dir`, or on `input` for its `stdin` command; `None` when the login shell's PATH has no
/// gitleaks.
fn gitleaks(
    git: &Git,
    dir: &Path,
    env: &[(OsString, OsString)],
    command: &[&str],
    input: Option<Vec<u8>>,
) -> Result<Option<Vec<Finding>>> {
    let Some(gitleaks) = git.find("gitleaks") else {
        return Ok(None);
    };
    let mut call = Call::new(dir, command).envs(env).args([
        "--no-banner",
        "--no-color",
        "--redact",
        "--report-format=json",
        "--report-path=-",
        "--log-level=error",
        &format!("--exit-code={LEAKS_FOUND}"),
    ]);
    call = match input {
        Some(input) => call.input(input),
        None => call.args(["."]),
    };
    let output = git.run_program(&gitleaks, call)?;
    // gitleaks exits with 0 even when git failed under it, and says so only in its log.
    let failed = output
        .stderr
        .lines()
        .any(|l| l.contains(" ERR ") || l.contains(" FTL "));
    if !(output.code == 0 || output.code == LEAKS_FOUND) || (failed && output.code == 0) {
        return Err(Error::Failed {
            command: format!("gitleaks {}", command.first().unwrap_or(&"")),
            code: Some(output.code),
            output: output.said(),
        });
    }
    parse(&output.stdout).map(Some)
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
            end_line: r.end_line.max(r.start_line),
            commit: (!r.commit.is_empty()).then(|| CommitId::from_git(r.commit)),
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
                end_line: 2,
                commit: None,
            }]
        );
        assert!(parse(b"[]\n").unwrap().is_empty());
        assert!(parse(b"").unwrap().is_empty());
        assert!(parse(b"{not json").is_err());
    }
}
