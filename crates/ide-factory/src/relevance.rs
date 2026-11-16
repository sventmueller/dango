//! Which changes of a diff matter for the checks (E172, C54): agent and scanner configuration, a fixed group nobody
//! can deselect; changes to existing tests; lint, CI and build configuration and the workflow file. Both lift the
//! measured risk class to high (E163) and come first in the evidence (R69). New test files do not count.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::station::RiskClass;

/// How a file changed, as `git diff --name-status` says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "from")]
pub enum ChangeKind {
    /// A new file.
    Added,
    /// Changed content.
    Modified,
    /// Removed.
    Deleted,
    /// Moved from the path given, possibly with changes.
    Renamed(String),
    /// Copied from the path given.
    Copied(String),
    /// Changed between file, link and submodule.
    TypeChanged,
}

/// One changed path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// The path relative to the repository, after the change.
    pub path: String,
    /// How it changed.
    pub kind: ChangeKind,
}

impl Change {
    /// A change of `kind` at `path`.
    pub fn new(path: &str, kind: ChangeKind) -> Change {
        Change {
            path: path.to_string(),
            kind,
        }
    }

    /// The paths the change touches: for a rename both ends.
    fn paths(&self) -> Vec<&str> {
        match &self.kind {
            ChangeKind::Renamed(from) => vec![from.as_str(), self.path.as_str()],
            _ => vec![self.path.as_str()],
        }
    }

    /// The path the file had before, if it existed: changes to it count for existing tests.
    fn existing(&self) -> Option<&str> {
        match &self.kind {
            ChangeKind::Added | ChangeKind::Copied(_) => None,
            ChangeKind::Renamed(from) => Some(from),
            ChangeKind::Modified | ChangeKind::Deleted | ChangeKind::TypeChanged => Some(&self.path),
        }
    }
}

/// The agent and scanner configuration (C52, C57): frozen from the main checkout per order, never loaded from the
/// worktree, and not deselectable. The own agent reads `AGENTS.md`, `CLAUDE.md` and `.claude/CLAUDE.md`.
pub const AGENT_CONFIG: &[&str] = &[
    "**/CLAUDE.md",
    "**/CLAUDE.local.md",
    "**/AGENTS.md",
    "**/.claude/**",
    ".mcp.json",
    ".codex/**",
    "opencode.json",
    "opencode.jsonc",
    ".opencode/**",
    ".gitleaks.toml",
    ".gitleaksignore",
];

/// The workflow file and its templates (E161).
pub const WORKFLOW_PATHS: &[&str] = &[".dango/factory.toml", ".dango/factory/**"];

/// CI configuration that counts whatever the tool.
const CI_CONFIG: &[&str] = &[
    ".github/**",
    ".gitlab-ci.yml",
    ".gitlab/**",
    ".circleci/**",
    ".buildkite/**",
    "Jenkinsfile",
    "azure-pipelines.yml",
    ".pre-commit-config.yaml",
    "**/Makefile",
    "**/makefile",
    "**/GNUmakefile",
    "**/justfile",
    "**/Justfile",
];

/// A test tool whose default paths apply, after `ide-test`'s tools (E110).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestTool {
    /// Cargo and cargo-nextest.
    Cargo,
    /// Go.
    Go,
    /// pytest.
    Python,
    /// Vitest and Jest.
    JavaScript,
}

impl TestTool {
    /// Every tool.
    pub const ALL: [TestTool; 4] = [TestTool::Cargo, TestTool::Go, TestTool::Python, TestTool::JavaScript];

    /// Paths of test files; Rust's unit tests inside source files cannot be told apart by path.
    pub fn tests(self) -> &'static [&'static str] {
        match self {
            TestTool::Cargo => &[
                "**/tests/**",
                "**/tests.rs",
                "**/*_test.rs",
                "**/*_tests.rs",
                "**/benches/**",
                "**/fuzz/**",
                "**/*.proptest-regressions",
            ],
            TestTool::Go => &["**/*_test.go", "**/testdata/**"],
            TestTool::Python => &["**/test_*.py", "**/*_test.py", "**/tests/**", "**/conftest.py"],
            TestTool::JavaScript => &["**/*.test.*", "**/*.spec.*", "**/__tests__/**"],
        }
    }

    /// Paths of lint and build configuration.
    pub fn config(self) -> &'static [&'static str] {
        match self {
            TestTool::Cargo => &[
                "**/Cargo.toml",
                "**/build.rs",
                "**/.cargo/**",
                "**/clippy.toml",
                "**/.clippy.toml",
                "**/rustfmt.toml",
                "**/.rustfmt.toml",
                "rust-toolchain",
                "rust-toolchain.toml",
                "deny.toml",
                "xtask/**",
            ],
            TestTool::Go => &["**/go.mod", "**/.golangci.*"],
            TestTool::Python => &[
                "**/pyproject.toml",
                "**/setup.cfg",
                "**/setup.py",
                "**/tox.ini",
                "**/pytest.ini",
                "**/.flake8",
                "**/ruff.toml",
                "**/.ruff.toml",
                "**/mypy.ini",
                "**/noxfile.py",
            ],
            TestTool::JavaScript => &[
                "**/package.json",
                "**/tsconfig*.json",
                "**/.eslintrc*",
                "**/eslint.config.*",
                "**/.prettierrc*",
                "**/prettier.config.*",
                "**/vitest.config.*",
                "**/vite.config.*",
                "**/jest.config.*",
                "**/babel.config.*",
                "**/.babelrc*",
                "**/biome.json",
                "**/biome.jsonc",
            ],
        }
    }
}

/// A path pattern that does not parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternError {
    /// The pattern.
    pub pattern: String,
    /// globset's words for what is wrong.
    pub message: String,
}

/// Builds one set from `patterns`; `*` does not cross `/`, `**` does.
pub fn glob_set<'a>(patterns: impl IntoIterator<Item = &'a str>) -> Result<GlobSet, PatternError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|error| PatternError {
                pattern: pattern.to_string(),
                message: error.kind().to_string(),
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|error| PatternError {
        pattern: String::new(),
        message: error.to_string(),
    })
}

/// The patterns a project's diff is classified by.
#[derive(Debug, Clone)]
pub struct Relevance {
    agent: GlobSet,
    tests: GlobSet,
    config: GlobSet,
    high: GlobSet,
    medium: GlobSet,
}

/// What the patterns of a [`Relevance`] are made of, as the workflow file and its defaults give them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelevancePatterns {
    /// The tools whose defaults apply; empty means all of them.
    pub tools: Vec<TestTool>,
    /// More test paths.
    pub tests: Vec<String>,
    /// More lint, CI and build configuration.
    pub config: Vec<String>,
    /// Paths of risk class high.
    pub high: Vec<String>,
    /// Paths of risk class medium.
    pub medium: Vec<String>,
}

impl Relevance {
    /// The classifier of `patterns`.
    pub fn new(patterns: &RelevancePatterns) -> Result<Relevance, PatternError> {
        let tools: &[TestTool] = if patterns.tools.is_empty() {
            &TestTool::ALL
        } else {
            &patterns.tools
        };
        let tests = tools.iter().flat_map(|t| t.tests().iter().copied());
        let config = tools
            .iter()
            .flat_map(|t| t.config().iter().copied())
            .chain(CI_CONFIG.iter().copied())
            .chain(WORKFLOW_PATHS.iter().copied());
        Ok(Relevance {
            agent: glob_set(AGENT_CONFIG.iter().copied())?,
            tests: glob_set(tests.chain(patterns.tests.iter().map(String::as_str)))?,
            config: glob_set(config.chain(patterns.config.iter().map(String::as_str)))?,
            high: glob_set(patterns.high.iter().map(String::as_str))?,
            medium: glob_set(patterns.medium.iter().map(String::as_str))?,
        })
    }

    /// The classification of `changes`.
    pub fn classify(&self, changes: &[Change]) -> Classification {
        let mut out = Classification::default();
        for change in changes {
            let paths = change.paths();
            if paths.iter().any(|p| self.agent.is_match(p)) {
                out.agent_config.push(change.path.clone());
            }
            if change.existing().is_some_and(|p| self.tests.is_match(p)) {
                out.tests.push(change.path.clone());
            }
            if paths.iter().any(|p| self.config.is_match(p)) {
                out.config.push(change.path.clone());
            }
            let class = if paths.iter().any(|p| self.high.is_match(p)) {
                RiskClass::High
            } else if paths.iter().any(|p| self.medium.is_match(p)) {
                RiskClass::Medium
            } else {
                RiskClass::Low
            };
            out.measured = out.measured.max(class);
        }
        if !out.agent_config.is_empty() || !out.tests.is_empty() || !out.config.is_empty() {
            out.measured = RiskClass::High;
        }
        out
    }
}

/// What a diff changes that matters for the checks, and its measured risk class.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Classification {
    /// Changed agent or scanner configuration; the order waits for the user before the review (E172).
    pub agent_config: Vec<String>,
    /// Changed existing tests.
    pub tests: Vec<String>,
    /// Changed lint, CI or build configuration or the workflow file.
    pub config: Vec<String>,
    /// The measured risk class.
    pub measured: RiskClass,
}

impl Classification {
    /// Whether anything relevant for the checks changed.
    pub fn relevant(&self) -> bool {
        !self.agent_config.is_empty() || !self.tests.is_empty() || !self.config.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relevance() -> Relevance {
        Relevance::new(&RelevancePatterns {
            high: vec!["src/auth/**".into()],
            medium: vec!["src/**".into()],
            ..RelevancePatterns::default()
        })
        .unwrap()
    }

    #[test]
    fn changes_relevant_for_the_checks() {
        use ChangeKind::*;
        let cases: [(&str, ChangeKind, &str, RiskClass); 14] = [
            ("tests/new_case.rs", Added, "", RiskClass::Low),
            ("tests/parser.rs", Modified, "tests", RiskClass::High),
            ("crates/a/tests/parser.rs", Deleted, "tests", RiskClass::High),
            (
                "tests/moved.rs",
                Renamed("tests/parser.rs".into()),
                "tests",
                RiskClass::High,
            ),
            ("pkg/handler_test.go", Modified, "tests", RiskClass::High),
            ("web/app.test.ts", Added, "", RiskClass::Low),
            ("Cargo.toml", Modified, "config", RiskClass::High),
            ("crates/x/build.rs", Added, "config", RiskClass::High),
            (".github/workflows/ci.yml", Modified, "config", RiskClass::High),
            (".dango/factory.toml", Modified, "config", RiskClass::High),
            ("CLAUDE.md", Modified, "agent", RiskClass::High),
            ("sub/.claude/settings.json", Added, "agent", RiskClass::High),
            ("src/auth/login.rs", Modified, "", RiskClass::High),
            ("src/lib.rs", Modified, "", RiskClass::Medium),
        ];
        for (path, kind, group, class) in cases {
            let got = relevance().classify(&[Change::new(path, kind.clone())]);
            let groups: Vec<&str> = [
                (!got.agent_config.is_empty()).then_some("agent"),
                (!got.tests.is_empty()).then_some("tests"),
                (!got.config.is_empty()).then_some("config"),
            ]
            .into_iter()
            .flatten()
            .collect();
            assert_eq!(groups.join(","), group, "{path} {kind:?}");
            assert_eq!(got.measured, class, "{path} {kind:?}");
        }
    }

    #[test]
    fn a_test_spoilt_by_one_inserted_line_counts_like_any_change() {
        let got = relevance().classify(&[Change::new("tests/parser.rs", ChangeKind::Modified)]);
        assert_eq!(got.tests, ["tests/parser.rs"]);
        assert!(got.relevant());
    }

    #[test]
    fn the_agent_configuration_cannot_be_deselected() {
        let got = Relevance::new(&RelevancePatterns {
            tools: vec![TestTool::Go],
            ..RelevancePatterns::default()
        })
        .unwrap()
        .classify(&[Change::new(".gitleaksignore", ChangeKind::Added)]);
        assert_eq!(got.agent_config, [".gitleaksignore"]);
    }

    #[test]
    fn a_star_does_not_cross_folders() {
        let set = glob_set(["src/*.rs"]).unwrap();
        assert!(set.is_match("src/lib.rs") && !set.is_match("src/a/lib.rs"));
        let error = glob_set(["src/[a"]).unwrap_err();
        assert_eq!(error.pattern, "src/[a");
    }

    #[test]
    fn plain_code_is_low_without_patterns() {
        let got = Relevance::new(&RelevancePatterns::default())
            .unwrap()
            .classify(&[Change::new("src/lib.rs", ChangeKind::Modified)]);
        assert_eq!(got, Classification::default());
    }
}
