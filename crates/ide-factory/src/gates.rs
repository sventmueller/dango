//! The automatic gates before the reviewer agent (E163), in their order: path scope, secrets scan, gate commands,
//! reviewer. The first failure stops the row, and its finding goes back to the builder with the cut-down log.

use globset::GlobSet;
use serde::{Deserialize, Serialize};

use crate::relevance::{Change, ChangeKind, glob_set};

/// One step of the review row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "step", content = "index")]
pub enum GateStep {
    /// The diff touches only the order's path scope; skipped, and logged so, without one (F54).
    PathScope,
    /// The secrets scan of the diff with the frozen configuration (C52).
    Secrets,
    /// The gate command at this index of the order's gate list.
    Command(usize),
    /// The reviewer agent (from stage 2).
    Reviewer,
}

/// What the secrets scan found, as the app maps `ide_git::SecretScan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scan")]
pub enum SecretScan {
    /// Nothing.
    Clean,
    /// Secrets, with where.
    Found {
        /// How many.
        count: u32,
        /// Files and lines, without the secrets themselves.
        places: Vec<String>,
    },
    /// gitleaks is not installed: "not checked", first in the evidence (F75).
    NotInstalled,
}

/// How many tests a gate ran, as `ide-test` read its output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCount {
    /// Tests that ran, skipped ones excluded.
    pub ran: u32,
    /// Tests that failed.
    pub failed: u32,
    /// Tests skipped or ignored.
    pub skipped: u32,
}

/// What a gate command did, as the app reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutcome {
    /// The exit status; `None` when the process group was ended.
    pub exit: Option<i32>,
    /// It ran past its deadline and its process group was ended (C67).
    pub timed_out: bool,
    /// It printed more than the output limit and was ended (C67).
    pub output_cut: bool,
    /// The tests it ran, when its tool's output could be read; `None` when no tool could be told.
    pub tests: Option<TestCount>,
    /// Paths `git status` showed after it; a gate must leave the tree clean (F53).
    pub left_behind: Vec<String>,
    /// How long it ran.
    pub duration_ms: u64,
    /// The cut-down log for the builder and the evidence (I8).
    pub excerpt: String,
    /// Pictures it wrote to its own folder outside the worktree (C69).
    pub images: Vec<String>,
}

/// Why a gate failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "failure")]
pub enum GateFailure {
    /// The diff leaves the path scope.
    OutsideScope {
        /// The paths outside.
        paths: Vec<String>,
    },
    /// The secrets scan found secrets.
    Secrets {
        /// How many.
        count: u32,
    },
    /// The command exited with this status.
    Exit {
        /// The status.
        code: i32,
    },
    /// The command ran past its deadline.
    TimedOut,
    /// The command printed more than the limit.
    OutputCut,
    /// The command ended without a status.
    Ended,
    /// The command ran no test, or only skipped ones (C68).
    NoTests,
    /// The reviewer found criteria not met or a finding of severity high (E179).
    Reviewer {
        /// The criteria not met.
        not_met: Vec<String>,
    },
    /// The command left files behind (F53).
    LeftBehind {
        /// The paths.
        paths: Vec<String>,
    },
}

/// The verdict on a gate command's outcome.
pub fn judge_command(outcome: &CommandOutcome) -> Result<(), GateFailure> {
    if outcome.timed_out {
        return Err(GateFailure::TimedOut);
    }
    if outcome.output_cut {
        return Err(GateFailure::OutputCut);
    }
    match outcome.exit {
        None => return Err(GateFailure::Ended),
        Some(0) => {}
        Some(code) => return Err(GateFailure::Exit { code }),
    }
    if outcome.tests.is_some_and(|t| t.ran == 0) {
        return Err(GateFailure::NoTests);
    }
    if !outcome.left_behind.is_empty() {
        return Err(GateFailure::LeftBehind {
            paths: outcome.left_behind.clone(),
        });
    }
    Ok(())
}

/// The verdict on the secrets scan; not installed passes and is named in the evidence.
pub fn judge_secrets(scan: &SecretScan) -> Result<(), GateFailure> {
    match scan {
        SecretScan::Found { count, .. } => Err(GateFailure::Secrets { count: *count }),
        SecretScan::Clean | SecretScan::NotInstalled => Ok(()),
    }
}

/// An order's path scope (E174): the patterns its diff may touch.
#[derive(Debug, Clone)]
pub struct PathScope {
    patterns: Vec<String>,
    set: GlobSet,
}

impl PathScope {
    /// The scope of `patterns`.
    pub fn new(patterns: &[String]) -> Result<PathScope, crate::relevance::PatternError> {
        Ok(PathScope {
            patterns: patterns.to_vec(),
            set: glob_set(patterns.iter().map(String::as_str))?,
        })
    }

    /// The patterns.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// The paths of `changes` outside the scope; a rename counts at both ends.
    pub fn outside(&self, changes: &[Change]) -> Vec<String> {
        let mut out = Vec::new();
        for change in changes {
            let mut paths = vec![change.path.as_str()];
            if let ChangeKind::Renamed(from) = &change.kind {
                paths.push(from);
            }
            for path in paths {
                if !self.set.is_match(path) && !out.iter().any(|p| p == path) {
                    out.push(path.to_string());
                }
            }
        }
        out
    }
}

/// The verdict on the path scope.
pub fn judge_scope(scope: &PathScope, changes: &[Change]) -> Result<(), GateFailure> {
    let paths = scope.outside(changes);
    if paths.is_empty() {
        Ok(())
    } else {
        Err(GateFailure::OutsideScope { paths })
    }
}

/// Whether two path scopes may touch the same file, so their orders run one after the other (E174). Patterns are
/// compared by their literal folder prefix, which errs towards overlapping.
pub fn scopes_overlap(a: Option<&[String]>, b: Option<&[String]>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return true;
    };
    a.iter().any(|x| b.iter().any(|y| prefixes_overlap(&literal_prefix(x), &literal_prefix(y))))
}

/// The folders of `pattern` before its first wildcard.
fn literal_prefix(pattern: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    for part in pattern.split('/') {
        if part.contains(['*', '?', '[', '{']) {
            return parts;
        }
        parts.push(part);
    }
    parts
}

fn prefixes_overlap(a: &[&str], b: &[&str]) -> bool {
    a.iter().zip(b).all(|(x, y)| x == y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok() -> CommandOutcome {
        CommandOutcome {
            exit: Some(0),
            tests: Some(TestCount {
                ran: 12,
                failed: 0,
                skipped: 1,
            }),
            ..CommandOutcome::default()
        }
    }

    #[test]
    fn gate_command_table() {
        type Case = (&'static str, fn(&mut CommandOutcome), Result<(), GateFailure>);
        let cases: [Case; 8] = [
            ("passes", |_| {}, Ok(())),
            ("non-zero exit", |o| o.exit = Some(2), Err(GateFailure::Exit { code: 2 })),
            ("ended", |o| o.exit = None, Err(GateFailure::Ended)),
            ("deadline", |o| o.timed_out = true, Err(GateFailure::TimedOut)),
            ("endless output", |o| o.output_cut = true, Err(GateFailure::OutputCut)),
            (
                "zero tests",
                |o| o.tests = Some(TestCount::default()),
                Err(GateFailure::NoTests),
            ),
            (
                "only skipped tests",
                |o| {
                    o.tests = Some(TestCount {
                        ran: 0,
                        failed: 0,
                        skipped: 9,
                    })
                },
                Err(GateFailure::NoTests),
            ),
            (
                "a file left behind",
                |o| o.left_behind = vec!["out.log".into()],
                Err(GateFailure::LeftBehind {
                    paths: vec!["out.log".into()],
                }),
            ),
        ];
        for (name, change, want) in cases {
            let mut outcome = ok();
            change(&mut outcome);
            assert_eq!(judge_command(&outcome), want, "{name}");
        }
    }

    #[test]
    fn a_missing_scanner_passes_but_secrets_fail() {
        assert_eq!(judge_secrets(&SecretScan::NotInstalled), Ok(()));
        assert_eq!(
            judge_secrets(&SecretScan::Found {
                count: 1,
                places: vec!["a.rs:3".into()]
            }),
            Err(GateFailure::Secrets { count: 1 })
        );
    }

    #[test]
    fn the_scope_names_paths_outside_it() {
        let scope = PathScope::new(&["src/export/**".into(), "tests/export.rs".into()]).unwrap();
        let changes = [
            Change::new("src/export/csv.rs", ChangeKind::Modified),
            Change::new("src/main.rs", ChangeKind::Modified),
            Change::new("src/export/x.rs", ChangeKind::Renamed("src/old.rs".into())),
        ];
        assert_eq!(
            judge_scope(&scope, &changes),
            Err(GateFailure::OutsideScope {
                paths: vec!["src/main.rs".into(), "src/old.rs".into()]
            })
        );
    }

    #[test]
    fn scopes_overlap_by_their_literal_prefixes() {
        let s = |list: &[&str]| list.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        let export = s(&["src/export/**"]);
        let parse = s(&["src/parse/**", "tests/parse.rs"]);
        let src = s(&["src/**"]);
        assert!(!scopes_overlap(Some(&export), Some(&parse)));
        assert!(scopes_overlap(Some(&export), Some(&src)));
        assert!(scopes_overlap(Some(&export), None));
        assert!(scopes_overlap(Some(&s(&["**/*.rs"])), Some(&parse)));
    }
}
