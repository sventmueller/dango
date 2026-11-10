//! How the agent's session updates change the thread (E4); `docs/acp.md` has the table this follows. Streamed text
//! goes into one entry until something else arrives; tool calls are keyed by the agent's id for them; what the
//! thread model has no place for is ignored, never guessed at.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use agent_client_protocol::schema::v1 as acp;
use ide_thread::{
    AgentMessage, EntryKind, ErrorKind, Lane, Notice, PermissionOption, PermissionRequest, RequestState, Status,
    TextEdit, Thought, Todo, TodoStatus, ToolCall, ToolKind, ToolOutput, ToolPatch, ToolStatus, ToolView, Update,
    UsagePatch, UserMessage,
};
use serde_json::Value;

use crate::capabilities::{Capabilities, Category};
use crate::diff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Streaming {
    Message,
    Thought,
}

/// Counts mappers, so that one taking over a thread another one filled never reuses its keys.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Turns what the agent reports into changes of the thread.
#[derive(Debug, Default)]
pub struct Mapper {
    /// This mapper's part of every key.
    generation: u64,
    /// The entry streaming text now: its key, what it is and the agent's id for the message.
    open: Option<(String, Streaming, Option<String>)>,
    /// A user message the agent replays while loading a session: its key and its text so far.
    user: Option<(String, String)>,
    next: u64,
    turn: u64,
    /// The tool calls so far by the agent's id: their name, title and input, which a permission request may leave out.
    tools: HashMap<String, (String, String, Value)>,
}

fn put(key: String, at: SystemTime, entry: EntryKind) -> Update {
    Update::Put {
        lane: Lane::Main,
        key: Some(key),
        at,
        entry,
    }
}

/// What a content block reads as in markdown; images and audio have no text.
fn block_text(block: &acp::ContentBlock) -> Option<String> {
    match block {
        acp::ContentBlock::Text(text) => Some(text.text.clone()),
        acp::ContentBlock::ResourceLink(link) => Some(format!(
            "[{}]({})",
            link.title.as_deref().unwrap_or(&link.name),
            link.uri
        )),
        acp::ContentBlock::Resource(resource) => match &resource.resource {
            acp::EmbeddedResourceResource::TextResourceContents(text) => Some(format!("[{0}]({0})", text.uri)),
            _ => None,
        },
        _ => None,
    }
}

fn tool_kind(kind: &acp::ToolKind) -> ToolKind {
    match kind {
        acp::ToolKind::Read => ToolKind::Read,
        acp::ToolKind::Edit => ToolKind::Edit,
        acp::ToolKind::Delete => ToolKind::Delete,
        acp::ToolKind::Move => ToolKind::Move,
        acp::ToolKind::Search => ToolKind::Search,
        acp::ToolKind::Execute => ToolKind::Execute,
        acp::ToolKind::Think => ToolKind::Think,
        acp::ToolKind::Fetch => ToolKind::Fetch,
        acp::ToolKind::SwitchMode => ToolKind::SwitchMode,
        _ => ToolKind::Other,
    }
}

fn tool_status(status: &acp::ToolCallStatus) -> ToolStatus {
    match status {
        acp::ToolCallStatus::InProgress => ToolStatus::InProgress,
        acp::ToolCallStatus::Completed => ToolStatus::Completed,
        acp::ToolCallStatus::Failed => ToolStatus::Failed,
        _ => ToolStatus::Pending,
    }
}

fn field<'a>(input: &'a Value, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|n| input.get(*n).and_then(Value::as_str))
}

/// The input field by field, as far as the kind and the input say it.
fn view(
    kind: ToolKind,
    input: &Value,
    content: &[acp::ToolCallContent],
    locations: &[acp::ToolCallLocation],
) -> ToolView {
    let diff = content.iter().find_map(|c| match c {
        acp::ToolCallContent::Diff(d) => Some(d),
        _ => None,
    });
    if let Some(diff) = diff {
        let path = diff.path.to_string_lossy().into_owned();
        return match &diff.old_text {
            Some(old) => ToolView::Edit {
                path,
                edits: vec![TextEdit {
                    old: old.clone(),
                    new: diff.new_text.clone(),
                    replace_all: false,
                }],
            },
            None => ToolView::Write {
                path,
                content: diff.new_text.clone(),
            },
        };
    }
    let location = locations.first();
    match kind {
        ToolKind::Execute => {
            let command = match input.get("command") {
                Some(Value::String(c)) => Some(c.clone()),
                Some(Value::Array(parts)) => Some(parts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")),
                _ => None,
            };
            command.map_or(ToolView::Other, |command| ToolView::Command {
                command,
                description: field(input, &["description"]).map(str::to_string),
                background: false,
                timeout: None,
            })
        }
        ToolKind::Read => field(input, &["path", "file_path", "abs_path"])
            .map(str::to_string)
            .or_else(|| location.map(|l| l.path.to_string_lossy().into_owned()))
            .map_or(ToolView::Other, |path| ToolView::Read {
                path,
                offset: location.and_then(|l| l.line).map(u64::from),
                limit: None,
            }),
        ToolKind::Search => field(input, &["pattern", "query"]).map_or(ToolView::Other, |pattern| ToolView::Search {
            pattern: pattern.to_string(),
            path: field(input, &["path"]).map(str::to_string),
            glob: field(input, &["glob", "include"]).map(str::to_string),
            results: None,
        }),
        ToolKind::Fetch => field(input, &["url"]).map_or(ToolView::Other, |url| ToolView::Fetch {
            url: url.to_string(),
            prompt: field(input, &["prompt"]).map(str::to_string),
        }),
        _ => ToolView::Other,
    }
}

/// What the call returned: its diff, else its text, else its raw output.
fn output(content: &[acp::ToolCallContent], raw: Option<&Value>) -> Option<ToolOutput> {
    for item in content {
        if let acp::ToolCallContent::Diff(d) = item {
            return Some(ToolOutput::Diff {
                path: d.path.to_string_lossy().into_owned(),
                hunks: diff::hunks(d.old_text.as_deref().unwrap_or_default(), &d.new_text),
            });
        }
    }
    let text: Vec<String> = content
        .iter()
        .filter_map(|c| match c {
            acp::ToolCallContent::Content(c) => block_text(&c.content),
            _ => None,
        })
        .collect();
    if !text.is_empty() {
        return Some(ToolOutput::Text(text.join("\n")));
    }
    match raw? {
        Value::String(s) => Some(ToolOutput::Text(s.clone())),
        Value::Object(map) if map.contains_key("stdout") || map.contains_key("stderr") => Some(ToolOutput::Terminal {
            stdout: map
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            stderr: map
                .get("stderr")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            interrupted: false,
        }),
        _ => None,
    }
}

fn todos(plan: &acp::Plan) -> Vec<Todo> {
    plan.entries
        .iter()
        .map(|e| Todo {
            content: e.content.clone(),
            active_form: None,
            status: match e.status {
                acp::PlanEntryStatus::InProgress => TodoStatus::InProgress,
                acp::PlanEntryStatus::Completed => TodoStatus::Completed,
                _ => TodoStatus::Pending,
            },
        })
        .collect()
}

/// The thread model's options for the agent's: allowing once or always, refusing; the agent's own names are kept
/// in the order given, which `option_index` maps back.
pub(crate) fn permission_options(options: &[acp::PermissionOption]) -> Vec<PermissionOption> {
    options
        .iter()
        .enumerate()
        .map(|(n, o)| match o.kind {
            acp::PermissionOptionKind::AllowOnce => PermissionOption::AllowOnce,
            acp::PermissionOptionKind::AllowAlways => PermissionOption::AllowAlways {
                suggestion: n,
                rule: o.name.clone(),
                scope: String::new(),
            },
            _ => PermissionOption::Deny,
        })
        .collect()
}

impl Mapper {
    /// A mapper for a thread, new or one an earlier session filled.
    pub fn new() -> Mapper {
        Mapper {
            generation: GENERATION.fetch_add(1, Ordering::Relaxed) + 1,
            ..Mapper::default()
        }
    }

    fn key(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}-{}-{}", self.generation, self.next)
    }

    /// Ends the text that streams now.
    pub(crate) fn settle(&mut self, at: SystemTime) -> Vec<Update> {
        self.user = None;
        self.open
            .take()
            .map(|(key, ..)| Update::Finish { key, text: None, at })
            .into_iter()
            .collect()
    }

    /// A prompt the user sent, which starts a turn.
    pub(crate) fn begin_turn(&mut self, message: &UserMessage, at: SystemTime) -> Vec<Update> {
        self.turn += 1;
        let mut out = self.settle(at);
        let key = self.key("user");
        out.push(put(key, at, EntryKind::UserMessage(message.clone())));
        out.push(Update::Status(Status::Running));
        out
    }

    /// The turn ended for `reason`, the agent's stop reason as it named it on the wire.
    pub(crate) fn end_turn(&mut self, reason: &str, at: SystemTime) -> Vec<Update> {
        let mut out = self.settle(at);
        out.push(Update::Status(Status::Idle));
        let notice = match reason {
            "end_turn" => None,
            "cancelled" => Some(Notice::Interrupted),
            other => Some(Notice::Error {
                kind: ErrorKind::Turn,
                message: other.to_string(),
            }),
        };
        if let Some(notice) = notice {
            let key = self.key("notice");
            out.push(put(key, at, EntryKind::Notice(notice)));
        }
        out
    }

    /// A failure to show in the thread, such as a prompt the agent rejected.
    pub(crate) fn error(&mut self, kind: ErrorKind, message: String, at: SystemTime) -> Vec<Update> {
        let mut out = self.settle(at);
        let key = self.key("notice");
        out.push(put(key, at, EntryKind::Notice(Notice::Error { kind, message })));
        out
    }

    fn stream(&mut self, kind: Streaming, chunk: &acp::ContentChunk, at: SystemTime) -> Vec<Update> {
        let Some(text) = block_text(&chunk.content) else {
            return Vec::new();
        };
        let id = chunk.message_id.as_ref().map(|m| m.0.to_string());
        let same = matches!(&self.open, Some((_, k, open_id)) if *k == kind && (id.is_none() || *open_id == id));
        let mut out = Vec::new();
        if !same {
            out = self.settle(at);
            let key = self.key(if kind == Streaming::Message {
                "message"
            } else {
                "thought"
            });
            let entry = match kind {
                Streaming::Message => EntryKind::AgentMessage(AgentMessage {
                    markdown: String::new(),
                    streaming: true,
                }),
                Streaming::Thought => EntryKind::Thought(Thought {
                    text: String::new(),
                    duration: None,
                    streaming: true,
                }),
            };
            out.push(put(key.clone(), at, entry));
            self.open = Some((key, kind, id));
        }
        if let Some((key, ..)) = &self.open {
            out.push(Update::Append { key: key.clone(), text });
        }
        out
    }

    fn replayed_user(&mut self, chunk: &acp::ContentChunk, at: SystemTime) -> Vec<Update> {
        let Some(text) = block_text(&chunk.content) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if self.user.is_none() {
            out = self.settle(at);
            let key = self.key("user");
            self.user = Some((key, String::new()));
        }
        let Some((key, so_far)) = self.user.as_mut() else {
            return out;
        };
        so_far.push_str(&text);
        let message = UserMessage {
            text: so_far.clone(),
            ..UserMessage::default()
        };
        out.push(put(key.clone(), at, EntryKind::UserMessage(message)));
        out
    }

    fn tool_call(&mut self, call: &acp::ToolCall, at: SystemTime) -> Vec<Update> {
        let mut out = self.settle(at);
        let id = call.tool_call_id.0.to_string();
        let kind = tool_kind(&call.kind);
        let input = call.raw_input.clone().unwrap_or(Value::Null);
        let status = tool_status(&call.status);
        let name = call.name.clone().unwrap_or_else(|| call.title.clone());
        self.tools
            .insert(id.clone(), (name.clone(), call.title.clone(), input.clone()));
        let mut entry = ToolCall::new(id.clone(), name, kind);
        entry.title = call.title.clone();
        entry.status = status;
        entry.view = view(kind, &input, &call.content, &call.locations);
        entry.output = output(&call.content, call.raw_output.as_ref());
        entry.input = input;
        if status != ToolStatus::Pending {
            entry.started = Some(at);
        }
        if matches!(status, ToolStatus::Completed | ToolStatus::Failed) {
            entry.ended = Some(at);
        }
        out.push(put(id, at, EntryKind::ToolCall(entry)));
        out
    }

    fn tool_update(&mut self, update: &acp::ToolCallUpdate, at: SystemTime) -> Vec<Update> {
        let id = update.tool_call_id.0.to_string();
        let fields = &update.fields;
        if !self.tools.contains_key(&id) {
            let mut call = acp::ToolCall::new(update.tool_call_id.clone(), fields.title.clone().unwrap_or_default());
            call.kind = fields.kind.unwrap_or_default();
            call.status = fields.status.unwrap_or_default();
            call.name.clone_from(&fields.name);
            call.content = fields.content.clone().unwrap_or_default();
            call.locations = fields.locations.clone().unwrap_or_default();
            call.raw_input.clone_from(&fields.raw_input);
            call.raw_output.clone_from(&fields.raw_output);
            return self.tool_call(&call, at);
        }
        let mut out = self.settle(at);
        if let Some(known) = self.tools.get_mut(&id) {
            if let Some(name) = &fields.name {
                known.0.clone_from(name);
            }
            if let Some(title) = &fields.title {
                known.1.clone_from(title);
            }
            if let Some(input) = &fields.raw_input {
                known.2.clone_from(input);
            }
        }
        let status = fields.status.as_ref().map(tool_status);
        let content = fields.content.clone().unwrap_or_default();
        let kind = fields.kind.as_ref().map(tool_kind);
        let input = fields.raw_input.clone();
        let view = (input.is_some() || !content.is_empty()).then(|| {
            view(
                kind.unwrap_or(ToolKind::Other),
                input.as_ref().unwrap_or(&Value::Null),
                &content,
                fields.locations.as_deref().unwrap_or_default(),
            )
        });
        let patch = ToolPatch {
            input,
            title: fields.title.clone(),
            view: view.filter(|v| *v != ToolView::Other),
            status,
            output: output(&content, fields.raw_output.as_ref()),
            started: (status == Some(ToolStatus::InProgress)).then_some(at),
            ended: matches!(status, Some(ToolStatus::Completed | ToolStatus::Failed)).then_some(at),
            ..ToolPatch::default()
        };
        out.push(Update::Tool { key: id, patch });
        out
    }

    /// The agent asks to run a tool call; `request` is the app's id for the question.
    pub(crate) fn permission(
        &mut self,
        request: &str,
        call: &acp::ToolCallUpdate,
        options: &[acp::PermissionOption],
        at: SystemTime,
    ) -> Vec<Update> {
        let mut out = self.tool_update(call, at);
        let fields = &call.fields;
        let id = call.tool_call_id.0.to_string();
        let (name, title, input) = self.tools.get(&id).cloned().unwrap_or_default();
        let entry = PermissionRequest {
            request_id: request.to_string(),
            tool_call: Some(id),
            tool_name: name,
            title: Some(title).filter(|t| !t.is_empty()),
            description: None,
            reason: None,
            blocked_path: fields
                .locations
                .as_ref()
                .and_then(|l| l.first())
                .map(|l| l.path.to_string_lossy().into_owned()),
            input,
            options: permission_options(options),
            default_to_no: false,
            state: RequestState::Pending,
        };
        out.push(put(
            format!("permission-{request}"),
            at,
            EntryKind::PermissionRequest(entry),
        ));
        out.push(Update::Status(Status::RequiresAction));
        out
    }

    /// Maps one session update; `caps` learns what the session now offers, and the flag says whether it changed.
    pub(crate) fn update(
        &mut self,
        update: &acp::SessionUpdate,
        caps: &mut Capabilities,
        at: SystemTime,
    ) -> (Vec<Update>, bool) {
        let updates = match update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => self.stream(Streaming::Message, chunk, at),
            acp::SessionUpdate::AgentThoughtChunk(chunk) => self.stream(Streaming::Thought, chunk, at),
            acp::SessionUpdate::UserMessageChunk(chunk) => self.replayed_user(chunk, at),
            acp::SessionUpdate::ToolCall(call) => self.tool_call(call, at),
            acp::SessionUpdate::ToolCallUpdate(call) => self.tool_update(call, at),
            acp::SessionUpdate::Plan(plan) => {
                let list = todos(plan);
                let key = format!("plan-{}-{}", self.generation, self.turn);
                let mut out = self.settle(at);
                out.push(Update::Todos(list.clone()));
                out.push(put(key, at, EntryKind::Todos(list)));
                out
            }
            acp::SessionUpdate::SessionInfoUpdate(info) => match &info.title {
                agent_client_protocol::schema::MaybeUndefined::Value(title) => vec![Update::Title(title.clone())],
                _ => Vec::new(),
            },
            acp::SessionUpdate::UsageUpdate(usage) => vec![Update::Usage(UsagePatch {
                context_used: Some(usage.used),
                context_size: Some(usage.size),
                cost_usd: usage.cost.as_ref().filter(|c| c.currency == "USD").map(|c| c.amount),
                ..UsagePatch::default()
            })],
            acp::SessionUpdate::AvailableCommandsUpdate(commands) => {
                caps.learn_commands(&commands.available_commands);
                return (Vec::new(), true);
            }
            acp::SessionUpdate::CurrentModeUpdate(mode) => {
                let id = mode.current_mode_id.0.to_string();
                caps.mode = Some(id.clone());
                return (vec![Update::Mode(id)], true);
            }
            acp::SessionUpdate::ConfigOptionUpdate(options) => {
                caps.learn_options(&options.config_options);
                return (self.options_changed(caps), true);
            }
            _ => Vec::new(),
        };
        (updates, false)
    }

    /// The thread's model, thought level and mode as the session's options now set them, by the agent's ids.
    pub(crate) fn options_changed(&self, caps: &Capabilities) -> Vec<Update> {
        let mut out = Vec::new();
        if let Some(model) = caps.model() {
            out.push(Update::Model(model.current_value()));
        }
        if let Some(level) = caps.thought_level() {
            out.push(Update::Effort(Some(level.current_value())));
        }
        if let Some(mode) = caps.options.iter().find(|o| o.category == Category::Mode) {
            out.push(Update::Mode(mode.current_value()));
        }
        out
    }

    /// Maps the parameters of a `session/update` notification as JSON; what the protocol does not define is ignored.
    pub fn apply_json(&mut self, params: &Value, caps: &mut Capabilities, at: SystemTime) -> (Vec<Update>, bool) {
        match serde_json::from_value::<acp::SessionNotification>(params.clone()) {
            Ok(notification) => self.update(&notification.update, caps, at),
            Err(_) => (Vec::new(), false),
        }
    }
}
