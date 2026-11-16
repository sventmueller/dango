//! The rule set an order runs under (E190): the user's defaults tightened by the workflow file. Limits and budgets take
//! the stricter value; loosenings and the gate command set apply only once the user confirmed them for this project on
//! this Mac, which the app keeps as hashes in its state, never in the repository.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{GateCommand, IssueIntake, UserDefaults, Workflow, WorkflowError, WorktreeLocation};
use crate::budget::Budgets;
use crate::hash::hash_json;
use crate::limits::Limits;
use crate::relevance::{PatternError, Relevance, RelevancePatterns};
use crate::routing::{RankEntry, ReviewerRule};
use crate::station::{RiskClass, Role};

/// A loosening the workflow file asks for, which applies only after the user's confirmation (E190).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "loosening")]
pub enum Loosening {
    /// Skipping G1 for orders of class low.
    SkipSpecApproval,
    /// Automatic intake from issues with this label and these authors (E176).
    IssueIntake {
        /// The label and authors.
        intake: IssueIntake,
    },
    /// The reviewer rule "preferred".
    ReviewerPreferred,
    /// Copying files with credentials into worktrees, each named on its own (E188, C50).
    CopyCredentials {
        /// The paths.
        paths: Vec<String>,
    },
    /// Tools switched on for a role (E191).
    Tools {
        /// The role.
        role: Role,
        /// The tools.
        tools: Vec<String>,
    },
}

impl Loosening {
    /// The hash the app stores when the user confirms this loosening; a changed value needs a new confirmation.
    pub fn hash(&self) -> String {
        hash_json(self)
    }
}

/// The hashes the user confirmed for one project on this Mac.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Confirmations {
    hashes: BTreeSet<String>,
}

impl Confirmations {
    /// Confirmations of `hashes`.
    pub fn new(hashes: impl IntoIterator<Item = String>) -> Confirmations {
        Confirmations {
            hashes: hashes.into_iter().collect(),
        }
    }

    /// Whether `hash` is confirmed.
    pub fn has(&self, hash: &str) -> bool {
        self.hashes.contains(hash)
    }

    /// Confirms `hash`.
    pub fn add(&mut self, hash: String) {
        self.hashes.insert(hash);
    }

    /// Every confirmed hash.
    pub fn hashes(&self) -> impl Iterator<Item = &String> {
        self.hashes.iter()
    }
}

/// Names of files that hold credentials and are copied only when named on their own and confirmed (C50).
const CREDENTIAL_NAMES: &[&str] = &[
    ".env",
    ".npmrc",
    ".netrc",
    ".pypirc",
    ".pgpass",
    "credentials",
    "secrets",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
];
const CREDENTIAL_ENDINGS: &[&str] = &[".pem", ".key", ".p12", ".pfx", ".keychain", ".keychain-db"];

/// Whether `path` looks like a file with credentials.
pub fn holds_credentials(path: &str) -> bool {
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or(path);
    CREDENTIAL_NAMES
        .iter()
        .any(|known| name == *known || name.starts_with(&format!("{known}.")))
        || CREDENTIAL_ENDINGS.iter().any(|ending| name.ends_with(ending))
}

/// What gates and setup run: the set whose hash the user confirms (E190).
#[derive(Serialize)]
struct GateSet<'a> {
    gates: &'a [GateCommand],
    env: &'a BTreeMap<String, String>,
    setup: Option<&'a str>,
    network: &'a [String],
    sockets: &'a [String],
}

/// The effective rule set of a project.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rules {
    limits: Limits,
    budgets: Budgets,
    roles: BTreeMap<Role, Vec<RankEntry>>,
    reviewer_rule: ReviewerRule,
    skip_spec_approval: bool,
    intake: Option<IssueIntake>,
    tools: BTreeMap<Role, Vec<String>>,
    gates: Vec<GateCommand>,
    gate_env: BTreeMap<String, String>,
    images: Option<String>,
    network: Vec<String>,
    sockets: Vec<String>,
    location: WorktreeLocation,
    copy: Vec<String>,
    setup: Option<String>,
    relevance: RelevancePatterns,
    templates: BTreeMap<String, String>,
    resume_after_restart: bool,
    awake_on_battery: bool,
    #[serde(skip)]
    pending: Vec<Loosening>,
    #[serde(skip)]
    errors: Vec<WorkflowError>,
    #[serde(skip)]
    gate_hash: String,
    #[serde(skip)]
    gates_confirmed: bool,
    #[serde(skip)]
    detected: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Rules::new(&UserDefaults::default(), None, &Confirmations::default())
    }
}

impl Rules {
    /// The rules of `defaults` tightened by `workflow` (none when the project has no file), with the loosenings
    /// `confirmed` holds.
    pub fn new(defaults: &UserDefaults, workflow: Option<&Workflow>, confirmed: &Confirmations) -> Rules {
        let empty = Workflow::default();
        let file = workflow.unwrap_or(&empty);
        let mut pending = Vec::new();
        let mut allow = |loosening: Loosening| {
            let ok = confirmed.has(&loosening.hash());
            if !ok {
                pending.push(loosening);
            }
            ok
        };
        let reviewer_rule = match (defaults.reviewer_rule(), file.reviewer_preferred()) {
            (_, Some(false)) => ReviewerRule::Required,
            (ReviewerRule::Required, Some(true)) if allow(Loosening::ReviewerPreferred) => ReviewerRule::Preferred,
            (ReviewerRule::Required, _) => ReviewerRule::Required,
            (ReviewerRule::Preferred, _) => ReviewerRule::Preferred,
        };
        let skip_spec_approval = file.skip_spec_approval() && allow(Loosening::SkipSpecApproval);
        let intake = file
            .intake()
            .filter(|intake| {
                allow(Loosening::IssueIntake {
                    intake: (*intake).clone(),
                })
            })
            .cloned();
        let tools = Role::ALL
            .into_iter()
            .filter(|&role| !file.tools(role).is_empty())
            .filter(|&role| {
                allow(Loosening::Tools {
                    role,
                    tools: file.tools(role).to_vec(),
                })
            })
            .map(|role| (role, file.tools(role).to_vec()))
            .collect();
        let credentials: Vec<String> = file.copy().iter().filter(|p| holds_credentials(p)).cloned().collect();
        let credentials_ok = credentials.is_empty()
            || allow(Loosening::CopyCredentials {
                paths: credentials.clone(),
            });
        let copy = file
            .copy()
            .iter()
            .filter(|p| credentials_ok || !credentials.contains(p))
            .cloned()
            .collect();
        let roles = Role::ALL
            .into_iter()
            .map(|role| {
                let list = file.role(role).unwrap_or_else(|| defaults.role(role));
                (role, list.to_vec())
            })
            .collect();
        let mut rules = Rules {
            limits: defaults.limits().tighten(&file.limits_over(defaults.limits())),
            budgets: defaults.budgets().tighten(file.budgets()),
            roles,
            reviewer_rule,
            skip_spec_approval,
            intake,
            tools,
            gates: file.gates().to_vec(),
            gate_env: file.gate_env().clone(),
            images: file.images().map(str::to_string),
            network: file.network().to_vec(),
            sockets: file.sockets().to_vec(),
            location: file.location().unwrap_or(defaults.location()),
            copy,
            setup: file.setup().map(str::to_string),
            relevance: file.relevance().clone(),
            templates: BTreeMap::new(),
            resume_after_restart: defaults.resume_after_restart(),
            awake_on_battery: defaults.awake_on_battery(),
            pending,
            errors: Vec::new(),
            gate_hash: String::new(),
            gates_confirmed: false,
            detected: false,
        };
        rules.gate_hash = rules.compute_gate_hash();
        rules.gates_confirmed = confirmed.has(&rules.gate_hash);
        rules
    }

    /// The rules of `defaults` for a project whose workflow file is invalid: nothing of the file applies, and no new
    /// station starts until it is fixed (E190).
    pub fn invalid(defaults: &UserDefaults, errors: Vec<WorkflowError>) -> Rules {
        let mut rules = Rules::new(defaults, None, &Confirmations::default());
        rules.errors = errors;
        rules
    }

    /// These rules with the gate commands `ide-test` detected (E110), used when the file names none; detected
    /// commands need the user's confirmation like the file's (E163).
    pub fn with_detected(mut self, commands: Vec<GateCommand>, confirmed: &Confirmations) -> Rules {
        if self.gates.is_empty() && !commands.is_empty() {
            self.gates = commands;
            self.detected = true;
            self.gate_hash = self.compute_gate_hash();
            self.gates_confirmed = confirmed.has(&self.gate_hash);
        }
        self
    }

    /// These rules with the prompt templates read from `.dango/factory/` of the working copy, by file name such as
    /// `review.md` (E161); they count towards the rule set's hash.
    pub fn with_templates(mut self, templates: BTreeMap<String, String>) -> Rules {
        self.templates = templates;
        self
    }

    /// The template for `kind`: the project's file, else the built-in one.
    pub fn template(&self, kind: crate::event::PromptKind) -> &str {
        self.templates
            .get(crate::prompt::template_file(kind))
            .map_or_else(|| crate::prompt::default_template(kind), String::as_str)
    }

    fn compute_gate_hash(&self) -> String {
        hash_json(&GateSet {
            gates: &self.gates,
            env: &self.gate_env,
            setup: self.setup.as_deref(),
            network: &self.network,
            sockets: &self.sockets,
        })
    }

    /// The hash of the whole rule set, part of the evidence's fingerprint (E163).
    pub fn hash(&self) -> String {
        hash_json(self)
    }

    /// The errors of the workflow file; while there are any, no new station starts.
    pub fn errors(&self) -> &[WorkflowError] {
        &self.errors
    }

    /// Loosenings the file asks for that the user has not confirmed; they do not apply.
    pub fn pending(&self) -> &[Loosening] {
        &self.pending
    }

    /// The hash of the gate command set, which the user confirms.
    pub fn gate_hash(&self) -> &str {
        &self.gate_hash
    }

    /// Whether the user confirmed the gate command set.
    pub fn gates_confirmed(&self) -> bool {
        self.gates_confirmed
    }

    /// Marks the gate command set confirmed.
    pub fn confirm_gates(&mut self) {
        self.gates_confirmed = true;
    }

    /// Whether the gate commands come from `ide-test` rather than the file.
    pub fn gates_detected(&self) -> bool {
        self.detected
    }

    /// The limits.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// The budgets.
    pub fn budgets(&self) -> &Budgets {
        &self.budgets
    }

    /// The ranked list of `role`.
    pub fn role(&self, role: Role) -> &[RankEntry] {
        self.roles.get(&role).map_or(&[], Vec::as_slice)
    }

    /// The reviewer rule.
    pub fn reviewer_rule(&self) -> ReviewerRule {
        self.reviewer_rule
    }

    /// Whether G1 may be skipped for orders of class low (E185).
    pub fn skip_spec_approval(&self) -> bool {
        self.skip_spec_approval
    }

    /// Automatic intake from issues.
    pub fn intake(&self) -> Option<&IssueIntake> {
        self.intake.as_ref()
    }

    /// Tools switched on for `role`.
    pub fn tools(&self, role: Role) -> &[String] {
        self.tools.get(&role).map_or(&[], Vec::as_slice)
    }

    /// The gate commands that apply to an order of `class`.
    pub fn gates_for(&self, class: RiskClass) -> Vec<GateCommand> {
        self.gates
            .iter()
            .filter(|gate| gate.classes.is_empty() || gate.classes.contains(&class))
            .cloned()
            .collect()
    }

    /// Whether any gate command is known, from the file or detected.
    pub fn has_gates(&self) -> bool {
        !self.gates.is_empty()
    }

    /// Environment variables for gates.
    pub fn gate_env(&self) -> &BTreeMap<String, String> {
        &self.gate_env
    }

    /// The folder for gate pictures.
    pub fn images(&self) -> Option<&str> {
        self.images.as_deref()
    }

    /// Hosts gates and agents may reach.
    pub fn network(&self) -> &[String] {
        &self.network
    }

    /// Unix sockets gates may reach.
    pub fn sockets(&self) -> &[String] {
        &self.sockets
    }

    /// Where worktrees lie.
    pub fn location(&self) -> WorktreeLocation {
        self.location
    }

    /// Paths copied into a new worktree; credentials only when confirmed.
    pub fn copy(&self) -> &[String] {
        &self.copy
    }

    /// The setup command.
    pub fn setup(&self) -> Option<&str> {
        self.setup.as_deref()
    }

    /// The classifier for relevant changes and the measured risk class.
    pub fn relevance(&self) -> Result<Relevance, PatternError> {
        Relevance::new(&self.relevance)
    }

    /// Whether orders resume by themselves after a restart.
    pub fn resume_after_restart(&self) -> bool {
        self.resume_after_restart
    }

    /// Whether running orders keep the Mac awake on battery.
    pub fn awake_on_battery(&self) -> bool {
        self.awake_on_battery
    }
}
