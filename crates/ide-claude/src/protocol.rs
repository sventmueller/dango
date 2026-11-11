//! The frames the CLI writes on stdout, one JSON object per line, as `@anthropic-ai/claude-agent-sdk` types them.
//!
//! Only the fields the app uses are typed. A frame whose type or subtype is unknown, or whose known fields do not
//! fit, becomes `Message::Unknown` with the raw JSON; the CLI adds types and fields over time and readers are to
//! ignore what they do not know.

use std::collections::HashMap;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// One frame from the CLI.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A complete content block of an assistant message.
    Assistant(Assistant),
    /// A user-role message the CLI adds, chiefly tool results.
    User(User),
    /// The end of a turn.
    Result(TurnResult),
    /// A `system` frame.
    System(System),
    /// A streaming event of the message being generated.
    StreamEvent(StreamEvent),
    /// A tool is still running.
    ToolProgress(ToolProgress),
    /// A one-line summary of a group of tool calls.
    ToolUseSummary(ToolUseSummary),
    /// The plan's rate-limit state changed.
    RateLimit(RateLimitInfo),
    /// The state of a sign-in the CLI performs.
    AuthStatus(AuthStatus),
    /// The conversation was discarded, as by `/clear`.
    ConversationReset(ConversationReset),
    /// A predicted next prompt.
    PromptSuggestion(String),
    /// A heartbeat.
    KeepAlive,
    /// A control request from the CLI; `control::Incoming` reads its body.
    ControlRequest {
        /// The id the answer echoes.
        request_id: String,
        /// The request body with its `subtype`.
        request: Value,
    },
    /// The answer to one of the app's control requests.
    ControlResponse(ControlResponse),
    /// The CLI no longer needs the answer to one of its requests.
    ControlCancel {
        /// The request withdrawn.
        request_id: String,
    },
    /// A frame the app does not know, kept as it came.
    Unknown(Value),
}

/// A line that is not a JSON object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotJson(pub String);

/// Reads one line of the CLI's output.
pub fn parse(line: &str) -> Result<Message, NotJson> {
    let value: Value = serde_json::from_str(line).map_err(|e| NotJson(e.to_string()))?;
    if !value.is_object() {
        return Err(NotJson("not an object".into()));
    }
    Ok(Message::from_value(value))
}

impl Message {
    /// The frame `value` holds.
    pub fn from_value(value: Value) -> Message {
        let kind = value.get("type").and_then(Value::as_str).unwrap_or_default();
        let parsed = match kind {
            "assistant" => typed(&value).map(Message::Assistant),
            "user" => typed(&value).map(Message::User),
            "result" => typed(&value).map(Message::Result),
            "system" => System::read(&value).map(Message::System),
            "stream_event" => typed(&value).map(Message::StreamEvent),
            "tool_progress" => typed(&value).map(Message::ToolProgress),
            "tool_use_summary" => typed(&value).map(Message::ToolUseSummary),
            "rate_limit_event" => value.get("rate_limit_info").and_then(typed).map(Message::RateLimit),
            "auth_status" => typed(&value).map(Message::AuthStatus),
            "conversation_reset" => typed(&value).map(Message::ConversationReset),
            "prompt_suggestion" => text(&value, "suggestion").map(Message::PromptSuggestion),
            "keep_alive" => Some(Message::KeepAlive),
            "control_request" => text(&value, "request_id").and_then(|request_id| {
                let request = value.get("request").filter(|r| r.is_object())?.clone();
                Some(Message::ControlRequest { request_id, request })
            }),
            "control_response" => value.get("response").and_then(typed).map(Message::ControlResponse),
            "control_cancel_request" => {
                text(&value, "request_id").map(|request_id| Message::ControlCancel { request_id })
            }
            _ => None,
        };
        parsed.unwrap_or(Message::Unknown(value))
    }
}

pub(crate) fn typed<T: DeserializeOwned>(value: &Value) -> Option<T> {
    T::deserialize(value).ok()
}

fn text(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

/// A message of the Messages API: id, model, content blocks and usage.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct ApiMessage {
    /// Its id; consecutive assistant frames of one streamed response share it.
    pub id: String,
    /// The model that wrote it, `<synthetic>` for messages the CLI made up itself.
    pub model: Option<String>,
    /// Its content blocks; a streamed assistant frame carries just one.
    pub content: Vec<Block>,
    /// Why the response stopped; null on streamed frames.
    pub stop_reason: Option<String>,
    /// Token counts; not final on streamed frames.
    pub usage: Option<ApiUsage>,
}

/// Token counts of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ApiUsage {
    /// Input tokens not read from or written to the cache.
    pub input_tokens: Option<u64>,
    /// Output tokens.
    pub output_tokens: Option<u64>,
    /// Input tokens written to the cache.
    pub cache_creation_input_tokens: Option<u64>,
    /// Input tokens read from the cache.
    pub cache_read_input_tokens: Option<u64>,
}

impl ApiUsage {
    /// All input tokens, cached or not: what the context window holds before the output.
    pub fn context_input(&self) -> u64 {
        self.input_tokens.unwrap_or(0)
            + self.cache_creation_input_tokens.unwrap_or(0)
            + self.cache_read_input_tokens.unwrap_or(0)
    }
}

/// A content block.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// Text.
    Text {
        /// The text.
        #[serde(default)]
        text: String,
    },
    /// Reasoning; empty when the CLI omits it.
    Thinking {
        /// The reasoning or its summary.
        #[serde(default)]
        thinking: String,
    },
    /// Reasoning the API keeps encrypted.
    RedactedThinking,
    /// A tool call.
    ToolUse {
        /// The call's id.
        #[serde(default)]
        id: String,
        /// The tool.
        #[serde(default)]
        name: String,
        /// Its input.
        #[serde(default)]
        input: Value,
    },
    /// A tool the API runs itself, such as web search.
    ServerToolUse {
        /// The call's id.
        #[serde(default)]
        id: String,
        /// The tool.
        #[serde(default)]
        name: String,
        /// Its input.
        #[serde(default)]
        input: Value,
    },
    /// The result of a tool call.
    ToolResult {
        /// The call it answers.
        #[serde(default)]
        tool_use_id: String,
        /// A string or a list of blocks.
        #[serde(default)]
        content: Value,
        /// Whether the call failed.
        #[serde(default)]
        is_error: Option<bool>,
    },
    /// An image.
    Image {
        /// Its bytes, when the frame carries them.
        #[serde(default)]
        source: Option<ImageSource>,
    },
    /// Any other block.
    #[serde(other)]
    Other,
}

/// Where an image's bytes are.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ImageSource {
    /// `base64` when `data` holds the bytes.
    #[serde(default, rename = "type")]
    pub kind: String,
    /// Its type, such as `image/png`.
    #[serde(default)]
    pub media_type: String,
    /// The bytes in base64.
    #[serde(default)]
    pub data: String,
}

/// An assistant frame.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Assistant {
    /// The message, with the one block this frame delivers.
    pub message: ApiMessage,
    /// The tool call that started the subagent which wrote it.
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    /// Why the request failed, for messages the CLI made up to report a failure.
    #[serde(default)]
    pub error: Option<String>,
    /// The frame's id.
    #[serde(default)]
    pub uuid: Option<String>,
    /// Whether an interrupt cut the message short.
    #[serde(default)]
    pub aborted: Option<bool>,
    /// Frames this one replaces.
    #[serde(default)]
    pub supersedes: Vec<String>,
}

/// A user-role frame from the CLI.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct User {
    /// The message.
    pub message: UserBody,
    /// The tool call that started the subagent it belongs to.
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    /// Whether the CLI made it up rather than the user typing it.
    #[serde(default, rename = "isSynthetic")]
    pub is_synthetic: Option<bool>,
    /// Whether it echoes a prompt the CLI took in, from the app or from Remote Control.
    #[serde(default, rename = "isReplay")]
    pub is_replay: Option<bool>,
    /// Where an echoed prompt came from, such as `{"kind": "task-notification"}`; none for a person's prompt.
    #[serde(default)]
    pub origin: Option<Value>,
    /// The tool's structured output, shaped per tool.
    #[serde(default, alias = "toolUseResult")]
    pub tool_use_result: Option<Value>,
    /// The frame's id.
    #[serde(default)]
    pub uuid: Option<String>,
}

/// The body of a user-role message.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UserBody {
    /// A string or a list of blocks.
    #[serde(default)]
    pub content: Value,
}

impl UserBody {
    /// The blocks, with a string read as one text block.
    pub fn blocks(&self) -> Vec<Block> {
        match &self.content {
            Value::String(text) => vec![Block::Text { text: text.clone() }],
            Value::Array(items) => items.iter().map(|item| typed(item).unwrap_or(Block::Other)).collect(),
            _ => Vec::new(),
        }
    }
}

/// The `result` frame at the end of a turn.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TurnResult {
    /// `success` or the sort of error.
    pub subtype: String,
    /// Whether the turn failed.
    #[serde(default)]
    pub is_error: bool,
    /// The final answer's text, or the error.
    #[serde(default)]
    pub result: Option<String>,
    /// How long the turn took.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// How many model turns it took.
    #[serde(default)]
    pub num_turns: Option<u64>,
    /// The session's cost so far, in US dollars.
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    /// Token counts of the main loop for this turn.
    #[serde(default)]
    pub usage: Option<ApiUsage>,
    /// Totals per model for the session so far.
    #[serde(default, rename = "modelUsage")]
    pub model_usage: HashMap<String, ModelUsage>,
    /// Errors of a failed turn.
    #[serde(default)]
    pub errors: Option<Vec<String>>,
    /// Why the turn ended.
    #[serde(default)]
    pub terminal_reason: Option<String>,
    /// Why the CLI could not start, when it could not.
    #[serde(default)]
    pub startup_failure_reason: Option<String>,
    /// The session's id.
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Totals for one model.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelUsage {
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Input tokens read from the cache.
    pub cache_read_input_tokens: u64,
    /// Input tokens written to the cache.
    pub cache_creation_input_tokens: u64,
    /// Cost in US dollars.
    #[serde(rename = "costUSD")]
    pub cost_usd: f64,
    /// The model's context window.
    pub context_window: Option<u64>,
}

/// A streaming event, sent with `--include-partial-messages`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StreamEvent {
    /// The Messages API event.
    pub event: RawEvent,
    /// The tool call that started the subagent it belongs to.
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
}

/// A Messages API streaming event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RawEvent {
    /// A message begins.
    MessageStart {
        /// The message so far.
        message: ApiMessage,
    },
    /// A content block begins.
    ContentBlockStart {
        /// Its position in the message.
        index: usize,
        /// The block so far.
        content_block: Block,
    },
    /// More of a content block.
    ContentBlockDelta {
        /// Its position in the message.
        index: usize,
        /// What arrived.
        delta: Delta,
    },
    /// A content block ends.
    ContentBlockStop {
        /// Its position in the message.
        index: usize,
    },
    /// The message's stop reason and usage.
    MessageDelta {
        /// Final token counts.
        #[serde(default)]
        usage: Option<ApiUsage>,
    },
    /// The message ends.
    MessageStop,
    /// Any other event, such as `ping`.
    #[serde(other)]
    Other,
}

/// What a streaming delta carries.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Delta {
    /// Text.
    TextDelta {
        /// The text.
        #[serde(default)]
        text: String,
    },
    /// Reasoning.
    ThinkingDelta {
        /// The reasoning.
        #[serde(default)]
        thinking: String,
    },
    /// Part of a tool call's input as JSON text.
    InputJsonDelta {
        /// The JSON text.
        #[serde(default)]
        partial_json: String,
    },
    /// Anything else, such as signatures.
    #[serde(other)]
    Other,
}

/// A tool is still running.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolProgress {
    /// The call.
    pub tool_use_id: String,
    /// The tool.
    #[serde(default)]
    pub tool_name: String,
    /// The tool call that started the subagent it belongs to.
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    /// How long it has run.
    #[serde(default)]
    pub elapsed_time_seconds: f64,
}

/// A one-line summary of a group of tool calls.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolUseSummary {
    /// The summary.
    pub summary: String,
    /// The calls it sums up.
    #[serde(default)]
    pub preceding_tool_use_ids: Vec<String>,
}

/// The plan's rate-limit state.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitInfo {
    /// `allowed`, `allowed_warning` or `rejected`.
    pub status: String,
    /// When the window resets, in seconds since 1970.
    #[serde(default)]
    pub resets_at: Option<f64>,
    /// Which window, such as `five_hour`.
    #[serde(default)]
    pub rate_limit_type: Option<String>,
    /// The share used, from 0 to 1.
    #[serde(default)]
    pub utilization: Option<f64>,
}

/// The state of a sign-in the CLI performs.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    /// Whether it is still going on.
    #[serde(default)]
    pub is_authenticating: bool,
    /// What it printed.
    #[serde(default)]
    pub output: Vec<String>,
    /// Why it failed.
    #[serde(default)]
    pub error: Option<String>,
}

/// The conversation was discarded.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ConversationReset {
    /// The new conversation's id.
    #[serde(default)]
    pub new_conversation_id: Option<String>,
}

/// The answer to a control request the app sent.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ControlResponse {
    /// `success` or `error`.
    pub subtype: String,
    /// The request answered.
    pub request_id: String,
    /// The success payload.
    #[serde(default)]
    pub response: Option<Value>,
    /// The failure.
    #[serde(default)]
    pub error: Option<String>,
}

/// A `system` frame, by subtype.
#[derive(Debug, Clone, PartialEq)]
pub enum System {
    /// The session's set-up.
    Init(Init),
    /// The CLI's status and permission mode.
    Status {
        /// `compacting`, `requesting` or none.
        status: Option<String>,
        /// The permission mode, when it changed.
        permission_mode: Option<String>,
        /// Whether a compaction failed.
        compact_failed: bool,
        /// Why it failed.
        compact_error: Option<String>,
    },
    /// A request failed and is retried.
    ApiRetry(ApiRetry),
    /// The conversation was compacted.
    CompactBoundary(CompactBoundary),
    /// A task started.
    TaskStarted(TaskStarted),
    /// A task's progress.
    TaskProgress(TaskProgress),
    /// Fields of a task changed.
    TaskUpdated(TaskUpdated),
    /// A task ended.
    TaskNotification(TaskNotification),
    /// The complete set of live background tasks.
    BackgroundTasksChanged(Vec<BackgroundTask>),
    /// A hook started, printed or ended.
    Hook(Hook),
    /// The session's state.
    SessionStateChanged(String),
    /// A text line to show.
    Informational {
        /// The text.
        content: String,
        /// `info`, `notice`, `suggestion` or `warning`.
        level: String,
    },
    /// What a local slash command printed.
    LocalCommandOutput(String),
    /// A notification.
    Notification(String),
    /// A tool call was denied without asking.
    PermissionDenied {
        /// The call.
        tool_use_id: String,
        /// What the model was told.
        message: String,
    },
    /// The model refused and the CLI retried or gave up.
    Refusal {
        /// The text to show.
        content: String,
        /// Frames the CLI took back.
        retracted: Vec<String>,
    },
    /// The slash commands changed.
    CommandsChanged(Vec<Value>),
    /// The disk is full and the transcript is not written.
    WritesLost(String),
    /// The Remote Control connection changed.
    BridgeState {
        /// `ready`, `connected`, `failed`, `policy_disabled` or another the app does not know.
        state: String,
        /// Which connection it speaks of.
        epoch: Option<u64>,
        /// Why it failed.
        detail: Option<String>,
    },
    /// Any other subtype.
    Unknown(String),
}

impl System {
    fn read(value: &Value) -> Option<System> {
        let subtype = value.get("subtype").and_then(Value::as_str).unwrap_or_default();
        let field = |name: &str| text(value, name);
        Some(match subtype {
            "init" => System::Init(typed(value)?),
            "status" => System::Status {
                status: field("status"),
                permission_mode: field("permissionMode"),
                compact_failed: field("compact_result").as_deref() == Some("failed"),
                compact_error: field("compact_error"),
            },
            "api_retry" => System::ApiRetry(typed(value)?),
            "compact_boundary" => System::CompactBoundary(typed(value.get("compact_metadata")?)?),
            "task_started" => System::TaskStarted(typed(value)?),
            "task_progress" => System::TaskProgress(typed(value)?),
            "task_updated" => System::TaskUpdated(typed(value)?),
            "task_notification" => System::TaskNotification(typed(value)?),
            "background_tasks_changed" => System::BackgroundTasksChanged(typed(value.get("tasks")?)?),
            "hook_started" | "hook_progress" | "hook_response" => {
                let mut hook: Hook = typed(value)?;
                if subtype == "hook_started" {
                    hook.outcome = None;
                } else if subtype == "hook_progress" {
                    hook.outcome = None;
                    hook.running = true;
                }
                System::Hook(hook)
            }
            "session_state_changed" => System::SessionStateChanged(field("state")?),
            "informational" => System::Informational {
                content: field("content")?,
                level: field("level").unwrap_or_default(),
            },
            "local_command_output" => System::LocalCommandOutput(field("content")?),
            "notification" => System::Notification(field("text")?),
            "permission_denied" => System::PermissionDenied {
                tool_use_id: field("tool_use_id")?,
                message: field("message").unwrap_or_default(),
            },
            "model_refusal_fallback" | "model_refusal_no_fallback" => System::Refusal {
                content: field("content").unwrap_or_default(),
                retracted: value.get("retracted_message_uuids").and_then(typed).unwrap_or_default(),
            },
            "commands_changed" => System::CommandsChanged(typed(value.get("commands")?)?),
            "session_writes_lost" => System::WritesLost(field("error_code").unwrap_or_default()),
            "bridge_state" => System::BridgeState {
                state: field("state")?,
                epoch: value.get("bridge_epoch").and_then(Value::as_u64),
                detail: field("detail"),
            },
            other => System::Unknown(other.to_string()),
        })
    }
}

/// The session's set-up, sent at the start of each turn.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Init {
    /// The session's id.
    pub session_id: String,
    /// The working folder.
    pub cwd: String,
    /// The model.
    pub model: String,
    /// The permission mode.
    #[serde(rename = "permissionMode")]
    pub permission_mode: String,
    /// The tools the model can call.
    pub tools: Vec<String>,
    /// The MCP servers and their state.
    pub mcp_servers: Vec<McpServer>,
    /// The slash commands.
    pub slash_commands: Vec<String>,
    /// The subagent types.
    pub agents: Vec<String>,
    /// The skills.
    pub skills: Vec<String>,
    /// The CLI's version.
    pub claude_code_version: String,
    /// The output style.
    pub output_style: String,
    /// Protocol capabilities, such as `interrupt_receipt_v1`.
    pub capabilities: Vec<String>,
    /// The effort for the next message.
    pub effort: Option<String>,
    /// Where an API key came from, `none` with a subscription sign-in.
    #[serde(rename = "apiKeySource")]
    pub api_key_source: Option<String>,
}

/// An MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct McpServer {
    /// Its name.
    pub name: String,
    /// `connected`, `pending`, `failed` or another state.
    pub status: String,
}

/// A request failed and is retried.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApiRetry {
    /// The attempt that failed.
    #[serde(default)]
    pub attempt: u32,
    /// How many attempts are made at most.
    #[serde(default)]
    pub max_retries: u32,
    /// How long the CLI waits.
    #[serde(default)]
    pub retry_delay_ms: u64,
    /// The HTTP status, when there was a response.
    #[serde(default)]
    pub error_status: Option<u16>,
    /// The CLI's name for the error.
    #[serde(default)]
    pub error: String,
}

/// What a compaction did.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CompactBoundary {
    /// `manual` or `auto`.
    #[serde(default)]
    pub trigger: String,
    /// Tokens before.
    #[serde(default)]
    pub pre_tokens: u64,
    /// Tokens after.
    #[serde(default)]
    pub post_tokens: Option<u64>,
}

/// A task started: a subagent, a shell, a monitor or a workflow.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TaskStarted {
    /// The task's id.
    pub task_id: String,
    /// The id of this run of it.
    #[serde(default)]
    pub run_id: Option<String>,
    /// The tool call that started it.
    #[serde(default)]
    pub tool_use_id: Option<String>,
    /// What it does.
    #[serde(default)]
    pub description: String,
    /// The sort of subagent.
    #[serde(default)]
    pub subagent_type: Option<String>,
    /// Whether it started in the background.
    #[serde(default)]
    pub is_backgrounded: Option<bool>,
    /// The subagent task that started it.
    #[serde(default)]
    pub parent_task_id: Option<String>,
    /// `local_agent`, `local_bash`, `monitor`, `local_workflow` or another.
    #[serde(default)]
    pub task_type: Option<String>,
    /// Whether it stays out of the transcript.
    #[serde(default)]
    pub skip_transcript: Option<bool>,
    /// Whether it is housekeeping.
    #[serde(default)]
    pub ambient: Option<bool>,
}

/// Token, tool and time counts of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TaskUsage {
    /// Tokens used.
    pub total_tokens: u64,
    /// Tools called.
    pub tool_uses: u64,
    /// Time taken.
    pub duration_ms: u64,
}

/// A task's progress.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TaskProgress {
    /// The task.
    pub task_id: String,
    /// The run.
    #[serde(default)]
    pub run_id: Option<String>,
    /// The tool call that started it.
    #[serde(default)]
    pub tool_use_id: Option<String>,
    /// What it does now.
    #[serde(default)]
    pub description: Option<String>,
    /// Counts so far.
    #[serde(default)]
    pub usage: TaskUsage,
    /// The tool it called last.
    #[serde(default)]
    pub last_tool_name: Option<String>,
    /// A one-line status.
    #[serde(default)]
    pub summary: Option<String>,
}

/// Fields of a task that changed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TaskUpdated {
    /// The task.
    pub task_id: String,
    /// The run.
    #[serde(default)]
    pub run_id: Option<String>,
    /// What changed.
    #[serde(default)]
    pub patch: TaskPatch,
}

/// The changed fields of a task.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct TaskPatch {
    /// `pending`, `running`, `completed`, `failed`, `killed` or `paused`.
    pub status: Option<String>,
    /// What it does.
    pub description: Option<String>,
    /// When it ended, in milliseconds since 1970.
    pub end_time: Option<f64>,
    /// Why it failed.
    pub error: Option<String>,
    /// Whether it runs in the background.
    pub is_backgrounded: Option<bool>,
}

/// A task ended.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TaskNotification {
    /// The task.
    pub task_id: String,
    /// The run.
    #[serde(default)]
    pub run_id: Option<String>,
    /// The tool call that started it.
    #[serde(default)]
    pub tool_use_id: Option<String>,
    /// `completed`, `failed` or `stopped`.
    #[serde(default)]
    pub status: String,
    /// The file its output went to.
    #[serde(default)]
    pub output_file: Option<String>,
    /// A summary of what it did.
    #[serde(default)]
    pub summary: Option<String>,
    /// Its final counts.
    #[serde(default)]
    pub usage: Option<TaskUsage>,
}

/// A live background task.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BackgroundTask {
    /// The task.
    pub task_id: String,
    /// The run.
    #[serde(default)]
    pub run_id: Option<String>,
    /// The sort of task.
    #[serde(default)]
    pub task_type: String,
    /// What it does.
    #[serde(default)]
    pub description: String,
    /// The subagent task that started it.
    #[serde(default)]
    pub parent_task_id: Option<String>,
    /// Whether it is housekeeping.
    #[serde(default)]
    pub ambient: Option<bool>,
}

/// A hook run, from any of its three frames.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Hook {
    /// The run's id.
    pub hook_id: String,
    /// The hook.
    #[serde(default)]
    pub hook_name: String,
    /// The event it runs for.
    #[serde(default)]
    pub hook_event: String,
    /// Its combined output.
    #[serde(default)]
    pub output: String,
    /// Its standard error.
    #[serde(default)]
    pub stderr: String,
    /// Its exit code.
    #[serde(default)]
    pub exit_code: Option<i32>,
    /// `success`, `error` or `cancelled` once it ended.
    #[serde(default)]
    pub outcome: Option<String>,
    /// Whether this frame reports output of a hook still running.
    #[serde(skip)]
    pub running: bool,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_line_that_is_not_a_json_object_is_refused() {
        assert!(parse("not json").is_err());
        assert!(parse("[1, 2]").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn unknown_types_and_subtypes_are_kept_raw() {
        let Message::Unknown(raw) = parse(r#"{"type":"warp","x":1}"#).unwrap() else {
            panic!()
        };
        assert_eq!(raw["x"], 1);
        let message = parse(r#"{"type":"system","subtype":"brand_new","session_id":"s"}"#).unwrap();
        assert_eq!(message, Message::System(System::Unknown("brand_new".into())));
    }

    #[test]
    fn a_known_type_that_does_not_fit_is_kept_raw_instead_of_failing() {
        let line = r#"{"type":"system","subtype":"task_progress","task_id":7}"#;
        assert!(matches!(parse(line).unwrap(), Message::Unknown(_)));
        let line = r#"{"type":"control_request","request_id":"r","request":"text"}"#;
        assert!(matches!(parse(line).unwrap(), Message::Unknown(_)));
    }

    #[test]
    fn extra_fields_and_unknown_blocks_do_not_stop_an_assistant_frame() {
        let frame = json!({
            "type": "assistant", "parent_tool_use_id": null, "new_field": {"a": 1},
            "message": {"id": "m", "content": [
                {"type": "redacted_thinking", "data": "xyz"},
                {"type": "hologram"},
                {"type": "text", "text": "hi", "citations": []}
            ]}
        });
        let Message::Assistant(a) = Message::from_value(frame) else {
            panic!()
        };
        assert_eq!(
            a.message.content,
            [Block::RedactedThinking, Block::Other, Block::Text { text: "hi".into() }]
        );
    }

    #[test]
    fn user_content_may_be_a_string_or_blocks() {
        let text = UserBody {
            content: json!("plain"),
        };
        assert_eq!(text.blocks(), [Block::Text { text: "plain".into() }]);
        let blocks = UserBody {
            content: json!([{"type": "tool_result", "tool_use_id": "t", "content": "ok"}, {"type": "new_kind"}]),
        };
        assert!(matches!(&blocks.blocks()[0], Block::ToolResult { tool_use_id, .. } if tool_use_id == "t"));
        assert_eq!(blocks.blocks()[1], Block::Other);
    }

    #[test]
    fn control_frames_are_told_apart() {
        let request = parse(r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool"}}"#);
        assert!(matches!(request.unwrap(), Message::ControlRequest { request_id, .. } if request_id == "r1"));
        let response =
            parse(r#"{"type":"control_response","response":{"subtype":"error","request_id":"r2","error":"no"}}"#);
        let Message::ControlResponse(r) = response.unwrap() else {
            panic!()
        };
        assert_eq!((r.subtype.as_str(), r.error.as_deref()), ("error", Some("no")));
        let cancel = parse(r#"{"type":"control_cancel_request","request_id":"r3"}"#).unwrap();
        assert_eq!(
            cancel,
            Message::ControlCancel {
                request_id: "r3".into()
            }
        );
        assert_eq!(parse(r#"{"type":"keep_alive"}"#).unwrap(), Message::KeepAlive);
    }

    #[test]
    fn hook_frames_tell_running_from_finished() {
        let started =
            parse(r#"{"type":"system","subtype":"hook_started","hook_id":"h","hook_name":"n","hook_event":"e"}"#);
        let Message::System(System::Hook(hook)) = started.unwrap() else {
            panic!()
        };
        assert_eq!((hook.outcome, hook.running), (None, false));
        let done =
            parse(r#"{"type":"system","subtype":"hook_response","hook_id":"h","outcome":"success","exit_code":0}"#);
        let Message::System(System::Hook(hook)) = done.unwrap() else {
            panic!()
        };
        assert_eq!(hook.outcome.as_deref(), Some("success"));
    }
}
