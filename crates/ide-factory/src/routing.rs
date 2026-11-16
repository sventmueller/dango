//! Roles and routing (E162, E170, E171): each role has a ranked list of agent, model and effort; the first entry that
//! may take the role, is ready, has quota and lies within budget gets it. The vendor counts by model, not by program,
//! and an unknown vendor never counts as a change of vendor.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::quota::{QuotaBook, QuotaState};
use crate::station::{Role, Stage};

/// A path to an agent (Weg), as Dango connects to it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum Agent {
    /// Claude Code over its SDK protocol.
    Claude,
    /// Codex over its app server.
    Codex,
    /// OpenCode over its server.
    OpenCode,
    /// Antigravity over ACP.
    Antigravity,
    /// Dango's own agent.
    Own,
    /// Another ACP agent, by its registry name.
    Acp(String),
}

impl From<String> for Agent {
    fn from(name: String) -> Agent {
        match name.as_str() {
            "claude" => Agent::Claude,
            "codex" => Agent::Codex,
            "opencode" => Agent::OpenCode,
            "antigravity" => Agent::Antigravity,
            "own" => Agent::Own,
            _ => Agent::Acp(name),
        }
    }
}

impl From<Agent> for String {
    fn from(agent: Agent) -> String {
        agent.to_string()
    }
}

impl fmt::Display for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::OpenCode => "opencode",
            Agent::Antigravity => "antigravity",
            Agent::Own => "own",
            Agent::Acp(name) => name,
        })
    }
}

/// The vendor of a model (E171).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vendor {
    /// Anthropic.
    Anthropic,
    /// OpenAI.
    OpenAi,
    /// Google.
    Google,
    /// Mistral AI.
    Mistral,
    /// Meta.
    Meta,
    /// DeepSeek.
    DeepSeek,
    /// Alibaba (Qwen).
    Alibaba,
    /// xAI.
    XAi,
    /// Moonshot AI.
    Moonshot,
    /// Zhipu AI.
    Zhipu,
    /// Not in the table; never counts as a change of vendor.
    Unknown,
}

/// Prefixes of model ids or provider names and their vendor, matched on each `/`-separated part of an id.
const VENDOR_PREFIXES: &[(&str, Vendor)] = &[
    ("anthropic", Vendor::Anthropic),
    ("claude", Vendor::Anthropic),
    ("openai", Vendor::OpenAi),
    ("gpt", Vendor::OpenAi),
    ("o1", Vendor::OpenAi),
    ("o3", Vendor::OpenAi),
    ("o4", Vendor::OpenAi),
    ("codex", Vendor::OpenAi),
    ("google", Vendor::Google),
    ("gemini", Vendor::Google),
    ("gemma", Vendor::Google),
    ("vertex", Vendor::Google),
    ("mistral", Vendor::Mistral),
    ("codestral", Vendor::Mistral),
    ("devstral", Vendor::Mistral),
    ("magistral", Vendor::Mistral),
    ("meta", Vendor::Meta),
    ("llama", Vendor::Meta),
    ("deepseek", Vendor::DeepSeek),
    ("qwen", Vendor::Alibaba),
    ("alibaba", Vendor::Alibaba),
    ("grok", Vendor::XAi),
    ("x-ai", Vendor::XAi),
    ("xai", Vendor::XAi),
    ("kimi", Vendor::Moonshot),
    ("moonshot", Vendor::Moonshot),
    ("glm", Vendor::Zhipu),
    ("z-ai", Vendor::Zhipu),
    ("zhipu", Vendor::Zhipu),
];

/// The vendor of `model` by the prefix table; ids like `openrouter/anthropic/claude-x` are read part by part.
pub fn vendor_of_model(model: &str) -> Vendor {
    model
        .split('/')
        .map(str::to_ascii_lowercase)
        .find_map(|part| {
            VENDOR_PREFIXES
                .iter()
                .find(|(prefix, _)| {
                    part.strip_prefix(prefix)
                        .is_some_and(|rest| rest.is_empty() || !rest.starts_with(|c: char| c.is_ascii_alphabetic()))
                })
                .map(|&(_, vendor)| vendor)
        })
        .unwrap_or(Vendor::Unknown)
}

/// The vendor behind `agent` running `model`: fixed for the vendors' own programs, by model otherwise (E171).
pub fn vendor_of(agent: &Agent, model: Option<&str>) -> Vendor {
    match agent {
        Agent::Claude => Vendor::Anthropic,
        Agent::Codex => Vendor::OpenAi,
        Agent::Antigravity => Vendor::Google,
        Agent::OpenCode | Agent::Own | Agent::Acp(_) => model.map_or(Vendor::Unknown, vendor_of_model),
    }
}

/// Whether `a` and `b` are different vendors; unknown is never different.
pub fn vendors_differ(a: Vendor, b: Vendor) -> bool {
    a != Vendor::Unknown && b != Vendor::Unknown && a != b
}

/// One entry of a role's ranked list.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankEntry {
    /// The agent.
    pub agent: Agent,
    /// The model; `None` is the agent's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The reasoning effort; `None` is the agent's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

impl RankEntry {
    /// An entry for `agent` with its default model and effort.
    pub fn new(agent: Agent) -> RankEntry {
        RankEntry {
            agent,
            model: None,
            effort: None,
        }
    }

    /// This entry with `model`.
    pub fn model(mut self, model: &str) -> RankEntry {
        self.model = Some(model.to_string());
        self
    }

    /// The vendor of this entry.
    pub fn vendor(&self) -> Vendor {
        vendor_of(&self.agent, self.model.as_deref())
    }
}

/// What a path can do in an order, per E173's table; the app may feed what it found out per path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// It can build: writing is confined to the worktree by a sandbox.
    pub builds: bool,
    /// It can take a read-only role: writing is blocked technically (C48).
    pub reads_only: bool,
    /// It reports subagents and background work reliably; if not, they are switched off in orders (E189).
    pub reports_subagents: bool,
    /// It can resume a session after its process ended (A15).
    pub resumes: bool,
    /// It takes a ceiling in dollars at its start (C64).
    pub takes_budget: bool,
}

impl Capabilities {
    /// What E173 says about `agent`. OpenCode and Antigravity take no role until a confined profile is enforced and
    /// tested (C48, C72); unknown ACP agents neither.
    pub fn of(agent: &Agent) -> Capabilities {
        let none = Capabilities {
            builds: false,
            reads_only: false,
            reports_subagents: false,
            resumes: false,
            takes_budget: false,
        };
        match agent {
            Agent::Claude => Capabilities {
                builds: true,
                reads_only: true,
                reports_subagents: true,
                resumes: true,
                takes_budget: true,
            },
            Agent::Codex => Capabilities {
                builds: true,
                reads_only: true,
                reports_subagents: false,
                resumes: true,
                takes_budget: false,
            },
            Agent::Own => Capabilities {
                builds: true,
                reads_only: true,
                reports_subagents: true,
                resumes: true,
                takes_budget: false,
            },
            Agent::OpenCode | Agent::Antigravity | Agent::Acp(_) => none,
        }
    }

    /// Whether the path may take `role`.
    pub fn takes(&self, role: Role) -> bool {
        if role.reads_only() {
            self.reads_only
        } else {
            self.builds
        }
    }
}

/// Whether a path can start now (E58).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "readiness", content = "detail")]
pub enum Readiness {
    /// Installed and signed in as far as known.
    Ready,
    /// Not installed.
    NotInstalled,
    /// Signing in failed.
    SignInFailed(String),
    /// Starting failed.
    StartFailed(String),
}

/// What the app knows about one path: whether it is ready, which account and billing service it uses, and what it
/// can do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentStatus {
    /// Whether it can start.
    pub readiness: Readiness,
    /// The account its quota hangs on.
    pub account: Option<String>,
    /// The billing service, such as `claude-subscription` or `openai-api`.
    pub service: Option<String>,
    /// What it can do in orders.
    pub capabilities: Capabilities,
    /// The vendor of the model it runs when an entry names none, where that depends on its settings rather than its
    /// program, such as the own agent's endpoint and model (E171).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<Vendor>,
}

impl AgentStatus {
    /// A ready path with E173's capabilities and no known account.
    pub fn ready(agent: &Agent) -> AgentStatus {
        AgentStatus {
            readiness: Readiness::Ready,
            account: None,
            service: None,
            capabilities: Capabilities::of(agent),
            vendor: None,
        }
    }

    /// The vendor behind `entry` on this path: by its program or model, else the path's own model's.
    pub fn vendor_of(&self, entry: &RankEntry) -> Vendor {
        match entry.vendor() {
            Vendor::Unknown if entry.model.is_none() => self.vendor.unwrap_or(Vendor::Unknown),
            vendor => vendor,
        }
    }
}

/// The stage orders created now run in, up to `highest`: from stage 2 on only while an entry of `builders` and one of
/// `reviewers` of another vendor are ready and may take their roles (E162, E170); otherwise stage 1, with the user's
/// text as the specification and no reviewer agent.
pub fn stage_for(
    agents: &BTreeMap<Agent, AgentStatus>,
    builders: &[RankEntry],
    reviewers: &[RankEntry],
    highest: Stage,
) -> Stage {
    if highest == Stage::One {
        return highest;
    }
    let ready = |role: Role, list: &[RankEntry]| -> Vec<Vendor> {
        list.iter()
            .filter_map(|entry| {
                let status = agents.get(&entry.agent)?;
                (status.readiness == Readiness::Ready && status.capabilities.takes(role))
                    .then(|| status.vendor_of(entry))
            })
            .collect()
    };
    let reviewing = ready(Role::Reviewer, reviewers);
    let two = ready(Role::Builder, builders)
        .into_iter()
        .any(|builder| reviewing.iter().any(|&reviewer| vendors_differ(builder, reviewer)));
    if two { highest } else { Stage::One }
}

/// Whether the reviewer must come from another vendor than every builder (E162, E183).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerRule {
    /// Another vendor is required; without one the order waits for the user.
    #[default]
    Required,
    /// Another vendor is preferred; without one the same vendor reviews and the evidence says so.
    Preferred,
}

/// Why routing passed over an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "why")]
pub enum Skip {
    /// The path may not take the role (E173).
    CannotTakeRole,
    /// The path is not ready, with the app's reason.
    NotReady {
        /// Why.
        readiness: Readiness,
    },
    /// The quota is at or above the start limit, or refused.
    Quota {
        /// What is known.
        state: QuotaState,
    },
    /// A budget of the path's service is reached.
    Budget,
    /// The reviewer would be of a builder's vendor.
    SameVendor,
    /// It already failed in this station run.
    Excluded,
}

/// The entry routing chose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    /// Its place in the list.
    pub index: usize,
    /// The entry.
    pub entry: RankEntry,
    /// Its vendor.
    pub vendor: Vendor,
    /// Its quota when chosen; unknown is named in the evidence.
    pub quota: QuotaState,
    /// The billing service it runs on.
    pub service: Option<String>,
    /// The account its quota hangs on.
    pub account: Option<String>,
}

/// What routing decided for a role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    /// The chosen entry, if any.
    pub choice: Option<Choice>,
    /// Each entry passed over, by its place, with why.
    pub skipped: Vec<(usize, Skip)>,
    /// The reviewer is of a builder's vendor, allowed by the rule "preferred" (E162).
    pub same_vendor: bool,
}

/// What routing looks at besides the list.
pub struct RouteContext<'a> {
    /// The paths the app knows; a path it does not list is not ready.
    pub agents: &'a BTreeMap<Agent, AgentStatus>,
    /// The quotas.
    pub quotas: &'a QuotaBook,
    /// Wall-clock now, for the age of quotas.
    pub now_wall: u64,
    /// The share from 0 to 1 at which no station starts.
    pub start_limit: f64,
    /// Whether a budget of the given billing service, or of the order or day, is reached.
    pub budget_reached: &'a dyn Fn(Option<&str>) -> bool,
    /// Entries that already failed in this station run.
    pub excluded: &'a [usize],
    /// The vendors that built in this order, for the reviewer.
    pub builders: &'a [Vendor],
    /// The reviewer rule.
    pub rule: ReviewerRule,
}

/// The first entry of `list` for `role` that may take it, is ready, has quota and budget (E170); for the reviewer
/// first among other vendors, then, under the rule "preferred", among all.
pub fn route(role: Role, list: &[RankEntry], cx: &RouteContext<'_>) -> Route {
    let other_vendor = role == Role::Reviewer;
    let first = pick(role, list, cx, other_vendor);
    if first.choice.is_some() || !other_vendor || cx.rule == ReviewerRule::Required {
        return first;
    }
    let mut second = pick(role, list, cx, false);
    second.same_vendor = second.choice.is_some();
    if second.choice.is_none() {
        second.skipped = first.skipped;
    }
    second
}

fn pick(role: Role, list: &[RankEntry], cx: &RouteContext<'_>, other_vendor: bool) -> Route {
    let mut skipped = Vec::new();
    for (index, entry) in list.iter().enumerate() {
        match judge(role, index, entry, cx, other_vendor) {
            Ok(choice) => {
                return Route {
                    choice: Some(choice),
                    skipped,
                    same_vendor: false,
                };
            }
            Err(skip) => skipped.push((index, skip)),
        }
    }
    Route {
        choice: None,
        skipped,
        same_vendor: false,
    }
}

fn judge(
    role: Role,
    index: usize,
    entry: &RankEntry,
    cx: &RouteContext<'_>,
    other_vendor: bool,
) -> Result<Choice, Skip> {
    if cx.excluded.contains(&index) {
        return Err(Skip::Excluded);
    }
    let status = cx.agents.get(&entry.agent).cloned().unwrap_or(AgentStatus {
        readiness: Readiness::NotInstalled,
        ..AgentStatus::ready(&entry.agent)
    });
    if !status.capabilities.takes(role) {
        return Err(Skip::CannotTakeRole);
    }
    if status.readiness != Readiness::Ready {
        return Err(Skip::NotReady {
            readiness: status.readiness,
        });
    }
    let vendor = status.vendor_of(entry);
    if other_vendor && !cx.builders.iter().all(|&built| vendors_differ(vendor, built)) {
        return Err(Skip::SameVendor);
    }
    let quota = match (&status.account, &status.service) {
        (Some(account), Some(service)) => cx.quotas.state(account, service, cx.now_wall),
        _ => QuotaState::Unknown,
    };
    if !quota.allows_start(cx.start_limit) {
        return Err(Skip::Quota { state: quota });
    }
    if (cx.budget_reached)(status.service.as_deref()) {
        return Err(Skip::Budget);
    }
    Ok(Choice {
        index,
        entry: entry.clone(),
        vendor,
        quota,
        service: status.service,
        account: status.account,
    })
}

#[cfg(test)]
mod tests;
