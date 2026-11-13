//! The control protocol on the same stream as the messages: requests either side sends, the one answer each gets,
//! and the cancel that withdraws one.

use std::collections::HashMap;
use std::time::Duration;

use ide_thread::{Decision, Update, UsagePatch};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::modes::{Effort, PermissionMode};
use crate::protocol::{ModelUsage, typed};

mod error;

pub use error::ControlError;

/// A request the app sends to the CLI.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Starts the session's protocol; the answer lists commands, models and the account.
    Initialize {
        /// Whether the app offers a stop control per background task, so that an interrupt spares them.
        per_task_stop: bool,
        /// Whether subagents report a one-line summary of their progress.
        progress_summaries: bool,
        /// Whether the text and thinking of subagents are forwarded, not only their tool calls.
        forward_subagent_text: bool,
    },
    /// Stops the current turn.
    Interrupt,
    /// Changes the permission mode.
    SetPermissionMode(PermissionMode),
    /// Changes the model for the following turns; `None` returns to the default.
    SetModel(Option<String>),
    /// Changes the reasoning effort of the running session.
    SetEffort(Effort),
    /// Stops one task.
    StopTask(String),
    /// Moves the foreground task started by this tool call, or all of them, to the background.
    BackgroundTasks(Option<String>),
    /// Reads the end of a shell or monitor task's output.
    GetTaskOutput(String),
    /// Reads how full the context window is, by category; `full` counts each category with the token-count API
    /// instead of estimating from the last response.
    GetContextUsage {
        /// Whether every category is counted exactly.
        full: bool,
    },
    /// Reads the session's cost and the plan's usage windows; experimental in the CLI.
    GetUsage,
    /// Restores the files the file tools changed since a user message; experimental in the CLI.
    RewindFiles {
        /// The user message.
        user_message_id: String,
        /// Only report what would change.
        dry_run: bool,
    },
    /// Connects the session to Remote Control on claude.ai, or ends that; internal in the CLI (`docs/claude-mapping.md`).
    RemoteControl(bool),
}

impl Request {
    /// The request's `subtype`.
    pub fn subtype(&self) -> &'static str {
        match self {
            Request::Initialize { .. } => "initialize",
            Request::Interrupt => "interrupt",
            Request::SetPermissionMode(_) => "set_permission_mode",
            Request::SetModel(_) => "set_model",
            Request::SetEffort(_) => "apply_flag_settings",
            Request::StopTask(_) => "stop_task",
            Request::BackgroundTasks(_) => "background_tasks",
            Request::GetTaskOutput(_) => "get_task_output",
            Request::GetContextUsage { .. } => "get_context_usage",
            Request::GetUsage => "get_usage",
            Request::RewindFiles { .. } => "rewind_files",
            Request::RemoteControl(_) => "remote_control",
        }
    }

    /// How long the app waits for the answer.
    pub fn timeout(&self) -> Duration {
        match self {
            Request::Initialize { .. } => Duration::from_secs(60),
            Request::GetContextUsage { .. }
            | Request::GetUsage
            | Request::RewindFiles { .. }
            | Request::RemoteControl(_) => Duration::from_secs(30),
            _ => Duration::from_secs(10),
        }
    }

    /// The request's body.
    pub fn body(&self) -> Value {
        let mut body = match self {
            Request::Initialize {
                per_task_stop,
                progress_summaries,
                forward_subagent_text,
            } => json!({
                "perTaskStopAffordance": per_task_stop,
                "agentProgressSummaries": progress_summaries,
                "forwardSubagentText": forward_subagent_text,
                "hooks": crate::hooks::registered(),
            }),
            Request::Interrupt | Request::BackgroundTasks(None) => json!({}),
            // The app reads no `behaviors`, whose scan reads every transcript of the last seven days.
            Request::GetUsage => json!({ "skip_behaviors": true }),
            Request::SetPermissionMode(mode) => json!({ "mode": mode.as_str() }),
            Request::SetModel(model) => json!({ "model": model }),
            Request::SetEffort(level) => json!({ "settings": { "effortLevel": level.as_str() } }),
            Request::StopTask(task) | Request::GetTaskOutput(task) => json!({ "task_id": task }),
            Request::BackgroundTasks(Some(call)) => json!({ "tool_use_id": call }),
            Request::GetContextUsage { full } => json!({ "detail": if *full { "full" } else { "summary" } }),
            Request::RewindFiles {
                user_message_id,
                dry_run,
            } => json!({ "user_message_id": user_message_id, "dry_run": dry_run }),
            Request::RemoteControl(enabled) => json!({ "enabled": enabled }),
        };
        if let Value::Object(map) = &mut body {
            map.insert("subtype".into(), Value::from(self.subtype()));
        }
        body
    }

    /// The frame that sends it.
    pub fn frame(&self, request_id: &str) -> Value {
        json!({ "type": "control_request", "request_id": request_id, "request": self.body() })
    }
}

/// The frame that withdraws a request the app sent.
pub fn cancel_frame(request_id: &str) -> Value {
    json!({ "type": "control_cancel_request", "request_id": request_id })
}

/// The frame that answers one of the CLI's requests with `response`.
pub fn success_frame(request_id: &str, response: Value) -> Value {
    json!({ "type": "control_response", "response": { "subtype": "success", "request_id": request_id, "response": response } })
}

/// The frame that refuses one of the CLI's requests.
pub fn error_frame(request_id: &str, error: &str) -> Value {
    json!({ "type": "control_response", "response": { "subtype": "error", "request_id": request_id, "error": error } })
}

/// What `initialize` answers.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct InitializeResult {
    /// The slash commands.
    pub commands: Vec<CommandInfo>,
    /// The models the session can use.
    pub models: Vec<ModelInfo>,
    /// The signed-in account.
    pub account: Account,
    /// The output style.
    pub output_style: String,
    /// The CLI's version.
    pub claude_code_version: Option<String>,
    /// Whether the account and the organisation allow Remote Control; older CLIs leave it out.
    pub remote_control_available: Option<bool>,
    /// Whether the user's settings (`remoteControlAtStartup`) connect each session to Remote Control at start.
    pub remote_control_auto_enable: Option<bool>,
}

impl InitializeResult {
    /// Whether Remote Control can be offered at all.
    pub fn remote_control_offered(&self) -> bool {
        self.remote_control_available == Some(true)
    }

    /// Whether the session connects to Remote Control as it starts, unless the user decided otherwise for the thread.
    pub fn remote_control_at_start(&self) -> bool {
        self.remote_control_offered() && self.remote_control_auto_enable == Some(true)
    }

    /// Whether the CLI has no sign-in at all: no subscription token and no API key on Anthropic's own API.
    pub fn signed_out(&self) -> bool {
        self.first_party_without_key() && self.account.token_source.as_deref() == Some("none")
    }

    /// Whether the CLI runs on a subscription, signed in to Anthropic's own API without an API key; the cost it
    /// reports is then not what the user pays (E57).
    pub fn on_subscription(&self) -> bool {
        let plan = self.account.subscription_type.as_deref().is_some_and(|p| !p.is_empty());
        plan && self.first_party_without_key() && !self.signed_out()
    }

    fn first_party_without_key(&self) -> bool {
        let first_party = matches!(self.account.api_provider.as_deref(), None | Some("firstParty"));
        let no_key = matches!(self.account.api_key_source.as_deref(), None | Some("none"));
        first_party && no_key
    }

    /// The thread's update that says whether the session runs on a subscription.
    pub fn billing(&self) -> Update {
        Update::Usage(UsagePatch {
            subscription: Some(self.on_subscription()),
            ..UsagePatch::default()
        })
    }
}

/// A slash command.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CommandInfo {
    /// Its name without the slash.
    pub name: String,
    /// What it does.
    pub description: String,
    /// What it takes.
    pub argument_hint: String,
}

/// A model the session can use.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelInfo {
    /// What `set_model` takes.
    pub value: String,
    /// What the user reads.
    pub display_name: String,
    /// What it is good for.
    pub description: String,
    /// The effort levels it supports.
    pub supported_effort_levels: Vec<String>,
}

/// The signed-in account; the app shows it and never stores it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Account {
    /// The account's e-mail address.
    pub email: Option<String>,
    /// The organisation.
    pub organization: Option<String>,
    /// The subscription, such as `max`.
    pub subscription_type: Option<String>,
    /// Where the token came from; `none` when there is none.
    pub token_source: Option<String>,
    /// Where an API key came from.
    pub api_key_source: Option<String>,
    /// The API the CLI talks to, such as `firstParty` or `bedrock`.
    pub api_provider: Option<String>,
}

/// What `interrupt` answers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct InterruptResult {
    /// Messages that survive the interrupt and will still run.
    pub still_queued: Vec<String>,
}

/// What `get_task_output` answers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TaskOutput {
    /// The end of the output.
    pub output: String,
    /// The size of the whole output.
    pub total_bytes: u64,
    /// Whether `output` is only the end.
    pub truncated: bool,
}

/// What `get_context_usage` answers.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextUsage {
    /// The rows of the breakdown.
    pub categories: Vec<ContextCategory>,
    /// Tokens in use.
    pub total_tokens: u64,
    /// The window measured against.
    pub raw_max_tokens: u64,
    /// The share in use, in percent.
    pub percentage: f64,
    /// The model.
    pub model: String,
    /// Where compacting by itself starts, in tokens.
    pub auto_compact_threshold: Option<f64>,
    /// Whether the conversation is compacted by itself.
    pub is_auto_compact_enabled: bool,
    /// The memory files, such as `CLAUDE.md`.
    pub memory_files: Vec<ContextFile>,
    /// The tools of MCP servers.
    pub mcp_tools: Vec<ContextMcpTool>,
    /// The custom agents.
    pub agents: Vec<ContextAgent>,
    /// The skills.
    pub skills: Option<ContextSkills>,
    /// The built-in tools.
    pub system_tools: Option<Vec<ContextNamed>>,
    /// The conversation by kind of message.
    pub message_breakdown: Option<ContextMessages>,
}

/// A memory file in the context.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ContextFile {
    /// Where it is.
    pub path: String,
    /// Its tokens.
    pub tokens: u64,
}

/// An MCP tool in the context.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextMcpTool {
    /// The tool.
    pub name: String,
    /// Its server.
    pub server_name: String,
    /// Its tokens.
    pub tokens: u64,
}

/// A custom agent in the context.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextAgent {
    /// The agent's type.
    pub agent_type: String,
    /// Its tokens.
    pub tokens: u64,
}

/// The skills in the context.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextSkills {
    /// Their tokens together.
    pub tokens: u64,
    /// Each skill.
    pub skill_frontmatter: Vec<ContextNamed>,
}

/// Something in the context with a name.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ContextNamed {
    /// Its name.
    pub name: String,
    /// Its tokens.
    pub tokens: u64,
}

/// The conversation in the context by kind of message.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextMessages {
    /// Tool calls.
    pub tool_call_tokens: u64,
    /// Tool results.
    pub tool_result_tokens: u64,
    /// Attachments.
    pub attachment_tokens: u64,
    /// The assistant's messages.
    pub assistant_message_tokens: u64,
    /// The user's messages.
    pub user_message_tokens: u64,
}

/// One row of the context breakdown.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ContextCategory {
    /// The stable id of the row, when the CLI sends one.
    pub id: Option<String>,
    /// The row's name as the CLI shows it.
    pub name: String,
    /// Its tokens.
    pub tokens: u64,
    /// `used`, `free`, `buffer` or `deferred`.
    pub kind: String,
}

/// What `get_usage` answers, as far as the app reads it; the CLI marks the shape experimental.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UsageReport {
    /// The session's cost so far, in US dollars.
    pub session_cost_usd: Option<f64>,
    /// The session's totals so far by model.
    pub session_models: HashMap<String, ModelUsage>,
    /// The subscription, such as `max`; none with an API key.
    pub subscription: Option<String>,
    /// The plan's usage windows.
    pub windows: Vec<UsageWindow>,
    /// The whole answer.
    pub raw: Value,
}

/// One usage window of the plan.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UsageWindow {
    /// The CLI's name for it, such as `five_hour`.
    pub name: String,
    /// The share used, in percent.
    pub utilization: Option<f64>,
    /// When it resets, as ISO 8601.
    pub resets_at: Option<String>,
}

impl UsageReport {
    fn read(value: Value) -> UsageReport {
        let windows = value
            .get("rate_limits")
            .and_then(Value::as_object)
            .map(|limits| {
                limits
                    .iter()
                    .filter_map(|(name, window)| {
                        let window = window.as_object()?;
                        Some(UsageWindow {
                            name: name.clone(),
                            utilization: window.get("utilization").and_then(Value::as_f64),
                            resets_at: window.get("resets_at").and_then(Value::as_str).map(str::to_string),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        UsageReport {
            session_cost_usd: value.pointer("/session/total_cost_usd").and_then(Value::as_f64),
            session_models: value
                .pointer("/session/model_usage")
                .and_then(|models| HashMap::<String, ModelUsage>::deserialize(models).ok())
                .unwrap_or_default(),
            subscription: value
                .get("subscription_type")
                .and_then(Value::as_str)
                .map(str::to_string),
            windows,
            raw: value,
        }
    }
}

/// What `rewind_files` answers; experimental in the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RewindResult {
    /// Whether the files can be or were restored.
    pub can_rewind: bool,
    /// Why not.
    pub error: Option<String>,
    /// The files that change.
    pub files_changed: Vec<String>,
    /// Lines added back.
    pub insertions: u64,
    /// Lines removed.
    pub deletions: u64,
}

/// What `remote_control` answers when it connects.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct RemoteControlResult {
    /// The session's page on claude.ai.
    pub session_url: Option<String>,
    /// Which connection of the session later `bridge_state` frames speak of.
    pub bridge_epoch: Option<u64>,
}

/// Reads a success payload as the answer of a request type.
pub trait Answer: Sized + Send + 'static {
    /// The answer in `payload`, which is `{}` when the CLI sent none.
    fn read(payload: Value) -> Result<Self, ControlError>;
}

impl Answer for () {
    fn read(_: Value) -> Result<(), ControlError> {
        Ok(())
    }
}

impl Answer for Value {
    fn read(payload: Value) -> Result<Value, ControlError> {
        Ok(payload)
    }
}

impl Answer for bool {
    fn read(payload: Value) -> Result<bool, ControlError> {
        Ok(payload.get("backgrounded").and_then(Value::as_bool).unwrap_or(true))
    }
}

impl Answer for UsageReport {
    fn read(payload: Value) -> Result<UsageReport, ControlError> {
        Ok(UsageReport::read(payload))
    }
}

macro_rules! answer_by_serde {
    ($($t:ty),*) => {$(
        impl Answer for $t {
            fn read(payload: Value) -> Result<$t, ControlError> {
                <$t>::deserialize(&payload).map_err(|e| ControlError::Unexpected(e.to_string()))
            }
        }
    )*};
}

answer_by_serde!(
    InitializeResult,
    InterruptResult,
    TaskOutput,
    ContextUsage,
    RewindResult,
    RemoteControlResult
);

/// A request from the CLI.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// The CLI asks whether a tool may run.
    CanUseTool(Box<PermissionAsk>),
    /// The CLI runs one of the hooks the app registered.
    HookCallback {
        /// The hook's id from `initialize`.
        callback_id: String,
        /// What the hook is about, such as the tool call for `PreToolUse`.
        input: Value,
    },
    /// Any other request, which the app refuses.
    Other {
        /// Its subtype.
        subtype: String,
    },
}

impl Incoming {
    /// The request in `body`.
    pub fn read(body: &Value) -> Incoming {
        let subtype = body.get("subtype").and_then(Value::as_str).unwrap_or_default();
        match subtype {
            "can_use_tool" => typed(body).map_or_else(
                || Incoming::Other {
                    subtype: subtype.to_string(),
                },
                Incoming::CanUseTool,
            ),
            "hook_callback" => Incoming::HookCallback {
                callback_id: body
                    .get("callback_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                input: body.get("input").cloned().unwrap_or(Value::Null),
            },
            _ => Incoming::Other {
                subtype: subtype.to_string(),
            },
        }
    }
}

/// The CLI asks whether a tool may run.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct PermissionAsk {
    /// The tool.
    pub tool_name: String,
    /// The input it would run with.
    pub input: Value,
    /// Rules or mode changes the CLI suggests for "always allow".
    pub permission_suggestions: Vec<Value>,
    /// The path that made it ask.
    pub blocked_path: Option<String>,
    /// Why it asks.
    pub decision_reason: Option<String>,
    /// A headline for the dialog.
    pub title: Option<String>,
    /// A short name for the tool.
    pub display_name: Option<String>,
    /// What the call does.
    pub description: Option<String>,
    /// The tool call.
    pub tool_use_id: String,
    /// The subagent that wants to call it.
    pub agent_id: Option<String>,
    /// Whether no "always allow" may be offered.
    pub suppress_always_allow_rule: bool,
    /// Whether allowing must not be preselected.
    pub default_to_no: bool,
}

/// One suggestion of the CLI for answering "always allow", as the user reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// What it allows.
    pub rule: String,
    /// Where it is stored: `session`, `localSettings`, `projectSettings` or `userSettings`.
    pub scope: String,
}

impl Suggestion {
    /// The suggestion `raw` describes, if it allows something.
    pub fn read(raw: &Value) -> Option<Suggestion> {
        let scope = raw
            .get("destination")
            .and_then(Value::as_str)
            .unwrap_or("session")
            .to_string();
        let rule = match raw.get("type").and_then(Value::as_str)? {
            "addRules" | "replaceRules" => {
                if raw.get("behavior").and_then(Value::as_str) != Some("allow") {
                    return None;
                }
                let rules: Vec<String> = raw
                    .get("rules")?
                    .as_array()?
                    .iter()
                    .filter_map(|rule| {
                        let tool = rule.get("toolName")?.as_str()?;
                        Some(match rule.get("ruleContent").and_then(Value::as_str) {
                            Some(content) => format!("{tool}({content})"),
                            None => tool.to_string(),
                        })
                    })
                    .collect();
                if rules.is_empty() {
                    return None;
                }
                rules.join(", ")
            }
            "setMode" => format!("mode {}", raw.get("mode")?.as_str()?),
            "addDirectories" => {
                let dirs: Vec<&str> = raw
                    .get("directories")?
                    .as_array()?
                    .iter()
                    .filter_map(Value::as_str)
                    .collect();
                format!("directories {}", dirs.join(", "))
            }
            _ => return None,
        };
        Some(Suggestion { rule, scope })
    }
}

/// The app's answer to a permission request.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionAnswer {
    /// The tool may run.
    Allow {
        /// The input it runs with, when the user changed it.
        updated_input: Option<Value>,
        /// The suggestions to store, by their position in the request.
        always: Vec<usize>,
    },
    /// The tool may not run.
    Deny {
        /// What the model is told.
        message: String,
        /// Whether the turn stops as well.
        interrupt: bool,
    },
}

impl PermissionAnswer {
    /// The success payload answering `ask`.
    pub fn payload(&self, ask: &PermissionAsk) -> Value {
        let mut payload = Map::new();
        match self {
            PermissionAnswer::Allow { updated_input, always } => {
                payload.insert("behavior".into(), Value::from("allow"));
                // The CLI runs the tool with exactly this input, so the original goes back when nothing changed.
                payload.insert(
                    "updatedInput".into(),
                    updated_input.clone().unwrap_or_else(|| ask.input.clone()),
                );
                let rules: Vec<Value> = always
                    .iter()
                    .filter_map(|n| ask.permission_suggestions.get(*n).cloned())
                    .collect();
                let classification = if rules.is_empty() {
                    "user_temporary"
                } else {
                    payload.insert("updatedPermissions".into(), Value::Array(rules));
                    "user_permanent"
                };
                payload.insert("decisionClassification".into(), Value::from(classification));
            }
            PermissionAnswer::Deny { message, interrupt } => {
                payload.insert("behavior".into(), Value::from("deny"));
                payload.insert("message".into(), Value::from(message.as_str()));
                if *interrupt {
                    payload.insert("interrupt".into(), Value::Bool(true));
                }
                payload.insert("decisionClassification".into(), Value::from("user_reject"));
            }
        }
        payload.insert("toolUseID".into(), Value::from(ask.tool_use_id.as_str()));
        Value::Object(payload)
    }
}

/// What the model is told when the user sends a plan back without saying what to change.
pub const PLAN_REVISE_MESSAGE: &str = "The user wants the plan changed.";

/// What the model is told when the user rejects a plan.
pub const PLAN_REJECT_MESSAGE: &str =
    "The user rejected the plan. Do not carry it out; wait for the user to say how to go on.";

/// The app's answer to a plan the agent asks to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanAnswer {
    /// Leave plan mode and carry the plan out.
    Approve,
    /// Go on planning; what the user wants changed, when they said.
    Revise(Option<String>),
    /// Do not carry the plan out, and end the turn.
    Reject,
}

impl PlanAnswer {
    /// The permission answer that says this.
    pub fn permission(&self) -> PermissionAnswer {
        match self {
            PlanAnswer::Approve => PermissionAnswer::Allow {
                updated_input: None,
                always: Vec::new(),
            },
            PlanAnswer::Revise(feedback) => PermissionAnswer::Deny {
                message: feedback.clone().unwrap_or_else(|| PLAN_REVISE_MESSAGE.into()),
                interrupt: false,
            },
            PlanAnswer::Reject => PermissionAnswer::Deny {
                message: PLAN_REJECT_MESSAGE.into(),
                interrupt: true,
            },
        }
    }

    /// The decision the thread records.
    pub fn decision(&self) -> Decision {
        match self {
            PlanAnswer::Approve => Decision::Approved,
            PlanAnswer::Revise(feedback) => plan_rejection(feedback.as_deref(), false),
            PlanAnswer::Reject => plan_rejection(None, true),
        }
    }
}

/// A plan the user did not accept, told to the model as `message`; the app's own fixed messages are no feedback.
pub fn plan_rejection(message: Option<&str>, stop: bool) -> Decision {
    let feedback = message
        .map(str::trim)
        .filter(|m| !m.is_empty() && *m != PLAN_REVISE_MESSAGE && !m.contains(PLAN_REJECT_MESSAGE))
        .map(str::to_string);
    Decision::Rejected { feedback, stop }
}

/// The input that answers an AskUserQuestion call: its questions with `answers`, several labels of one question
/// joined by a comma as the official extension does.
pub fn question_answer(input: &Value, answers: &[(String, String)]) -> Value {
    let mut updated = input.as_object().cloned().unwrap_or_default();
    let map: Map<String, Value> = answers
        .iter()
        .map(|(question, answer)| (question.clone(), Value::from(answer.as_str())))
        .collect();
    updated.insert("answers".into(), Value::Object(map));
    Value::Object(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(suggestions: Value) -> PermissionAsk {
        PermissionAsk::deserialize(&json!({
            "subtype": "can_use_tool",
            "tool_name": "Bash",
            "input": {"command": "touch marker.txt"},
            "permission_suggestions": suggestions,
            "tool_use_id": "toolu_1",
            "future_field": {"ignored": true}
        }))
        .unwrap()
    }

    #[test]
    fn a_request_frame_carries_its_id_and_subtype() {
        let frame = Request::StopTask("b1".into()).frame("req-7");
        assert_eq!(
            frame,
            json!({"type": "control_request", "request_id": "req-7", "request": {"subtype": "stop_task", "task_id": "b1"}})
        );
        assert_eq!(
            Request::SetModel(None).body(),
            json!({"subtype": "set_model", "model": null})
        );
        assert_eq!(
            Request::SetEffort(Effort::High).body(),
            json!({"subtype": "apply_flag_settings", "settings": {"effortLevel": "high"}})
        );
        assert_eq!(
            Request::BackgroundTasks(None).body(),
            json!({"subtype": "background_tasks"})
        );
    }

    #[test]
    fn allowing_sends_the_original_input_back_unless_it_was_edited() {
        let ask = ask(json!([]));
        let plain = PermissionAnswer::Allow {
            updated_input: None,
            always: vec![],
        }
        .payload(&ask);
        assert_eq!(plain["updatedInput"], ask.input);
        assert_eq!(plain["toolUseID"], "toolu_1");
        assert_eq!(plain["decisionClassification"], "user_temporary");
        let edited = PermissionAnswer::Allow {
            updated_input: Some(json!({"command": "touch other.txt"})),
            always: vec![],
        }
        .payload(&ask);
        assert_eq!(edited["updatedInput"]["command"], "touch other.txt");
        assert!(edited.get("updatedPermissions").is_none());
    }

    #[test]
    fn allowing_always_stores_the_chosen_suggestion_verbatim() {
        let rule = json!({"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "touch:*"}], "behavior": "allow", "destination": "localSettings"});
        let ask = ask(json!([rule.clone(), {"type": "setMode", "mode": "acceptEdits", "destination": "session"}]));
        let payload = PermissionAnswer::Allow {
            updated_input: None,
            always: vec![0],
        }
        .payload(&ask);
        assert_eq!(payload["updatedPermissions"], json!([rule]));
        assert_eq!(payload["decisionClassification"], "user_permanent");
        assert_eq!(
            Suggestion::read(&ask.permission_suggestions[0]),
            Some(Suggestion {
                rule: "Bash(touch:*)".into(),
                scope: "localSettings".into()
            })
        );
        assert_eq!(
            Suggestion::read(&ask.permission_suggestions[1]).unwrap().rule,
            "mode acceptEdits"
        );
    }

    #[test]
    fn denying_tells_the_model_why_and_may_stop_the_turn() {
        let payload = PermissionAnswer::Deny {
            message: "Not now".into(),
            interrupt: true,
        }
        .payload(&ask(json!([])));
        assert_eq!(payload["behavior"], "deny");
        assert_eq!(payload["message"], "Not now");
        assert_eq!(payload["interrupt"], true);
    }

    #[test]
    fn a_plan_is_revised_with_the_feedback_and_a_rejected_one_stops_the_turn() {
        let ask = ask(json!([]));
        let revise = PlanAnswer::Revise(Some("> step 1\n\nsmaller".into()))
            .permission()
            .payload(&ask);
        assert_eq!(revise["behavior"], "deny");
        assert_eq!(revise["message"], "> step 1\n\nsmaller");
        assert!(revise.get("interrupt").is_none());
        assert_eq!(
            PlanAnswer::Revise(None).permission().payload(&ask)["message"],
            PLAN_REVISE_MESSAGE
        );
        let reject = PlanAnswer::Reject.permission().payload(&ask);
        assert_eq!(
            (&reject["message"], &reject["interrupt"]),
            (&json!(PLAN_REJECT_MESSAGE), &json!(true))
        );
        assert_eq!(PlanAnswer::Approve.permission().payload(&ask)["behavior"], "allow");
        assert_eq!(
            PlanAnswer::Reject.decision(),
            Decision::Rejected {
                feedback: None,
                stop: true
            }
        );
        assert_eq!(
            plan_rejection(Some(PLAN_REVISE_MESSAGE), false),
            PlanAnswer::Revise(None).decision()
        );
        assert_eq!(
            plan_rejection(Some(" smaller "), false),
            Decision::Rejected {
                feedback: Some("smaller".into()),
                stop: false
            }
        );
    }

    #[test]
    fn an_unknown_request_from_the_cli_is_kept_by_its_subtype() {
        let incoming = Incoming::read(&json!({"subtype": "request_user_dialog", "dialog_kind": "x"}));
        assert_eq!(
            incoming,
            Incoming::Other {
                subtype: "request_user_dialog".into()
            }
        );
        assert!(matches!(
            Incoming::read(&json!({"subtype": "can_use_tool", "tool_name": "Bash", "tool_use_id": "t"})),
            Incoming::CanUseTool(_)
        ));
    }

    #[test]
    fn answers_are_read_by_their_request_type() {
        let output = TaskOutput::read(json!({"output": "tick 1\n", "total_bytes": 7, "truncated": false})).unwrap();
        assert_eq!(output.output, "tick 1\n");
        assert!(bool::read(json!({})).unwrap(), "an older CLI sends no flag");
        assert!(!bool::read(json!({"backgrounded": false})).unwrap());
        assert!(matches!(
            TaskOutput::read(json!({"output": 5})),
            Err(ControlError::Unexpected(_))
        ));
        let usage = UsageReport::read(json!({
            "session": {"total_cost_usd": 0.5, "model_usage": {"claude-haiku-5-5": {"inputTokens": 3, "outputTokens": 4, "costUSD": 0.5}}},
            "subscription_type": "max",
            "rate_limits": {"five_hour": {"utilization": 12.0, "resets_at": "2026-11-09T20:00:00Z"}, "seven_day": null}
        }));
        assert_eq!(usage.session_cost_usd, Some(0.5));
        assert_eq!(usage.session_models["claude-haiku-5-5"].output_tokens, 4);
        assert_eq!(usage.windows.len(), 1);
        assert_eq!(usage.windows[0].utilization, Some(12.0));
        assert_eq!(
            Request::GetUsage.body(),
            json!({"subtype": "get_usage", "skip_behaviors": true})
        );
    }

    #[test]
    fn a_subscription_is_told_from_an_api_key_and_a_cloud_account() {
        let read = |account: Value| InitializeResult::read(json!({ "account": account })).unwrap();
        assert!(read(json!({"subscriptionType": "pro", "apiProvider": "firstParty"})).on_subscription());
        assert!(!read(json!({"apiKeySource": "ANTHROPIC_API_KEY", "apiProvider": "firstParty"})).on_subscription());
        assert!(!read(json!({"subscriptionType": "max", "apiKeySource": "user"})).on_subscription());
        assert!(!read(json!({"subscriptionType": "max", "apiProvider": "bedrock"})).on_subscription());
        assert!(!read(json!({"tokenSource": "none", "apiProvider": "firstParty"})).on_subscription());
    }

    #[test]
    fn a_missing_sign_in_is_recognised_from_the_account() {
        let signed_out =
            InitializeResult::read(json!({"account": {"tokenSource": "none", "apiProvider": "firstParty"}}));
        assert!(signed_out.unwrap().signed_out());
        let subscriber =
            InitializeResult::read(json!({"account": {"subscriptionType": "max", "apiProvider": "firstParty"}}));
        assert!(!subscriber.unwrap().signed_out());
        let bedrock = InitializeResult::read(json!({"account": {"tokenSource": "none", "apiProvider": "bedrock"}}));
        assert!(!bedrock.unwrap().signed_out());
    }

    #[test]
    fn question_answers_are_added_to_the_input() {
        let input = json!({"questions": [{"question": "Which?", "options": []}]});
        let updated = question_answer(&input, &[("Which?".into(), "red, blue".into())]);
        assert_eq!(updated["answers"]["Which?"], "red, blue");
        assert_eq!(updated["questions"], input["questions"]);
    }
}
