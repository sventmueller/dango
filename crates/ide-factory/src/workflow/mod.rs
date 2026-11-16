//! The workflow file `.dango/factory.toml` (E161): gate commands, roles, risk classes, limits, copying and setup of
//! worktrees, read from the user's working copy, never from an order's worktree (E172). It has its own version and
//! migration; [`edit`] writes a changed value back with comments and order kept (E183), and [`rules`] merges it with
//! the user's defaults so that it only tightens (E190).

mod defaults;
pub mod edit;
mod read;
pub mod recurring;
pub mod rules;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use toml_edit::{Document, DocumentMut, Item, TableLike};

pub use defaults::UserDefaults;
pub use read::Expected;
use read::{Reader, join};

use crate::budget::{Budget, Budgets};
use crate::limits::{Limit, Limits};
use crate::relevance::{RelevancePatterns, TestTool, glob_set};
use crate::routing::RankEntry;
use crate::station::{RiskClass, Role};

/// The workflow file's path within the project folder (`ide_product::PROJECT_DIR`).
pub const WORKFLOW_FILE: &str = ".dango/factory.toml";

/// The folder of the prompt templates within the project folder.
pub const TEMPLATE_DIR: &str = ".dango/factory";

/// The version this build writes and reads.
pub const VERSION: u32 = 1;

/// The migrations, the one at index `n` lifting a document from version `n` to `n + 1`; a file without `version` is
/// version 1, so there is none yet.
const MIGRATIONS: &[fn(&mut DocumentMut)] = &[];

/// What is wrong with the workflow file, with the key and place, for a sentence in the user's language (E141).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowError {
    /// The dotted key, such as `limits.turn_minutes`; empty for the file as a whole.
    pub key: String,
    /// The line, from one.
    pub line: Option<usize>,
    /// The column, from one.
    pub column: Option<usize>,
    /// What is wrong.
    pub kind: WorkflowErrorKind,
}

/// The kinds of errors in the workflow file.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum WorkflowErrorKind {
    /// The file is no valid TOML; `message` is the parser's own words, for the details.
    Syntax {
        /// The parser's words.
        message: String,
    },
    /// A key this version does not know.
    UnknownKey,
    /// A key holds the wrong kind of value.
    WrongKind {
        /// What it should hold.
        expected: Expected,
    },
    /// The file is of a newer version than this build reads.
    NewerVersion {
        /// The file's version.
        found: u64,
        /// The newest this build reads.
        supported: u32,
    },
    /// A path pattern does not parse.
    Pattern {
        /// The pattern.
        pattern: String,
        /// globset's words.
        message: String,
    },
    /// A gate has no command.
    EmptyCommand,
    /// Two gates have the same name.
    DuplicateGate {
        /// The name.
        name: String,
    },
    /// A path to copy is absolute or leaves the repository.
    CopyOutsideRepository {
        /// The path.
        path: String,
    },
    /// The folder for gate pictures lies inside the repository.
    ImagesInsideRepository {
        /// The path.
        path: String,
    },
    /// An environment variable's name is not a valid name.
    BadVariableName {
        /// The name.
        name: String,
    },
    /// A key the table needs is missing or empty.
    Missing,
    /// A recurring order's interval is none of `daily`, `weekly` or a number of days such as `14d`.
    BadInterval {
        /// What the file says.
        value: String,
    },
    /// Two recurring orders have the same title.
    DuplicateRecurring {
        /// The title.
        title: String,
    },
}

/// A gate command and the risk classes it applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GateCommand {
    /// Its name in the evidence.
    pub name: String,
    /// The shell command, run in the gate sandbox (E187).
    pub run: String,
    /// The classes it runs for; all when the file names none.
    pub classes: Vec<RiskClass>,
}

/// Where an order's worktree lies (E188).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeLocation {
    /// In the app's data folder.
    #[default]
    DataFolder,
    /// Beside the repository, so `../x` path dependencies resolve.
    Sibling,
}

/// Automatic intake from issues (E176).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IssueIntake {
    /// The label that marks an issue for the factory.
    pub label: String,
    /// The authors whose issues come in.
    pub authors: Vec<String>,
}

/// The workflow file as read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Workflow {
    gates: Vec<GateCommand>,
    gate_env: BTreeMap<String, String>,
    images: Option<String>,
    network: Vec<String>,
    sockets: Vec<String>,
    location: Option<WorktreeLocation>,
    copy: Vec<String>,
    setup: Option<String>,
    roles: BTreeMap<Role, Vec<RankEntry>>,
    reviewer_preferred: Option<bool>,
    skip_spec_approval: bool,
    intake: Option<IssueIntake>,
    tools: BTreeMap<Role, Vec<String>>,
    limits: Vec<(Limit, u64)>,
    budgets: Budgets,
    relevance: RelevancePatterns,
    recurring: Vec<recurring::RecurringOrder>,
}

/// A workflow file read, with the text it migrated to when it was of an older version.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    /// What the file says.
    pub workflow: Workflow,
    /// The file lifted to [`VERSION`], for writing back after the user agrees.
    pub migrated: Option<String>,
}

impl Workflow {
    /// The gate commands.
    pub fn gates(&self) -> &[GateCommand] {
        &self.gates
    }

    /// Environment variables for gates, such as a shared build cache (F70).
    pub fn gate_env(&self) -> &BTreeMap<String, String> {
        &self.gate_env
    }

    /// The folder outside the worktree where gates write pictures for the evidence (C69).
    pub fn images(&self) -> Option<&str> {
        self.images.as_deref()
    }

    /// Hosts gates and agents may reach (E173, E187).
    pub fn network(&self) -> &[String] {
        &self.network
    }

    /// Unix sockets gates may reach (C50).
    pub fn sockets(&self) -> &[String] {
        &self.sockets
    }

    /// Where worktrees lie, when the file says.
    pub fn location(&self) -> Option<WorktreeLocation> {
        self.location
    }

    /// Paths copied from the main checkout into a new worktree (E188).
    pub fn copy(&self) -> &[String] {
        &self.copy
    }

    /// The setup command for a new worktree.
    pub fn setup(&self) -> Option<&str> {
        self.setup.as_deref()
    }

    /// The ranked list of `role`, when the file gives one.
    pub fn role(&self, role: Role) -> Option<&[RankEntry]> {
        self.roles.get(&role).map(Vec::as_slice)
    }

    /// Whether the file sets the reviewer rule to "preferred" (`Some(true)`) or "required".
    pub fn reviewer_preferred(&self) -> Option<bool> {
        self.reviewer_preferred
    }

    /// Whether the file allows skipping G1 for orders of class low.
    pub fn skip_spec_approval(&self) -> bool {
        self.skip_spec_approval
    }

    /// Automatic intake from issues.
    pub fn intake(&self) -> Option<&IssueIntake> {
        self.intake.as_ref()
    }

    /// Tools switched on per role against E191's default, such as an MCP server.
    pub fn tools(&self, role: Role) -> &[String] {
        self.tools.get(&role).map_or(&[], Vec::as_slice)
    }

    /// The limits the file sets.
    pub fn limits(&self) -> &[(Limit, u64)] {
        &self.limits
    }

    /// The file's limits over `base`, unset ones taken from it.
    pub fn limits_over(&self, base: &Limits) -> Limits {
        self.limits
            .iter()
            .fold(base.clone(), |limits, &(limit, value)| limits.with(limit, value))
    }

    /// The budgets the file sets.
    pub fn budgets(&self) -> &Budgets {
        &self.budgets
    }

    /// The patterns for risk classes and relevant changes.
    pub fn relevance(&self) -> &RelevancePatterns {
        &self.relevance
    }

    /// The recurring cleanup orders (B47).
    pub fn recurring(&self) -> &[recurring::RecurringOrder] {
        &self.recurring
    }
}

/// Reads the workflow file `text`, migrating an older version; every error found is reported.
pub fn parse(text: &str) -> Result<Parsed, Vec<WorkflowError>> {
    let document = Document::parse(text).map_err(|error| vec![syntax(text, &error)])?;
    let version = match document.as_table().get("version") {
        None => 1,
        Some(item) => match item
            .as_integer()
            .and_then(|n| u64::try_from(n).ok())
            .filter(|n| *n >= 1)
        {
            Some(n) => n,
            None => {
                let mut reader = Reader::new(text);
                reader.error(
                    "version",
                    item.span(),
                    WorkflowErrorKind::WrongKind {
                        expected: Expected::Count,
                    },
                );
                return Err(reader.errors);
            }
        },
    };
    if version > u64::from(VERSION) {
        let mut reader = Reader::new(text);
        let span = document.as_table().get("version").and_then(Item::span);
        reader.error(
            "version",
            span,
            WorkflowErrorKind::NewerVersion {
                found: version,
                supported: VERSION,
            },
        );
        return Err(reader.errors);
    }
    let migrated = migrate(text, version);
    let source = migrated.as_deref().unwrap_or(text);
    let document = Document::parse(source).map_err(|error| vec![syntax(source, &error)])?;
    let mut reader = Reader::new(source);
    let workflow = read_workflow(&mut reader, document.as_table());
    if reader.errors.is_empty() {
        Ok(Parsed { workflow, migrated })
    } else {
        Err(reader.errors)
    }
}

/// `text` lifted from `version` to [`VERSION`], or `None` when it is current.
fn migrate(text: &str, version: u64) -> Option<String> {
    let from = usize::try_from(version).ok()?;
    let steps = MIGRATIONS.get(from.saturating_sub(1)..)?;
    if steps.is_empty() {
        return None;
    }
    let mut document: DocumentMut = text.parse().ok()?;
    for step in steps {
        step(&mut document);
    }
    document.insert("version", toml_edit::value(i64::from(VERSION)));
    Some(document.to_string())
}

fn syntax(text: &str, error: &toml_edit::TomlError) -> WorkflowError {
    let mut reader = Reader::new(text);
    reader.error(
        "",
        error.span(),
        WorkflowErrorKind::Syntax {
            message: error.message().to_string(),
        },
    );
    reader.errors.remove(0)
}

const TOP: &[&str] = &[
    "version",
    "gates",
    "worktree",
    "roles",
    "review",
    "spec",
    "intake",
    "tools",
    "limits",
    "budget",
    "risk",
    "relevant",
    "recurring",
];

fn read_workflow(r: &mut Reader<'_>, top: &dyn TableLike) -> Workflow {
    r.known(top, "", TOP);
    let mut workflow = Workflow::default();
    if let Some(gates) = r.table(top, "", "gates") {
        read_gates(r, gates, &mut workflow);
    }
    if let Some(worktree) = r.table(top, "", "worktree") {
        read_worktree(r, worktree, &mut workflow);
    }
    if let Some(roles) = r.table(top, "", "roles") {
        workflow.roles = read_roles(r, roles);
    }
    if let Some(review) = r.table(top, "", "review") {
        r.known(review, "review", &["rule"]);
        workflow.reviewer_preferred = r
            .word(review, "review", "rule", &["required", "preferred"])
            .map(|w| w == "preferred");
    }
    if let Some(spec) = r.table(top, "", "spec") {
        r.known(spec, "spec", &["skip_approval"]);
        workflow.skip_spec_approval = r.switch(spec, "spec", "skip_approval").unwrap_or(false);
    }
    if let Some(intake) = r.table(top, "", "intake") {
        r.known(intake, "intake", &["label", "authors"]);
        let label = r.text(intake, "intake", "label");
        let authors = r.texts(intake, "intake", "authors");
        workflow.intake = label.map(|label| IssueIntake { label, authors });
    }
    if let Some(tools) = r.table(top, "", "tools") {
        let keys: Vec<&str> = Role::ALL.iter().map(|role| role.key()).collect();
        r.known(tools, "tools", &keys);
        for role in Role::ALL {
            let names = r.texts(tools, "tools", role.key());
            if !names.is_empty() {
                workflow.tools.insert(role, names);
            }
        }
    }
    if let Some(limits) = r.table(top, "", "limits") {
        workflow.limits = read_limits(r, limits);
    }
    if let Some(budget) = r.table(top, "", "budget") {
        workflow.budgets = read_budgets(r, budget);
    }
    workflow.relevance = read_relevance(r, top);
    workflow.recurring = recurring::read(r, top);
    workflow
}

fn read_gates(r: &mut Reader<'_>, gates: &dyn TableLike, workflow: &mut Workflow) {
    r.known(gates, "gates", &["command", "env", "images", "network", "sockets"]);
    for item in r.tables(gates, "gates", "command") {
        let Some(table) = item.as_table_like() else { continue };
        let path = "gates.command";
        r.known(table, path, &["name", "run", "classes"]);
        let run = r.text(table, path, "run").unwrap_or_default();
        let name = r.text(table, path, "name").unwrap_or_else(|| run.clone());
        let classes: Vec<RiskClass> = r
            .texts(table, path, "classes")
            .iter()
            .filter_map(|word| {
                let class = RiskClass::from_key(word);
                if class.is_none() {
                    r.error(
                        &join(path, "classes"),
                        table.get("classes").and_then(Item::span),
                        WorkflowErrorKind::WrongKind {
                            expected: Expected::OneOf(&["low", "medium", "high"]),
                        },
                    );
                }
                class
            })
            .collect();
        if run.trim().is_empty() {
            r.error(&join(path, "run"), item.span(), WorkflowErrorKind::EmptyCommand);
        } else if workflow.gates.iter().any(|g| g.name == name) {
            r.error(
                &join(path, "name"),
                item.span(),
                WorkflowErrorKind::DuplicateGate { name },
            );
        } else {
            workflow.gates.push(GateCommand { name, run, classes });
        }
    }
    if let Some(env) = r.table(gates, "gates", "env") {
        for (name, _) in env.iter() {
            let path = join("gates.env", name);
            let valid = !name.is_empty()
                && !name.starts_with(|c: char| c.is_ascii_digit())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !valid {
                let span = env.get(name).and_then(Item::span);
                r.error(
                    &path,
                    span,
                    WorkflowErrorKind::BadVariableName { name: name.to_string() },
                );
            } else if let Some(value) = r.text(env, "gates.env", name) {
                workflow.gate_env.insert(name.to_string(), value);
            }
        }
    }
    if let Some(images) = r.text(gates, "gates", "images") {
        if images.starts_with('/') || images.starts_with("~/") {
            workflow.images = Some(images);
        } else {
            let span = gates.get("images").and_then(Item::span);
            r.error(
                "gates.images",
                span,
                WorkflowErrorKind::ImagesInsideRepository { path: images },
            );
        }
    }
    workflow.network = r.texts(gates, "gates", "network");
    workflow.sockets = r.texts(gates, "gates", "sockets");
}

fn read_worktree(r: &mut Reader<'_>, worktree: &dyn TableLike, workflow: &mut Workflow) {
    r.known(worktree, "worktree", &["location", "copy", "setup"]);
    workflow.location = r.word(worktree, "worktree", "location", &["data", "sibling"]).map(|w| {
        if w == "sibling" {
            WorktreeLocation::Sibling
        } else {
            WorktreeLocation::DataFolder
        }
    });
    for path in r.texts(worktree, "worktree", "copy") {
        let leaves = path.starts_with('/') || path.split('/').any(|part| part == "..") || path.is_empty();
        if leaves {
            let span = worktree.get("copy").and_then(Item::span);
            r.error("worktree.copy", span, WorkflowErrorKind::CopyOutsideRepository { path });
        } else {
            workflow.copy.push(path);
        }
    }
    workflow.setup = r.text(worktree, "worktree", "setup").filter(|s| !s.trim().is_empty());
}

fn read_roles(r: &mut Reader<'_>, roles: &dyn TableLike) -> BTreeMap<Role, Vec<RankEntry>> {
    let keys: Vec<&str> = Role::ALL.iter().map(|role| role.key()).collect();
    r.known(roles, "roles", &keys);
    let mut out = BTreeMap::new();
    for role in Role::ALL {
        let path = join("roles", role.key());
        let mut entries = Vec::new();
        for item in r.tables(roles, "roles", role.key()) {
            let Some(table) = item.as_table_like() else { continue };
            r.known(table, &path, &["agent", "model", "effort"]);
            let Some(agent) = r.text(table, &path, "agent") else {
                r.error(
                    &join(&path, "agent"),
                    item.span(),
                    WorkflowErrorKind::WrongKind {
                        expected: Expected::Text,
                    },
                );
                continue;
            };
            entries.push(RankEntry {
                agent: agent.into(),
                model: r.text(table, &path, "model"),
                effort: r.text(table, &path, "effort"),
            });
        }
        if !entries.is_empty() {
            out.insert(role, entries);
        }
    }
    out
}

fn read_limits(r: &mut Reader<'_>, limits: &dyn TableLike) -> Vec<(Limit, u64)> {
    let keys: Vec<&str> = Limit::ALL.iter().map(|limit| limit.key()).collect();
    r.known(limits, "limits", &keys);
    Limit::ALL
        .into_iter()
        .filter_map(|limit| r.count(limits, "limits", limit.key()).map(|value| (limit, value)))
        .collect()
}

fn read_budget(r: &mut Reader<'_>, table: &dyn TableLike, path: &str) -> Budget {
    r.known(table, path, &["dollars", "tokens", "window_percent"]);
    Budget::new(
        r.number(table, path, "dollars"),
        r.count(table, path, "tokens"),
        r.number(table, path, "window_percent"),
    )
}

fn read_budgets(r: &mut Reader<'_>, budget: &dyn TableLike) -> Budgets {
    r.known(budget, "budget", &["order", "day", "providers"]);
    let order = r
        .table(budget, "budget", "order")
        .map(|t| read_budget(r, t, "budget.order"));
    let day = r
        .table(budget, "budget", "day")
        .map(|t| read_budget(r, t, "budget.day"));
    let mut providers = BTreeMap::new();
    if let Some(table) = r.table(budget, "budget", "providers") {
        for (service, _) in table.iter() {
            let path = join("budget.providers", service);
            if let Some(inner) = r.table(table, "budget.providers", service) {
                providers.insert(service.to_string(), read_budget(r, inner, &path));
            }
        }
    }
    Budgets::new(order.unwrap_or_default(), day.unwrap_or_default(), providers)
}

fn read_relevance(r: &mut Reader<'_>, top: &dyn TableLike) -> RelevancePatterns {
    let mut patterns = RelevancePatterns::default();
    if let Some(risk) = r.table(top, "", "risk") {
        r.known(risk, "risk", &["high", "medium"]);
        patterns.high = checked(r, risk, "risk", "high");
        patterns.medium = checked(r, risk, "risk", "medium");
    }
    if let Some(relevant) = r.table(top, "", "relevant") {
        r.known(relevant, "relevant", &["tools", "tests", "config"]);
        const TOOLS: &[&str] = &["cargo", "go", "python", "javascript"];
        let words = r.texts(relevant, "relevant", "tools");
        for word in words {
            match TestTool::ALL.into_iter().zip(TOOLS).find(|(_, w)| **w == word) {
                Some((tool, _)) => patterns.tools.push(tool),
                None => {
                    let span = relevant.get("tools").and_then(Item::span);
                    r.error(
                        "relevant.tools",
                        span,
                        WorkflowErrorKind::WrongKind {
                            expected: Expected::OneOf(TOOLS),
                        },
                    );
                }
            }
        }
        patterns.tests = checked(r, relevant, "relevant", "tests");
        patterns.config = checked(r, relevant, "relevant", "config");
    }
    patterns
}

/// The patterns at `key`, each checked by globset.
fn checked(r: &mut Reader<'_>, table: &dyn TableLike, path: &str, key: &str) -> Vec<String> {
    let patterns = r.texts(table, path, key);
    let mut good = Vec::new();
    for pattern in patterns {
        match glob_set([pattern.as_str()]) {
            Ok(_) => good.push(pattern),
            Err(error) => {
                let span = table.get(key).and_then(Item::span);
                r.error(
                    &join(path, key),
                    span,
                    WorkflowErrorKind::Pattern {
                        pattern: error.pattern,
                        message: error.message,
                    },
                );
            }
        }
    }
    good
}

#[cfg(test)]
mod tests;
