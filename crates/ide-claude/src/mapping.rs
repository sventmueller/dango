//! Turns the CLI's frames into thread updates, as `docs/claude-mapping.md` lays down.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ide_thread::{
    AgentMessage, BackgroundItem, BackgroundKind, BackgroundPatch, ContextBreakdown, ContextGroup, ContextKind,
    ContextPart, ContextRow, Decision, EntryKind, ErrorKind, HookOutcome, InfoLevel, Lane, LimitStatus, ModelCost,
    Notice, OutputTail, PermissionOption, PermissionRequest, PlanApproval, Question, QuestionItem, QuestionOption,
    RateLimit, RequestState, Status, SubagentPatch, TaskStatus, Thought, Tokens, ToolCall, ToolPatch, ToolStatus,
    TurnUsage, Update, UsagePatch, UserMessage,
};
use serde_json::Value;

use crate::control::{self, ContextUsage, PermissionAsk, Suggestion, TaskOutput, UsageReport};
use crate::protocol::{
    ApiUsage, Assistant, BackgroundTask, Block, Delta, Hook, ImageSource, Message, ModelUsage, RateLimitInfo, RawEvent,
    StreamEvent, System, TaskNotification, TaskProgress, TaskStarted, TaskUpdated, TurnResult, User,
};
use crate::tools::{self, Shape};

/// The errors on assistant frames that mean the CLI is not signed in or its sign-in was refused.
const AUTH_ERRORS: [&str; 5] = [
    "authentication_failed",
    "oauth_org_not_allowed",
    "account_on_hold",
    "verification_required",
    "cloud_credential_error",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Text,
    Thought,
}

#[derive(Debug)]
struct Streamed {
    slot: Slot,
    key: String,
    claimed: bool,
}

#[derive(Debug, Default)]
struct Stream {
    message: String,
    input_tokens: u64,
}

#[derive(Debug)]
struct Tool {
    name: String,
    lane: Lane,
}

#[derive(Debug)]
struct Task {
    tool_use_id: Option<String>,
    subagent: Option<String>,
    started: SystemTime,
    output_file: Option<String>,
}

#[derive(Debug)]
struct Ask {
    key: String,
    tool_use_id: String,
}

/// The state that turns a stream of frames into thread updates; one per session.
#[derive(Debug, Default)]
pub struct Mapper {
    history: bool,
    model: Option<String>,
    turn: u64,
    streams: HashMap<Lane, Stream>,
    streamed: HashMap<String, Vec<Streamed>>,
    open: Vec<String>,
    unnamed: u64,
    tools: HashMap<String, Tool>,
    running: HashSet<String>,
    tool_requests: HashMap<String, String>,
    tasks: HashMap<String, Task>,
    runs: HashMap<String, String>,
    level: Vec<BackgroundTask>,
    seen: HashMap<String, SystemTime>,
    live_subagents: HashSet<String>,
    asks: HashMap<String, Ask>,
    /// The plans the user answered in this session, by their tool call.
    answered: HashSet<String>,
    frames: HashMap<String, Vec<String>>,
    auth_reported: bool,
    chain: Vec<String>,
    /// The session's totals after the last turn, which the next turn's figures are the difference to.
    totals: Totals,
    /// Whether `totals` are not known, as when the CLI resumed a session and may count from an earlier process.
    totals_unknown: bool,
    /// The prompts sent so far.
    prompts: u64,
    /// The ids of the prompts the app sent, whose echoes it leaves out.
    sent: HashSet<String>,
    /// Whether the last status sent was other than idle.
    busy: bool,
}

/// What the CLI counts for the whole session: its cost and tokens over all models.
#[derive(Debug, Clone, Copy, Default)]
struct Totals {
    cost_usd: f64,
    tokens: Tokens,
}

fn tokens_of<'a>(models: impl Iterator<Item = &'a ModelUsage>) -> Tokens {
    models.fold(Tokens::default(), |sum, m| Tokens {
        input: sum.input + m.input_tokens,
        output: sum.output + m.output_tokens,
        cache_read: sum.cache_read + m.cache_read_input_tokens,
        cache_write: sum.cache_write + m.cache_creation_input_tokens,
    })
}

fn tokens_since(now: Tokens, before: Tokens) -> Tokens {
    Tokens {
        input: now.input.saturating_sub(before.input),
        output: now.output.saturating_sub(before.output),
        cache_read: now.cache_read.saturating_sub(before.cache_read),
        cache_write: now.cache_write.saturating_sub(before.cache_write),
    }
}

fn lane(parent: Option<&String>) -> Lane {
    parent.map_or(Lane::Main, |p| Lane::Subagent(p.clone()))
}

/// Whether `message` is the agent answering in the main conversation rather than in a subagent's.
fn writes_in_main(message: &Message) -> bool {
    match message {
        Message::Assistant(a) => a.parent_tool_use_id.is_none(),
        Message::StreamEvent(e) => e.parent_tool_use_id.is_none(),
        _ => false,
    }
}

fn put(lane: &Lane, key: Option<String>, at: SystemTime, entry: EntryKind) -> Update {
    Update::Put {
        lane: lane.clone(),
        key,
        at,
        entry,
    }
}

fn notice(at: SystemTime, notice: Notice) -> Update {
    put(&Lane::Main, None, at, EntryKind::Notice(notice))
}

fn seconds(secs: f64) -> Option<Duration> {
    Duration::try_from_secs_f64(secs).ok()
}

fn epoch_millis(ms: f64) -> Option<SystemTime> {
    seconds(ms / 1000.0).map(|d| UNIX_EPOCH + d)
}

fn task_status(status: &str) -> Option<TaskStatus> {
    Some(match status {
        "pending" => TaskStatus::Pending,
        "running" => TaskStatus::Running,
        "paused" => TaskStatus::Paused,
        "completed" => TaskStatus::Completed,
        "failed" => TaskStatus::Failed,
        "killed" | "stopped" => TaskStatus::Stopped,
        _ => return None,
    })
}

fn background_kind(task_type: &str) -> BackgroundKind {
    match task_type {
        "local_bash" => BackgroundKind::Shell,
        "local_agent" => BackgroundKind::Subagent,
        t if t.contains("monitor") => BackgroundKind::Monitor,
        t if t.contains("workflow") => BackgroundKind::Workflow,
        other => BackgroundKind::Other(other.to_string()),
    }
}

fn questions(input: &Value) -> Vec<QuestionItem> {
    let text = |v: &Value, f: &str| v.get(f).and_then(Value::as_str).map(str::to_string);
    input
        .get("questions")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|q| {
                    Some(QuestionItem {
                        header: text(q, "header"),
                        question: text(q, "question")?,
                        options: q
                            .get("options")
                            .and_then(Value::as_array)
                            .map(|options| {
                                options
                                    .iter()
                                    .filter_map(|o| {
                                        Some(QuestionOption {
                                            label: text(o, "label")?,
                                            description: text(o, "description"),
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        multi_select: q.get("multiSelect").and_then(Value::as_bool).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Mapper {
    /// A mapper for a live session, which adds the user's messages itself when they are sent.
    pub fn new() -> Mapper {
        Mapper::default()
    }

    /// A mapper for a saved transcript, which takes the user's messages from the frames.
    pub fn for_history() -> Mapper {
        Mapper {
            history: true,
            ..Mapper::default()
        }
    }

    /// The updates `message` makes, as of `now`.
    pub fn map(&mut self, message: &Message, now: SystemTime) -> Vec<Update> {
        // The CLI starts a turn of its own when a background task ends, with no frame that says so.
        let mut out = if !self.history && !self.busy && writes_in_main(message) {
            vec![self.status(Status::Running)]
        } else {
            Vec::new()
        };
        out.extend(self.frame(message, now));
        out
    }

    fn status(&mut self, status: Status) -> Update {
        self.busy = status != Status::Idle;
        Update::Status(status)
    }

    fn frame(&mut self, message: &Message, now: SystemTime) -> Vec<Update> {
        match message {
            Message::Assistant(a) => self.assistant(a, now),
            Message::User(u) => self.user(u, now),
            Message::Result(r) => self.result(r, now),
            Message::System(s) => self.system(s, now),
            Message::StreamEvent(e) => self.stream(e, now),
            Message::ToolProgress(p) => {
                let elapsed = seconds(p.elapsed_time_seconds);
                vec![Update::Tool {
                    key: p.tool_use_id.clone(),
                    patch: ToolPatch {
                        status: Some(ToolStatus::InProgress),
                        started: elapsed.and_then(|e| now.checked_sub(e)),
                        elapsed,
                        ..ToolPatch::default()
                    },
                }]
            }
            Message::ToolUseSummary(s) => s
                .preceding_tool_use_ids
                .iter()
                .map(|id| Update::Tool {
                    key: id.clone(),
                    patch: ToolPatch {
                        summary: Some(s.summary.clone()),
                        ..ToolPatch::default()
                    },
                })
                .collect(),
            Message::RateLimit(info) => rate_limit(info, now),
            Message::AuthStatus(a) => a
                .error
                .iter()
                .map(|error| {
                    notice(
                        now,
                        Notice::Error {
                            kind: ErrorKind::Auth,
                            message: error.clone(),
                        },
                    )
                })
                .collect(),
            Message::ConversationReset(_) => self.reset(now),
            Message::PromptSuggestion(_)
            | Message::KeepAlive
            | Message::ControlRequest { .. }
            | Message::ControlResponse(_)
            | Message::ControlCancel { .. }
            | Message::Unknown(_) => Vec::new(),
        }
    }

    /// The updates for a message the user sends, with `uuid` as the CLI's id for it.
    pub fn user_message(&mut self, message: UserMessage, uuid: &str, now: SystemTime) -> Vec<Update> {
        self.auth_reported = false;
        self.prompts += 1;
        self.sent.insert(uuid.to_string());
        let key = format!("user:{uuid}");
        let mut out = vec![put(
            &Lane::Main,
            Some(key.clone()),
            now,
            EntryKind::UserMessage(UserMessage {
                message_id: Some(uuid.to_string()),
                ..message
            }),
        )];
        out.extend(self.link(&[key], uuid));
        out.push(self.status(Status::Running));
        out
    }

    /// Appends the main conversation's frame `uuid` to the chain and ties the entries with `keys` to it.
    fn link(&mut self, keys: &[String], uuid: &str) -> Vec<Update> {
        let follows = self.chain.last().cloned();
        self.chain.push(uuid.to_string());
        keys.iter()
            .map(|key| Update::Chain {
                key: key.clone(),
                uuid: uuid.to_string(),
                follows: follows.clone(),
            })
            .collect()
    }

    /// The updates for a permission request of the CLI.
    pub fn permission(&mut self, request_id: &str, ask: &PermissionAsk, now: SystemTime) -> Vec<Update> {
        let lane = ask
            .agent_id
            .as_ref()
            .and_then(|agent| self.tasks.get(agent))
            .and_then(|task| task.subagent.clone())
            .map(Lane::Subagent)
            .or_else(|| self.tools.get(&ask.tool_use_id).map(|t| t.lane.clone()))
            .unwrap_or(Lane::Main);
        let call = ask.tool_use_id.clone();
        let mut out = Vec::new();
        let key = match tools::shape(&ask.tool_name) {
            Shape::Question => {
                self.tool_requests.insert(call.clone(), request_id.to_string());
                out.push(put(
                    &lane,
                    Some(call.clone()),
                    now,
                    question(&call, &ask.input, Some(request_id)),
                ));
                call.clone()
            }
            Shape::Plan => {
                self.tool_requests.insert(call.clone(), request_id.to_string());
                out.push(put(
                    &lane,
                    Some(call.clone()),
                    now,
                    plan(&call, &ask.input, Some(request_id)),
                ));
                call.clone()
            }
            Shape::Card | Shape::Todos => {
                let key = format!("perm:{request_id}");
                out.push(put(&lane, Some(key.clone()), now, permission_request(request_id, ask)));
                out.push(Update::Tool {
                    key: call.clone(),
                    patch: ToolPatch {
                        status: Some(ToolStatus::Pending),
                        ..ToolPatch::default()
                    },
                });
                key
            }
        };
        self.asks.insert(request_id.to_string(), Ask { key, tool_use_id: call });
        out.push(self.status(Status::RequiresAction));
        out
    }

    /// The entry key of the CLI's request `request_id`, while it waits.
    pub fn request_key(&self, request_id: &str) -> Option<&str> {
        self.asks.get(request_id).map(|a| a.key.as_str())
    }

    /// The updates for the user's decision on the CLI's request `request_id`.
    pub fn decided(&mut self, request_id: &str, decision: Decision) -> Vec<Update> {
        let Some(ask) = self.asks.remove(request_id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if matches!(decision, Decision::Approved | Decision::Rejected { .. }) {
            self.answered.insert(ask.tool_use_id.clone());
        }
        if matches!(
            decision,
            Decision::Allowed { .. } | Decision::Answered(_) | Decision::Approved
        ) {
            out.push(Update::Tool {
                key: ask.tool_use_id.clone(),
                patch: ToolPatch {
                    status: Some(ToolStatus::InProgress),
                    ..ToolPatch::default()
                },
            });
        }
        out.push(Update::Resolve {
            key: ask.key,
            state: RequestState::Decided(decision),
        });
        if self.asks.is_empty() {
            out.push(self.status(Status::Running));
        }
        out
    }

    /// The updates when the CLI withdraws its request `request_id`.
    pub fn withdrawn(&mut self, request_id: &str) -> Vec<Update> {
        let Some(ask) = self.asks.remove(request_id) else {
            return Vec::new();
        };
        let mut out = vec![Update::Resolve {
            key: ask.key,
            state: RequestState::Withdrawn,
        }];
        if self.asks.is_empty() {
            out.push(self.status(Status::Running));
        }
        out
    }

    /// The updates for the output of a background task that `get_task_output` returned.
    pub fn task_output(&self, task_id: &str, output: &TaskOutput) -> Vec<Update> {
        vec![Update::BackgroundItem(BackgroundPatch {
            id: task_id.to_string(),
            output: Some(OutputTail {
                path: None,
                text: Some(output.output.clone()),
                total_bytes: Some(output.total_bytes),
                truncated: output.truncated,
            }),
            ..BackgroundPatch::default()
        })]
    }

    /// The updates for the context usage that `get_context_usage` returned at `now`; `exact` when it counted every
    /// category rather than estimating.
    pub fn context(&self, usage: &ContextUsage, exact: bool, now: SystemTime) -> Vec<Update> {
        vec![Update::Usage(UsagePatch {
            context_used: Some(usage.total_tokens),
            context_size: (usage.raw_max_tokens > 0).then_some(usage.raw_max_tokens),
            breakdown: Some(breakdown(usage, exact, now)),
            ..UsagePatch::default()
        })]
    }

    /// Forgets the session's totals: a resumed CLI may count on from an earlier process, so the next turn's cost
    /// stays unknown unless `baseline` learns the totals first.
    pub fn resumed(&mut self) {
        self.totals_unknown = true;
    }

    /// The prompts sent so far.
    pub fn prompts(&self) -> u64 {
        self.prompts
    }

    /// Takes the session's totals from a `get_usage` answer that was asked for after `prompts` prompts, unless they
    /// are known already or a prompt was sent since, whose turn the answer may count in part.
    pub fn baseline(&mut self, report: &UsageReport, prompts: u64) {
        if !self.totals_unknown || self.prompts != prompts {
            return;
        }
        if let Some(cost_usd) = report.session_cost_usd {
            self.totals = Totals {
                cost_usd,
                tokens: tokens_of(report.session_models.values()),
            };
            self.totals_unknown = false;
        }
    }

    /// The updates when the CLI's process has ended: nothing streams, waits or runs any more.
    pub fn ended(&mut self, now: SystemTime) -> Vec<Update> {
        let mut out = self.close_streams(now);
        let asks: Vec<String> = self.asks.keys().cloned().collect();
        for request in asks {
            out.extend(self.withdrawn(&request));
        }
        for call in self.running.drain() {
            out.push(Update::Tool {
                key: call,
                patch: ToolPatch {
                    status: Some(ToolStatus::Failed),
                    ended: Some(now),
                    ..ToolPatch::default()
                },
            });
        }
        for key in self.live_subagents.drain() {
            let mut patch = SubagentPatch::new(key, now);
            patch.status = Some(TaskStatus::Stopped);
            patch.ended = Some(now);
            patch.cancellable = Some(false);
            out.push(Update::Subagent(patch));
        }
        for task in self.level.drain(..) {
            out.push(Update::BackgroundItem(BackgroundPatch {
                id: task.task_id,
                status: Some(TaskStatus::Stopped),
                ended: Some(now),
                ..BackgroundPatch::default()
            }));
        }
        out.push(Update::Background(Vec::new()));
        out.push(Update::Activity(None));
        out.push(self.status(Status::Idle));
        out
    }

    fn close_streams(&mut self, now: SystemTime) -> Vec<Update> {
        self.streamed.clear();
        self.open
            .drain(..)
            .map(|key| Update::Finish {
                key,
                text: None,
                at: now,
            })
            .collect()
    }

    fn reset(&mut self, now: SystemTime) -> Vec<Update> {
        self.streams.clear();
        self.streamed.clear();
        self.open.clear();
        self.tools.clear();
        self.running.clear();
        self.frames.clear();
        self.chain.clear();
        vec![Update::Clear, Update::Background(self.background_items(now))]
    }

    fn assistant(&mut self, a: &Assistant, now: SystemTime) -> Vec<Update> {
        let lane = lane(a.parent_tool_use_id.as_ref());
        if let Some(error) = &a.error {
            return self.assistant_error(error, a, now);
        }
        let mut out = Vec::new();
        let mut keys = Vec::new();
        let id = &a.message.id;
        for block in &a.message.content {
            match block {
                Block::Text { text } => keys.push(self.text_block(&lane, id, Slot::Text, text, now, &mut out)),
                Block::Thinking { thinking } => {
                    keys.push(self.text_block(&lane, id, Slot::Thought, thinking, now, &mut out));
                }
                Block::RedactedThinking => keys.push(self.text_block(&lane, id, Slot::Thought, "", now, &mut out)),
                Block::ToolUse { id, name, input } | Block::ServerToolUse { id, name, input } => {
                    self.tool_use(
                        &lane,
                        a.parent_tool_use_id.as_ref(),
                        id,
                        name,
                        input,
                        true,
                        now,
                        &mut out,
                    );
                    keys.push(id.clone());
                }
                _ => {}
            }
        }
        for gone in &a.supersedes {
            out.extend(self.retract(gone));
        }
        if let Some(uuid) = &a.uuid {
            if lane == Lane::Main {
                out.extend(self.link(&keys, uuid));
            }
            self.frames.insert(uuid.clone(), keys);
        }
        if lane == Lane::Main
            && !self.streams.get(&lane).is_some_and(|s| &s.message == id)
            && let Some(usage) = &a.message.usage
        {
            out.push(context_used(usage.context_input() + usage.output_tokens.unwrap_or(0)));
        }
        out
    }

    fn assistant_error(&mut self, error: &str, a: &Assistant, now: SystemTime) -> Vec<Update> {
        let text: Vec<&str> = a
            .message
            .content
            .iter()
            .filter_map(|b| match b {
                Block::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let kind = if AUTH_ERRORS.contains(&error) {
            self.auth_reported = true;
            ErrorKind::Auth
        } else {
            ErrorKind::Api
        };
        let message = if text.is_empty() {
            error.to_string()
        } else {
            text.join("\n")
        };
        vec![notice(now, Notice::Error { kind, message })]
    }

    fn retract(&mut self, uuid: &str) -> Vec<Update> {
        self.chain.retain(|frame| frame != uuid);
        self.frames
            .remove(uuid)
            .unwrap_or_default()
            .into_iter()
            .map(|key| Update::Remove { key })
            .collect()
    }

    fn text_block(
        &mut self,
        lane: &Lane,
        message: &str,
        slot: Slot,
        text: &str,
        now: SystemTime,
        out: &mut Vec<Update>,
    ) -> String {
        let streamed = self
            .streamed
            .get_mut(message)
            .and_then(|list| list.iter_mut().find(|s| !s.claimed && s.slot == slot));
        if let Some(streamed) = streamed {
            streamed.claimed = true;
            let key = streamed.key.clone();
            self.open.retain(|k| k != &key);
            out.push(Update::Finish {
                key: key.clone(),
                text: Some(text.to_string()),
                at: now,
            });
            return key;
        }
        self.unnamed += 1;
        let key = format!("{message}:f{}", self.unnamed);
        let entry = match slot {
            Slot::Text => EntryKind::AgentMessage(AgentMessage {
                markdown: text.to_string(),
                streaming: false,
            }),
            Slot::Thought => EntryKind::Thought(Thought {
                text: text.to_string(),
                duration: None,
                streaming: false,
            }),
        };
        out.push(put(lane, Some(key.clone()), now, entry));
        key
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one call site per frame kind, all fields are needed"
    )]
    fn tool_use(
        &mut self,
        lane: &Lane,
        parent: Option<&String>,
        id: &str,
        name: &str,
        input: &Value,
        complete: bool,
        now: SystemTime,
        out: &mut Vec<Update>,
    ) {
        let known = self.tools.contains_key(id);
        self.tools.insert(
            id.to_string(),
            Tool {
                name: name.to_string(),
                lane: lane.clone(),
            },
        );
        let request = self.tool_requests.get(id).map(String::as_str);
        match tools::shape(name) {
            Shape::Todos => {
                let list = tools::todos(input);
                if complete && *lane == Lane::Main {
                    out.push(Update::Todos(list.clone()));
                }
                out.push(put(lane, Some(id.to_string()), now, EntryKind::Todos(list)));
            }
            Shape::Question => out.push(put(lane, Some(id.to_string()), now, question(id, input, request))),
            Shape::Plan => out.push(put(lane, Some(id.to_string()), now, plan(id, input, request))),
            Shape::Card => {
                let (kind, title, view) = tools::describe(name, input);
                let status = if complete {
                    ToolStatus::InProgress
                } else {
                    ToolStatus::Pending
                };
                if complete {
                    self.running.insert(id.to_string());
                }
                if known {
                    out.push(Update::Tool {
                        key: id.to_string(),
                        patch: ToolPatch {
                            input: Some(input.clone()),
                            title: Some(title),
                            view: Some(view),
                            status: Some(status),
                            started: complete.then_some(now),
                            ..ToolPatch::default()
                        },
                    });
                } else {
                    let mut call = ToolCall::new(id, name, kind);
                    call.title = title;
                    call.view = view;
                    call.input = input.clone();
                    call.status = status;
                    call.started = complete.then_some(now);
                    call.parent = parent.cloned();
                    out.push(put(lane, Some(id.to_string()), now, EntryKind::ToolCall(call)));
                }
                if complete && tools::starts_subagent(name) {
                    out.push(self.subagent_from_call(id, input, now));
                }
            }
        }
    }

    fn subagent_from_call(&mut self, id: &str, input: &Value, now: SystemTime) -> Update {
        let text = |f: &str| input.get(f).and_then(Value::as_str).map(str::to_string);
        self.live_subagents.insert(id.to_string());
        let mut patch = SubagentPatch::new(id, now);
        patch.description = text("description");
        patch.agent_type = text("subagent_type");
        patch.status = Some(TaskStatus::Running);
        patch.backgrounded = input.get("run_in_background").and_then(Value::as_bool);
        Update::Subagent(patch)
    }

    fn user(&mut self, u: &User, now: SystemTime) -> Vec<Update> {
        if u.is_replay == Some(true) {
            return self.prompt_from_elsewhere(u, now);
        }
        let lane = lane(u.parent_tool_use_id.as_ref());
        let mut out = Vec::new();
        let mut text = Vec::new();
        let mut images = Vec::new();
        let mut keys = Vec::new();
        for block in u.message.blocks() {
            match block {
                Block::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    self.tool_result(
                        &tool_use_id,
                        &content,
                        is_error == Some(true),
                        u.tool_use_result.as_ref(),
                        now,
                        &mut out,
                    );
                    keys.push(tool_use_id);
                }
                Block::Text { text: t } => text.push(t),
                Block::Image { source: Some(source) } => images.extend(image(&source)),
                _ => {}
            }
        }
        let wanted = self.history || lane != Lane::Main;
        if wanted && !(text.is_empty() && images.is_empty()) && u.is_synthetic != Some(true) {
            let key = u.uuid.as_ref().map(|id| format!("user:{id}"));
            let message = UserMessage {
                text: text.join("\n"),
                attachments: images,
                message_id: u.uuid.clone(),
            };
            keys.extend(key.clone());
            out.push(put(&lane, key, now, EntryKind::UserMessage(message)));
        }
        // Every user-role frame of the main conversation is part of its chain, also those the timeline leaves out.
        if lane == Lane::Main
            && let Some(uuid) = &u.uuid
        {
            out.extend(self.link(&keys, uuid));
        }
        out
    }

    /// The echo of a prompt the app did not send, such as one typed on claude.ai through Remote Control, as the user's
    /// message; echoes of the app's own prompts, of the CLI's own and of task notifications are left out.
    fn prompt_from_elsewhere(&mut self, u: &User, now: SystemTime) -> Vec<Update> {
        let Some(uuid) = u.uuid.as_deref() else {
            return Vec::new();
        };
        let person = u
            .origin
            .as_ref()
            .is_none_or(|o| matches!(o.get("kind").and_then(Value::as_str), Some("human" | "remote")));
        if self.history
            || self.sent.contains(uuid)
            || u.parent_tool_use_id.is_some()
            || u.is_synthetic == Some(true)
            || !person
        {
            return Vec::new();
        }
        let mut text = Vec::new();
        let mut images = Vec::new();
        for block in u.message.blocks() {
            match block {
                Block::Text { text: t } => text.push(t),
                Block::Image { source: Some(source) } => images.extend(image(&source)),
                _ => {}
            }
        }
        let text = text.join("\n");
        if text.trim_start().starts_with('<') || (text.trim().is_empty() && images.is_empty()) {
            return Vec::new();
        }
        let message = UserMessage {
            text,
            attachments: images,
            message_id: None,
        };
        self.user_message(message, uuid, now)
    }

    fn tool_result(
        &mut self,
        id: &str,
        content: &Value,
        failed: bool,
        structured: Option<&Value>,
        now: SystemTime,
        out: &mut Vec<Update>,
    ) {
        self.running.remove(id);
        let name = self.tools.get(id).map(|t| t.name.clone()).unwrap_or_default();
        match tools::shape(&name) {
            Shape::Todos => {}
            Shape::Question => {
                let answers: Vec<ide_thread::Answer> = structured
                    .and_then(|s| s.get("answers"))
                    .and_then(Value::as_object)
                    .map(|map| {
                        map.iter()
                            .map(|(q, a)| ide_thread::Answer {
                                question: q.clone(),
                                answer: a.as_str().map_or_else(|| a.to_string(), str::to_string),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let decision = if failed {
                    Decision::Denied {
                        message: Some(tools::result_text(content)),
                    }
                } else {
                    Decision::Answered(answers)
                };
                out.push(Update::Resolve {
                    key: id.to_string(),
                    state: RequestState::Decided(decision),
                });
            }
            // The user's own answer stands; only a session rebuilt from a transcript reads it back from the result.
            Shape::Plan if self.answered.remove(id) => {}
            Shape::Plan => {
                let decision = if failed {
                    let text = tools::result_text(content);
                    control::plan_rejection(Some(&text), text.contains(control::PLAN_REJECT_MESSAGE))
                } else {
                    Decision::Approved
                };
                out.push(Update::Resolve {
                    key: id.to_string(),
                    state: RequestState::Decided(decision),
                });
            }
            Shape::Card => {
                out.push(Update::Tool {
                    key: id.to_string(),
                    patch: ToolPatch {
                        status: Some(if failed {
                            ToolStatus::Failed
                        } else {
                            ToolStatus::Completed
                        }),
                        output: Some(tools::output(&name, structured, content)),
                        ended: Some(now),
                        results: tools::result_count(&name, structured),
                        background_task: tools::background_task(structured),
                        ..ToolPatch::default()
                    },
                });
                if tools::starts_subagent(&name)
                    && let Some(update) = self.subagent_result(id, failed, structured, now)
                {
                    out.push(update);
                }
            }
        }
    }

    fn subagent_result(
        &mut self,
        id: &str,
        failed: bool,
        structured: Option<&Value>,
        now: SystemTime,
    ) -> Option<Update> {
        let result = structured.unwrap_or(&Value::Null);
        let status = match (failed, result.get("status").and_then(Value::as_str)) {
            (true, _) => TaskStatus::Failed,
            (false, Some("completed")) => TaskStatus::Completed,
            (false, Some(other)) => task_status(other)?,
            (false, None) => TaskStatus::Completed,
        };
        let number = |f: &str| result.get(f).and_then(Value::as_u64);
        self.live_subagents.remove(id);
        let mut patch = SubagentPatch::new(id, now);
        patch.status = Some(status);
        patch.ended = Some(now);
        patch.tokens = number("totalTokens");
        patch.tool_uses = number("totalToolUseCount");
        patch.elapsed = number("totalDurationMs").map(Duration::from_millis);
        patch.cancellable = Some(false);
        Some(Update::Subagent(patch))
    }

    fn result(&mut self, r: &TurnResult, now: SystemTime) -> Vec<Update> {
        let mut out = self.close_streams(now);
        let aborted = r.terminal_reason.as_deref().is_some_and(|t| t.starts_with("aborted"));
        if aborted {
            for call in self.running.drain() {
                out.push(Update::Tool {
                    key: call,
                    patch: ToolPatch {
                        status: Some(ToolStatus::Failed),
                        ended: Some(now),
                        ..ToolPatch::default()
                    },
                });
            }
        }
        let mut usage = self.result_usage(r);
        let rejected = rewind_rejected(r);
        if rejected.is_none() {
            usage.turn = Some(self.turn_usage(r));
        }
        out.push(Update::Usage(usage));
        if let Some(detail) = rejected {
            out.push(notice(now, Notice::RewindRejected { detail }));
        } else if r.is_error && !aborted {
            let text = r.result.clone().unwrap_or_default();
            let auth = text.contains("/login");
            if !(auth && self.auth_reported) {
                let message = match &r.errors {
                    Some(errors) if !errors.is_empty() => errors.join("\n"),
                    _ if !text.is_empty() => text,
                    _ => r.subtype.clone(),
                };
                let kind = if auth { ErrorKind::Auth } else { ErrorKind::Turn };
                out.push(notice(now, Notice::Error { kind, message }));
            }
        } else if aborted {
            out.push(notice(now, Notice::Interrupted));
        }
        out.push(Update::Activity(None));
        out.push(self.status(Status::Idle));
        self.turn += 1;
        self.auth_reported = false;
        out
    }

    /// What the turn that `r` ends took: the difference of the session's totals after it and before it.
    fn turn_usage(&mut self, r: &TurnResult) -> TurnUsage {
        let known = !self.totals_unknown;
        let tokens = tokens_of(r.model_usage.values());
        let turn = TurnUsage {
            message: None,
            cost_usd: r
                .total_cost_usd
                .filter(|_| known)
                .map(|cost| (cost - self.totals.cost_usd).max(0.0)),
            tokens: known.then(|| tokens_since(tokens, self.totals.tokens)),
            duration: r.duration_ms.map(Duration::from_millis),
        };
        if let Some(cost_usd) = r.total_cost_usd {
            self.totals.cost_usd = cost_usd;
            self.totals_unknown = false;
        }
        if !r.model_usage.is_empty() {
            self.totals.tokens = tokens;
        }
        turn
    }

    fn result_usage(&self, r: &TurnResult) -> UsagePatch {
        let mut patch = UsagePatch {
            cost_usd: r.total_cost_usd,
            models: (!r.model_usage.is_empty()).then(|| model_costs(r)),
            ..UsagePatch::default()
        };
        if !r.model_usage.is_empty() {
            let sum = |f: fn(&crate::protocol::ModelUsage) -> u64| Some(r.model_usage.values().map(f).sum());
            patch.input_tokens = sum(|m| m.input_tokens);
            patch.output_tokens = sum(|m| m.output_tokens);
            patch.cache_read_tokens = sum(|m| m.cache_read_input_tokens);
            patch.cache_write_tokens = sum(|m| m.cache_creation_input_tokens);
            patch.context_size = self
                .model
                .as_ref()
                .and_then(|m| r.model_usage.get(m))
                .and_then(|m| m.context_window)
                .or_else(|| r.model_usage.values().filter_map(|m| m.context_window).max());
        }
        patch
    }

    fn system(&mut self, s: &System, now: SystemTime) -> Vec<Update> {
        match s {
            System::Init(init) => {
                self.model = Some(init.model.clone()).filter(|m| !m.is_empty());
                let mut out = vec![Update::Session(init.session_id.clone())];
                if let Some(model) = &self.model {
                    out.push(Update::Model(model.clone()));
                }
                if !init.permission_mode.is_empty() {
                    out.push(Update::Mode(init.permission_mode.clone()));
                }
                if init.effort.is_some() {
                    out.push(Update::Effort(init.effort.clone()));
                }
                out
            }
            System::Status {
                status,
                permission_mode,
                compact_failed,
                compact_error,
            } => status_frame(
                status.as_deref(),
                permission_mode.as_ref(),
                *compact_failed,
                compact_error,
                now,
            ),
            System::ApiRetry(r) => vec![put(
                &Lane::Main,
                Some(format!("retry:{}", self.turn)),
                now,
                EntryKind::Notice(Notice::Retrying {
                    attempt: r.attempt,
                    max_attempts: r.max_retries,
                    delay: Duration::from_millis(r.retry_delay_ms),
                    error: r.error.clone(),
                    http_status: r.error_status,
                }),
            )],
            System::CompactBoundary(c) => {
                let mut out = vec![notice(
                    now,
                    Notice::Compaction {
                        manual: c.trigger == "manual",
                        tokens_before: c.pre_tokens,
                        tokens_after: c.post_tokens,
                    },
                )];
                if let Some(after) = c.post_tokens {
                    out.push(context_used(after));
                }
                out
            }
            System::TaskStarted(t) => self.task_started(t, now),
            System::TaskProgress(p) => self.task_progress(p, now),
            System::TaskUpdated(u) => self.task_updated(u, now),
            System::TaskNotification(n) => self.task_notification(n, now),
            System::BackgroundTasksChanged(tasks) => self.background_changed(tasks, now),
            System::Hook(hook) => vec![hook_notice(hook, now)],
            System::SessionStateChanged(state) => match state.as_str() {
                "idle" => vec![self.status(Status::Idle)],
                "running" => vec![self.status(Status::Running)],
                "requires_action" => vec![self.status(Status::RequiresAction)],
                _ => Vec::new(),
            },
            _ => self.other_system(s, now),
        }
    }

    fn other_system(&mut self, s: &System, now: SystemTime) -> Vec<Update> {
        match s {
            System::Informational { content, level } => {
                let level = match level.as_str() {
                    "warning" => InfoLevel::Warning,
                    "suggestion" => InfoLevel::Suggestion,
                    "notice" => InfoLevel::Notice,
                    _ => InfoLevel::Info,
                };
                vec![notice(
                    now,
                    Notice::Info {
                        level,
                        text: content.clone(),
                    },
                )]
            }
            System::LocalCommandOutput(text) => vec![put(
                &Lane::Main,
                None,
                now,
                EntryKind::AgentMessage(AgentMessage {
                    markdown: text.clone(),
                    streaming: false,
                }),
            )],
            System::Notification(text) => vec![notice(
                now,
                Notice::Info {
                    level: InfoLevel::Notice,
                    text: text.clone(),
                },
            )],
            System::PermissionDenied { tool_use_id, .. } => vec![Update::Tool {
                key: tool_use_id.clone(),
                patch: ToolPatch {
                    status: Some(ToolStatus::Failed),
                    ..ToolPatch::default()
                },
            }],
            System::Refusal { content, retracted } => {
                let mut out: Vec<Update> = retracted.iter().flat_map(|uuid| self.retract(uuid)).collect();
                out.push(notice(
                    now,
                    Notice::Info {
                        level: InfoLevel::Warning,
                        text: content.clone(),
                    },
                ));
                out
            }
            System::WritesLost(code) => vec![notice(
                now,
                Notice::Error {
                    kind: ErrorKind::Disk,
                    message: code.clone(),
                },
            )],
            _ => Vec::new(),
        }
    }

    /// Whether a frame of `run` of `task` is late because a later run of the task is already known.
    fn late(&mut self, task: &str, run: Option<&String>) -> bool {
        let Some(run) = run else { return false };
        match self.runs.get(task) {
            Some(known) if run < known => true,
            Some(known) if run == known => false,
            _ => {
                self.runs.insert(task.to_string(), run.clone());
                false
            }
        }
    }

    fn task_started(&mut self, t: &TaskStarted, now: SystemTime) -> Vec<Update> {
        if self.late(&t.task_id, t.run_id.as_ref()) {
            return Vec::new();
        }
        let kind = t.task_type.as_deref().unwrap_or_default();
        let is_agent = kind == "local_agent" || kind.is_empty() && t.subagent_type.is_some();
        let key = is_agent.then(|| t.tool_use_id.clone().unwrap_or_else(|| t.task_id.clone()));
        let parent = t
            .parent_task_id
            .as_ref()
            .and_then(|p| self.tasks.get(p))
            .and_then(|task| task.subagent.clone());
        self.tasks.insert(
            t.task_id.clone(),
            Task {
                tool_use_id: t.tool_use_id.clone(),
                subagent: key.clone(),
                started: now,
                output_file: None,
            },
        );
        let mut out = Vec::new();
        if let Some(key) = key {
            self.live_subagents.insert(key.clone());
            let mut patch = SubagentPatch::new(key, now);
            patch.task_id = Some(t.task_id.clone());
            patch.agent_type.clone_from(&t.subagent_type);
            patch.description = Some(t.description.clone()).filter(|d| !d.is_empty());
            patch.status = Some(TaskStatus::Running);
            patch.backgrounded = t.is_backgrounded;
            patch.cancellable = Some(true);
            patch.parent = parent;
            out.push(Update::Subagent(patch));
        } else if let (Some(call), Some(true)) = (&t.tool_use_id, t.is_backgrounded) {
            out.push(Update::Tool {
                key: call.clone(),
                patch: ToolPatch {
                    background_task: Some(t.task_id.clone()),
                    ..ToolPatch::default()
                },
            });
        }
        if self.level.iter().any(|b| b.task_id == t.task_id) {
            out.push(Update::Background(self.background_items(now)));
        }
        out
    }

    fn subagent_key(&self, task_id: &str, tool_use_id: Option<&String>) -> Option<String> {
        if let Some(task) = self.tasks.get(task_id) {
            return task.subagent.clone();
        }
        tool_use_id
            .filter(|call| self.tools.get(*call).is_some_and(|t| tools::starts_subagent(&t.name)))
            .cloned()
    }

    fn task_progress(&mut self, p: &TaskProgress, now: SystemTime) -> Vec<Update> {
        if self.late(&p.task_id, p.run_id.as_ref()) {
            return Vec::new();
        }
        let Some(key) = self.subagent_key(&p.task_id, p.tool_use_id.as_ref()) else {
            return Vec::new();
        };
        let mut patch = SubagentPatch::new(key, now);
        patch.task_id = Some(p.task_id.clone());
        patch.elapsed = Some(Duration::from_millis(p.usage.duration_ms));
        patch.tokens = Some(p.usage.total_tokens);
        patch.tool_uses = Some(p.usage.tool_uses);
        patch.last_tool.clone_from(&p.last_tool_name);
        patch.summary.clone_from(&p.summary);
        vec![Update::Subagent(patch)]
    }

    fn task_updated(&mut self, u: &TaskUpdated, now: SystemTime) -> Vec<Update> {
        if self.late(&u.task_id, u.run_id.as_ref()) {
            return Vec::new();
        }
        let status = u.patch.status.as_deref().and_then(task_status);
        let ended = u.patch.end_time.and_then(epoch_millis);
        match self.subagent_key(&u.task_id, None) {
            Some(key) => {
                if status.is_some_and(TaskStatus::is_final) {
                    self.live_subagents.remove(&key);
                }
                let mut patch = SubagentPatch::new(key, now);
                patch.status = status;
                patch.ended = ended;
                patch.backgrounded = u.patch.is_backgrounded;
                patch.summary.clone_from(&u.patch.error);
                patch.description.clone_from(&u.patch.description);
                if status.is_some_and(TaskStatus::is_final) {
                    patch.cancellable = Some(false);
                }
                vec![Update::Subagent(patch)]
            }
            None => vec![Update::BackgroundItem(BackgroundPatch {
                id: u.task_id.clone(),
                status,
                ended,
                output: None,
                is_backgrounded: u.patch.is_backgrounded,
            })],
        }
    }

    fn task_notification(&mut self, n: &TaskNotification, now: SystemTime) -> Vec<Update> {
        if self.late(&n.task_id, n.run_id.as_ref()) {
            return Vec::new();
        }
        let status = task_status(&n.status);
        if let Some(task) = self.tasks.get_mut(&n.task_id) {
            task.output_file.clone_from(&n.output_file);
        }
        match self.subagent_key(&n.task_id, n.tool_use_id.as_ref()) {
            Some(key) => {
                self.live_subagents.remove(&key);
                let mut patch = SubagentPatch::new(key, now);
                patch.task_id = Some(n.task_id.clone());
                patch.status = status;
                patch.ended = Some(now);
                patch.summary.clone_from(&n.summary);
                patch.cancellable = Some(false);
                if let Some(usage) = n.usage {
                    patch.tokens = Some(usage.total_tokens);
                    patch.tool_uses = Some(usage.tool_uses);
                    patch.elapsed = Some(Duration::from_millis(usage.duration_ms));
                }
                vec![Update::Subagent(patch)]
            }
            None => vec![Update::BackgroundItem(BackgroundPatch {
                id: n.task_id.clone(),
                status,
                ended: status.filter(|s| s.is_final()).map(|_| now),
                output: Some(OutputTail {
                    path: n.output_file.clone(),
                    ..OutputTail::default()
                }),
                is_backgrounded: None,
            })],
        }
    }

    fn background_changed(&mut self, tasks: &[BackgroundTask], now: SystemTime) -> Vec<Update> {
        // A set that names an older run of a task is older than the set already shown.
        let late = tasks.iter().any(|t| {
            let known = self.runs.get(&t.task_id);
            matches!((t.run_id.as_ref(), known), (Some(run), Some(known)) if run < known)
        });
        if late {
            return Vec::new();
        }
        for task in tasks {
            self.late(&task.task_id, task.run_id.as_ref());
            self.seen.entry(task.task_id.clone()).or_insert(now);
        }
        self.level = tasks.to_vec();
        vec![Update::Background(self.background_items(now))]
    }

    fn background_items(&self, now: SystemTime) -> Vec<BackgroundItem> {
        self.level
            .iter()
            .map(|b| {
                let task = self.tasks.get(&b.task_id);
                BackgroundItem {
                    id: b.task_id.clone(),
                    kind: background_kind(&b.task_type),
                    label: b.description.clone(),
                    status: TaskStatus::Running,
                    started: task
                        .map(|t| t.started)
                        .or_else(|| self.seen.get(&b.task_id).copied())
                        .or(Some(now)),
                    ended: None,
                    tool_call: task.and_then(|t| t.tool_use_id.clone()),
                    parent: b.parent_task_id.clone(),
                    output: OutputTail {
                        path: task.and_then(|t| t.output_file.clone()),
                        ..OutputTail::default()
                    },
                    is_backgrounded: true,
                    ambient: b.ambient.unwrap_or(false),
                }
            })
            .collect()
    }

    fn stream(&mut self, e: &StreamEvent, now: SystemTime) -> Vec<Update> {
        let lane = lane(e.parent_tool_use_id.as_ref());
        match &e.event {
            RawEvent::MessageStart { message } => {
                let input = message.usage.as_ref().map_or(0, ApiUsage::context_input);
                self.streams.insert(
                    lane.clone(),
                    Stream {
                        message: message.id.clone(),
                        input_tokens: input,
                    },
                );
                if lane == Lane::Main && input > 0 {
                    return vec![context_used(input)];
                }
                Vec::new()
            }
            RawEvent::ContentBlockStart { index, content_block } => self.block_start(&lane, *index, content_block, now),
            RawEvent::ContentBlockDelta { index, delta } => {
                let text = match delta {
                    Delta::TextDelta { text } => text,
                    Delta::ThinkingDelta { thinking } => thinking,
                    _ => return Vec::new(),
                };
                let Some(stream) = self.streams.get(&lane) else {
                    return Vec::new();
                };
                vec![Update::Append {
                    key: format!("{}:{index}", stream.message),
                    text: text.clone(),
                }]
            }
            RawEvent::ContentBlockStop { index } => {
                let Some(stream) = self.streams.get(&lane) else {
                    return Vec::new();
                };
                let key = format!("{}:{index}", stream.message);
                if !self.open.contains(&key) {
                    return Vec::new();
                }
                self.open.retain(|k| k != &key);
                vec![Update::Finish {
                    key,
                    text: None,
                    at: now,
                }]
            }
            RawEvent::MessageDelta { usage: Some(usage) } if lane == Lane::Main => {
                let input = self.streams.get(&lane).map_or(0, |s| s.input_tokens);
                vec![context_used(input + usage.output_tokens.unwrap_or(0))]
            }
            _ => Vec::new(),
        }
    }

    fn block_start(&mut self, lane: &Lane, index: usize, block: &Block, now: SystemTime) -> Vec<Update> {
        let Some(message) = self.streams.get(lane).map(|s| s.message.clone()) else {
            return Vec::new();
        };
        let key = format!("{message}:{index}");
        let (slot, entry) = match block {
            Block::Text { .. } => (
                Slot::Text,
                EntryKind::AgentMessage(AgentMessage {
                    markdown: String::new(),
                    streaming: true,
                }),
            ),
            Block::Thinking { .. } | Block::RedactedThinking => (
                Slot::Thought,
                EntryKind::Thought(Thought {
                    text: String::new(),
                    duration: None,
                    streaming: true,
                }),
            ),
            Block::ToolUse { id, name, input } | Block::ServerToolUse { id, name, input } => {
                let parent = match lane {
                    Lane::Main => None,
                    Lane::Subagent(p) => Some(p.clone()),
                };
                let mut out = Vec::new();
                if !self.tools.contains_key(id) {
                    self.tool_use(lane, parent.as_ref(), id, name, input, false, now, &mut out);
                }
                return out;
            }
            _ => return Vec::new(),
        };
        self.streamed.entry(message).or_default().push(Streamed {
            slot,
            key: key.clone(),
            claimed: false,
        });
        self.open.push(key.clone());
        vec![put(lane, Some(key), now, entry)]
    }
}

/// The session's totals by model, the most expensive first.
fn model_costs(r: &TurnResult) -> Vec<ModelCost> {
    let mut models: Vec<ModelCost> = r
        .model_usage
        .iter()
        .map(|(model, usage)| ModelCost {
            model: model.clone(),
            cost_usd: usage.cost_usd,
            tokens: tokens_of(std::iter::once(usage)),
            context_window: usage.context_window,
        })
        .collect();
    models.sort_by(|a, b| b.cost_usd.total_cmp(&a.cost_usd).then_with(|| a.model.cmp(&b.model)));
    models
}

/// The thread's view of a `get_context_usage` answer.
fn breakdown(usage: &ContextUsage, exact: bool, now: SystemTime) -> ContextBreakdown {
    let rows = usage
        .categories
        .iter()
        .map(|c| ContextRow {
            id: c.id.clone(),
            name: c.name.clone(),
            tokens: c.tokens,
            kind: match c.kind.as_str() {
                "free" => ContextKind::Free,
                "buffer" => ContextKind::Buffer,
                "deferred" => ContextKind::Deferred,
                _ => ContextKind::Used,
            },
        })
        .collect();
    let threshold = usage
        .auto_compact_threshold
        .filter(|t| usage.is_auto_compact_enabled && t.is_finite() && *t > 0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a positive token count far below 2^53"
    )]
    let auto_compact_at = threshold.map(|t| {
        if t <= 1.0 {
            (t * usage.raw_max_tokens as f64).round() as u64
        } else {
            t.round() as u64
        }
    });
    ContextBreakdown {
        rows,
        used: usage.total_tokens,
        size: usage.raw_max_tokens,
        auto_compact_at,
        model: usage.model.clone(),
        exact,
        at: now,
        parts: context_parts(usage),
    }
}

/// What the rows of a `get_context_usage` answer consist of, by sort.
fn context_parts(usage: &ContextUsage) -> Vec<ContextPart> {
    let part = |group, name: &str, tokens| ContextPart {
        group,
        name: name.to_string(),
        tokens,
    };
    let mut parts: Vec<ContextPart> = Vec::new();
    parts.extend(
        usage
            .memory_files
            .iter()
            .map(|f| part(ContextGroup::MemoryFile, &f.path, f.tokens)),
    );
    parts.extend(usage.mcp_tools.iter().map(|t| {
        part(
            ContextGroup::McpTool,
            &format!("{}: {}", t.server_name, t.name),
            t.tokens,
        )
    }));
    parts.extend(
        usage
            .agents
            .iter()
            .map(|a| part(ContextGroup::Agent, &a.agent_type, a.tokens)),
    );
    if let Some(skills) = &usage.skills {
        parts.extend(
            skills
                .skill_frontmatter
                .iter()
                .map(|s| part(ContextGroup::Skill, &s.name, s.tokens)),
        );
    }
    parts.extend(
        usage
            .system_tools
            .iter()
            .flatten()
            .map(|t| part(ContextGroup::SystemTool, &t.name, t.tokens)),
    );
    if let Some(m) = &usage.message_breakdown {
        for (name, tokens) in [
            ("tool_calls", m.tool_call_tokens),
            ("tool_results", m.tool_result_tokens),
            ("attachments", m.attachment_tokens),
            ("assistant_messages", m.assistant_message_tokens),
            ("user_messages", m.user_message_tokens),
        ] {
            if tokens > 0 {
                parts.push(part(ContextGroup::Message, name, tokens));
            }
        }
    }
    parts
}

fn context_used(tokens: u64) -> Update {
    Update::Usage(UsagePatch {
        context_used: Some(tokens),
        ..UsagePatch::default()
    })
}

fn status_frame(
    status: Option<&str>,
    mode: Option<&String>,
    compact_failed: bool,
    compact_error: &Option<String>,
    now: SystemTime,
) -> Vec<Update> {
    let mut out = vec![Update::Activity(match status {
        Some("compacting") => Some(ide_thread::Activity::Compacting),
        Some("requesting") => Some(ide_thread::Activity::Requesting),
        _ => None,
    })];
    if let Some(mode) = mode {
        out.push(Update::Mode(mode.clone()));
    }
    if compact_failed {
        out.push(notice(
            now,
            Notice::Error {
                kind: ErrorKind::Compaction,
                message: compact_error.clone().unwrap_or_default(),
            },
        ));
    }
    out
}

fn rate_limit(info: &RateLimitInfo, now: SystemTime) -> Vec<Update> {
    let status = match info.status.as_str() {
        "allowed" => LimitStatus::Allowed,
        "rejected" => LimitStatus::Rejected,
        _ => LimitStatus::Warning,
    };
    let limit = RateLimit {
        window: info.rate_limit_type.clone().unwrap_or_else(|| "unknown".into()),
        status,
        utilization: info.utilization,
        resets_at: info.resets_at.and_then(|s| seconds(s).map(|d| UNIX_EPOCH + d)),
    };
    let mut out = vec![Update::RateLimit(limit.clone())];
    if status != LimitStatus::Allowed {
        out.push(put(
            &Lane::Main,
            Some(format!("limit:{}:{}", limit.window, info.status)),
            now,
            EntryKind::Notice(Notice::RateLimit(limit)),
        ));
    }
    out
}

fn hook_notice(hook: &Hook, now: SystemTime) -> Update {
    let outcome = match hook.outcome.as_deref() {
        None => HookOutcome::Running,
        Some("success") => HookOutcome::Success,
        Some("cancelled") => HookOutcome::Cancelled,
        Some(_) => HookOutcome::Error,
    };
    let output = if hook.output.is_empty() {
        hook.stderr.clone()
    } else {
        hook.output.clone()
    };
    put(
        &Lane::Main,
        Some(format!("hook:{}", hook.hook_id)),
        now,
        EntryKind::Notice(Notice::Hook {
            id: hook.hook_id.clone(),
            name: hook.hook_name.clone(),
            event: hook.hook_event.clone(),
            output,
            exit_code: hook.exit_code,
            outcome,
        }),
    )
}

fn question(call: &str, input: &Value, request: Option<&str>) -> EntryKind {
    EntryKind::Question(Question {
        request_id: request.map(str::to_string),
        tool_call: call.to_string(),
        items: questions(input),
        state: RequestState::Pending,
    })
}

fn plan(call: &str, input: &Value, request: Option<&str>) -> EntryKind {
    EntryKind::PlanApproval(PlanApproval {
        request_id: request.map(str::to_string),
        tool_call: call.to_string(),
        plan: input
            .get("plan")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        state: RequestState::Pending,
    })
}

fn permission_request(request_id: &str, ask: &PermissionAsk) -> EntryKind {
    let mut options = vec![PermissionOption::AllowOnce];
    if !ask.suppress_always_allow_rule {
        options.extend(ask.permission_suggestions.iter().enumerate().filter_map(|(n, raw)| {
            let suggestion = Suggestion::read(raw)?;
            Some(PermissionOption::AllowAlways {
                suggestion: n,
                rule: suggestion.rule,
                scope: suggestion.scope,
            })
        }));
    }
    options.push(PermissionOption::Deny);
    EntryKind::PermissionRequest(PermissionRequest {
        request_id: request_id.to_string(),
        tool_call: Some(ask.tool_use_id.clone()).filter(|c| !c.is_empty()),
        tool_name: ask.tool_name.clone(),
        title: ask.title.clone().or_else(|| ask.display_name.clone()),
        description: ask.description.clone(),
        reason: ask.decision_reason.clone(),
        blocked_path: ask.blocked_path.clone(),
        input: ask.input.clone(),
        options,
        default_to_no: ask.default_to_no,
        state: RequestState::Pending,
    })
}

/// The start of the CLI's refusal of a guarded rewind; the refusal is deterministic, so the session is resumed
/// without the fork instead of trying again.
const REWIND_REJECTED: &str = "Resume rejected by --resume-drops-turn:";

/// The CLI's explanation when `r` refuses a guarded rewind.
pub(crate) fn rewind_rejected(r: &TurnResult) -> Option<String> {
    r.errors
        .iter()
        .flatten()
        .chain(r.result.iter())
        .find(|message| message.starts_with(REWIND_REJECTED))
        .cloned()
}

/// The CLI's own words when `a` reports a missing or refused sign-in.
pub(crate) fn sign_in_problem(a: &Assistant) -> Option<String> {
    let error = a.error.as_deref()?;
    if !AUTH_ERRORS.contains(&error) {
        return None;
    }
    let text: Vec<&str> = a
        .message
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    Some(if text.is_empty() {
        error.to_string()
    } else {
        text.join("\n")
    })
}

/// The picture a user frame carries, as an attachment.
pub(crate) fn image(source: &ImageSource) -> Option<ide_thread::Attachment> {
    use base64::Engine;
    (source.kind == "base64").then_some(())?;
    Some(ide_thread::Attachment::Image {
        name: None,
        media_type: source.media_type.clone(),
        data: base64::engine::general_purpose::STANDARD
            .decode(&source.data)
            .ok()?
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_texts(updates: &[Update]) -> Vec<String> {
        updates
            .iter()
            .filter_map(|u| match u {
                Update::Put {
                    entry: EntryKind::UserMessage(m),
                    ..
                } => Some(m.text.clone()),
                _ => None,
            })
            .collect()
    }

    fn echo(mapper: &mut Mapper, frame: &str) -> Vec<String> {
        let message = crate::protocol::parse(frame).unwrap();
        user_texts(&mapper.map(&message, SystemTime::UNIX_EPOCH))
    }

    #[test]
    fn only_echoes_of_prompts_typed_elsewhere_become_user_messages() {
        let mut mapper = Mapper::new();
        let own = UserMessage {
            text: "from the app".into(),
            ..UserMessage::default()
        };
        assert_eq!(
            user_texts(&mapper.user_message(own, "u-1", SystemTime::UNIX_EPOCH)),
            ["from the app"]
        );
        let replay = |uuid: &str, extra: &str, content: &str| {
            format!(
                r#"{{"type":"user","uuid":"{uuid}","isReplay":true,{extra}"message":{{"role":"user","content":"{content}"}}}}"#
            )
        };
        assert!(echo(&mut mapper, &replay("u-1", "", "from the app")).is_empty());
        assert!(echo(&mut mapper, &replay("s-1", r#""isSynthetic":true,"#, "made up")).is_empty());
        assert!(echo(&mut mapper, &replay("c-1", "", "<bash-stdout>ls</bash-stdout>")).is_empty());
        let task = replay("t-1", r#""origin":{"kind":"task-notification"},"#, "done");
        assert!(echo(&mut mapper, &task).is_empty());
        let phone = replay("p-1", r#""origin":{"kind":"human"},"#, "from the phone");
        assert_eq!(echo(&mut mapper, &phone), ["from the phone"]);
        assert_eq!(mapper.prompts(), 2);
        let mut history = Mapper::for_history();
        assert!(echo(&mut history, &replay("p-2", "", "from the phone")).is_empty());
    }

    #[test]
    fn background_work_still_running_when_the_cli_ends_is_stopped_then() {
        let mut mapper = Mapper::new();
        let set = r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"b1","task_type":"local_bash","description":"loop"}],"session_id":"s","uuid":"u"}"#;
        mapper.map(&crate::protocol::parse(set).unwrap(), SystemTime::UNIX_EPOCH);
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(7);
        let updates = mapper.ended(at);
        let stopped = updates.iter().position(|u| {
            matches!(u, Update::BackgroundItem(BackgroundPatch { id, status: Some(TaskStatus::Stopped), ended: Some(when), .. })
                if id == "b1" && *when == at)
        });
        let emptied = updates
            .iter()
            .position(|u| matches!(u, Update::Background(items) if items.is_empty()));
        assert!(stopped.zip(emptied).is_some_and(|(s, e)| s < e), "{updates:?}");
    }

    fn statuses(mapper: &mut Mapper, frame: &str) -> Vec<Status> {
        let message = crate::protocol::parse(frame).unwrap();
        mapper
            .map(&message, SystemTime::UNIX_EPOCH)
            .into_iter()
            .filter_map(|u| match u {
                Update::Status(status) => Some(status),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_turn_the_cli_starts_after_a_background_task_runs() {
        let start = |parent: &str| {
            format!(
                r#"{{"type":"stream_event","event":{{"type":"message_start","message":{{"id":"m","type":"message","role":"assistant","content":[]}}}},"parent_tool_use_id":{parent},"session_id":"s","uuid":"u"}}"#
            )
        };
        let result = r#"{"type":"result","subtype":"success","is_error":false,"session_id":"s"}"#;
        let mut mapper = Mapper::new();
        mapper.user_message(UserMessage::default(), "u-1", SystemTime::UNIX_EPOCH);
        assert!(statuses(&mut mapper, &start("null")).is_empty());
        assert_eq!(statuses(&mut mapper, result), [Status::Idle]);
        assert!(statuses(&mut mapper, &start(r#""toolu_bg""#)).is_empty());
        assert_eq!(statuses(&mut mapper, &start("null")), [Status::Running]);
        assert!(statuses(&mut mapper, &start("null")).is_empty());
        assert_eq!(statuses(&mut mapper, result), [Status::Idle]);
        let mut history = Mapper::for_history();
        assert!(statuses(&mut history, &start("null")).is_empty());
    }
}
