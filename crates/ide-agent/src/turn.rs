//! One turn of the agent: request, stream the answer into the thread, run the tools it calls with the user's
//! approval where the policy wants it, and go on until the model ends its turn, the user cancels or a failure stops
//! it. Passing failures are retried with the provider's wait or a growing pause; a conversation that no longer fits
//! the context window is compacted first (E42).

use std::collections::HashSet;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant, SystemTime};

use ide_thread::{
    AgentMessage, Decision, EntryKind, ErrorKind as ThreadError, Lane, ModelCost, Notice, PermissionOption,
    PermissionRequest, RequestState, Status, Thought, Tokens, ToolCall, ToolOutput, ToolPatch, ToolStatus,
    TurnUsage as ThreadTurn, Update, UsagePatch,
};
use serde_json::{Value, json};

use crate::chat::{Cancel, Delta, ErrorKind, ProviderError, Reply, Request, Stop, TurnUsage};
use crate::conversation::{Block, Message, Role};
use crate::notes::{Note, ToolNote};
use crate::policy::Verdict;
use crate::session::{Choice, Event, Shared};
use crate::store::Totals;
use crate::tools::{self, Context, Outcome};

/// Tries of one request before a passing failure counts.
const ATTEMPTS: u32 = 5;
/// Characters of a tool's result the model gets at most.
const MAX_RESULT: usize = 100_000;

/// Runs the turn the last user message started.
pub(crate) fn run(shared: &Shared, cancel: &Cancel) {
    let (started, before) = (Instant::now(), shared.lock().totals);
    shared.apply(vec![Update::Status(Status::Running)]);
    loop {
        if crate::compact::due(shared) {
            crate::compact::run(shared, cancel, false, None);
            if cancel.is_cancelled() {
                notice(shared, Notice::Interrupted);
                break;
            }
        }
        let Some(reply) = request(shared, cancel) else {
            break;
        };
        let calls: Vec<(String, String, Value, Option<String>)> = reply
            .message
            .content
            .iter()
            .filter_map(|block| match block {
                Block::ToolUse {
                    id,
                    name,
                    input,
                    invalid,
                    ..
                } => Some((id.clone(), name.clone(), input.clone(), invalid.clone())),
                _ => None,
            })
            .collect();
        if !reply.message.content.is_empty() {
            shared.lock().conversation.push(reply.message.clone());
        }
        shared.save();
        if !go_on(shared, &reply.stop, calls.is_empty()) {
            break;
        }
        let results = if reply.stop == Stop::MaxTokens {
            cut_off(shared, &calls)
        } else {
            run_tools(shared, cancel, &calls)
        };
        shared.lock().conversation.push(Message {
            role: Role::User,
            content: results,
        });
        shared.save();
        if cancel.is_cancelled() {
            notice(shared, Notice::Interrupted);
            break;
        }
    }
    shared.lock().running = false;
    let mut patch = usage(shared);
    patch.turn = Some(turn_usage(before, shared.lock().totals, started.elapsed()));
    shared.apply(vec![Update::Usage(patch), Update::Status(Status::Idle)]);
}

/// Compacts the conversation because the user asked to, as a turn of its own.
pub(crate) fn compact(shared: &Shared, cancel: &Cancel, focus: Option<&str>) {
    shared.apply(vec![Update::Status(Status::Running)]);
    crate::compact::run(shared, cancel, true, focus);
    shared.lock().running = false;
    shared.apply(vec![Update::Usage(usage(shared)), Update::Status(Status::Idle)]);
}

/// What a turn took: the difference of the totals after it and before it.
fn turn_usage(before: Totals, after: Totals, duration: Duration) -> ThreadTurn {
    let spent = before.input + before.output + before.cache_read + before.cache_write > 0;
    ThreadTurn {
        message: None,
        cost_usd: match (before.cost_usd, after.cost_usd) {
            (Some(before), Some(after)) => Some((after - before).max(0.0)),
            (None, Some(after)) if !spent => Some(after),
            _ => None,
        },
        tokens: Some(Tokens {
            input: after.input.saturating_sub(before.input),
            output: after.output.saturating_sub(before.output),
            cache_read: after.cache_read.saturating_sub(before.cache_read),
            cache_write: after.cache_write.saturating_sub(before.cache_write),
        }),
        duration: Some(duration),
    }
}

/// Whether the turn goes on after a reply that stopped for `stop`, telling the user why when it does not.
fn go_on(shared: &Shared, stop: &Stop, no_calls: bool) -> bool {
    match stop {
        Stop::ToolUse | Stop::Other(_) if !no_calls => true,
        Stop::MaxTokens if !no_calls => true,
        Stop::EndTurn | Stop::ToolUse | Stop::Other(_) => false,
        Stop::Cancelled => {
            notice(shared, Notice::Interrupted);
            false
        }
        Stop::MaxTokens => {
            info(shared, &Note::LengthLimit);
            false
        }
        Stop::Refusal(category) => {
            notice(
                shared,
                Notice::Error {
                    kind: ThreadError::Api,
                    message: shared.wording.note(&Note::Refused(category.clone())),
                },
            );
            false
        }
    }
}

pub(crate) fn notice(shared: &Shared, notice: Notice) {
    let key = shared.key("n");
    shared.apply(vec![Update::Put {
        lane: Lane::Main,
        key: Some(key),
        at: SystemTime::now(),
        entry: EntryKind::Notice(notice),
    }]);
}

fn info(shared: &Shared, note: &Note) {
    notice(
        shared,
        Notice::Info {
            level: ide_thread::InfoLevel::Notice,
            text: shared.wording.note(note),
        },
    );
}

/// The entries one attempt has put into the timeline so far.
#[derive(Default)]
struct Streamed {
    message: Option<String>,
    thought: Option<String>,
    tools: Vec<String>,
}

impl Streamed {
    /// Takes the entries out of the timeline again, as the request is made anew.
    fn remove(self, shared: &Shared) {
        let removed = [self.message, self.thought]
            .into_iter()
            .flatten()
            .chain(self.tools)
            .map(|key| Update::Remove { key })
            .collect();
        shared.apply(removed);
    }
}

/// Sends the conversation and streams the answer into the thread, retrying passing failures and compacting a
/// conversation the window no longer holds; `None` when the turn ends here.
fn request(shared: &Shared, cancel: &Cancel) -> Option<Reply> {
    let mut attempt = 0;
    let mut compacted = false;
    loop {
        attempt += 1;
        let mut streamed = Streamed::default();
        let error = match stream_once(shared, cancel, &mut streamed) {
            Ok(reply) => {
                finish(shared, &streamed, &reply);
                return Some(reply);
            }
            Err(error) => error,
        };
        if error.kind == ErrorKind::ContextLength && !compacted && !cancel.is_cancelled() {
            compacted = true;
            streamed.remove(shared);
            if crate::compact::run(shared, cancel, false, None) {
                attempt = 0;
                continue;
            }
            streamed = Streamed::default();
        }
        if error.is_retryable() && attempt < ATTEMPTS && !cancel.is_cancelled() {
            streamed.remove(shared);
            if wait_to_retry(shared, cancel, &error, attempt) {
                notice(shared, Notice::Interrupted);
                return None;
            }
            continue;
        }
        fail(shared, streamed, &error);
        return None;
    }
}

/// Tells the user that the request is made again and waits for it; whether the user cancelled meanwhile.
fn wait_to_retry(shared: &Shared, cancel: &Cancel, error: &ProviderError, attempt: u32) -> bool {
    let delay = error
        .retry_after
        .unwrap_or_else(|| Duration::from_secs(1 << attempt.min(5)))
        .min(Duration::from_secs(120));
    notice(
        shared,
        Notice::Retrying {
            attempt,
            max_attempts: ATTEMPTS,
            delay,
            error: shared.wording.retry(error),
            http_status: error.status,
        },
    );
    sleep(delay, cancel)
}

/// Ends the streaming of what arrived and tells the user why the turn stops.
fn fail(shared: &Shared, streamed: Streamed, error: &ProviderError) {
    let finished = [streamed.message, streamed.thought]
        .into_iter()
        .flatten()
        .map(|key| Update::Finish {
            key,
            text: None,
            at: SystemTime::now(),
        })
        .collect();
    shared.apply(finished);
    let kind = match error.kind {
        ErrorKind::NoKey | ErrorKind::Auth | ErrorKind::Permission => ThreadError::Auth,
        _ => ThreadError::Api,
    };
    notice(
        shared,
        Notice::Error {
            kind,
            message: shared.wording.note(&Note::Failed(error.clone())),
        },
    );
}

/// Waits `delay`, or less when the turn is cancelled meanwhile; whether it was.
fn sleep(delay: Duration, cancel: &Cancel) -> bool {
    let end = std::time::Instant::now() + delay;
    while std::time::Instant::now() < end {
        if cancel.is_cancelled() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cancel.is_cancelled()
}

fn stream_once(shared: &Shared, cancel: &Cancel, streamed: &mut Streamed) -> Result<Reply, ProviderError> {
    let (provider, model, effort, policy, messages) = {
        let state = shared.lock();
        (
            state.provider.clone(),
            state.model.clone(),
            state.effort.clone(),
            state.policy.clone(),
            state.conversation.clone(),
        )
    };
    let system = crate::prompt::system(shared.workspace.root(), policy.profile, policy.sandbox, &shared.rules);
    let tools = tools::specs(policy.profile);
    let effort = effort.filter(|_| crate::takes_effort(provider.endpoint().kind, &model));
    let request = Request {
        model: &model,
        system: &system,
        messages: &messages,
        tools: &tools,
        max_tokens: shared.max_tokens,
        effort: effort.as_deref(),
    };
    provider.stream(&request, cancel, &mut |delta| on_delta(shared, streamed, delta))
}

/// Shows a part of the answer as it arrives.
fn on_delta(shared: &Shared, streamed: &mut Streamed, delta: Delta) {
    let put = |key: &str, entry: EntryKind| Update::Put {
        lane: Lane::Main,
        key: Some(key.to_string()),
        at: SystemTime::now(),
        entry,
    };
    let mut updates = Vec::new();
    match delta {
        Delta::Text(text) => {
            let key = streamed.message.get_or_insert_with(|| {
                let key = shared.key("m");
                updates.push(put(
                    &key,
                    EntryKind::AgentMessage(AgentMessage {
                        markdown: String::new(),
                        streaming: true,
                    }),
                ));
                key
            });
            updates.push(Update::Append { key: key.clone(), text });
        }
        Delta::Thinking(text) => {
            let key = streamed.thought.get_or_insert_with(|| {
                let key = shared.key("t");
                updates.push(put(
                    &key,
                    EntryKind::Thought(Thought {
                        text: String::new(),
                        duration: None,
                        streaming: true,
                    }),
                ));
                key
            });
            updates.push(Update::Append { key: key.clone(), text });
        }
        Delta::ToolStarted { id, name } => {
            let (kind, title, view) = tools::card(&name, &json!({}), &shared.workspace);
            let mut call = ToolCall::new(id.clone(), name, kind);
            call.title = title;
            call.view = view;
            updates.push(put(&id, EntryKind::ToolCall(call)));
            streamed.tools.push(id);
        }
        Delta::ToolInput { .. } => {}
        Delta::Fallback { from, to } => {
            updates.extend(streamed.tools.drain(..).map(|key| Update::Remove { key }));
            updates.push(put(
                &shared.key("n"),
                EntryKind::Notice(Notice::Info {
                    level: ide_thread::InfoLevel::Notice,
                    text: shared.wording.note(&Note::Fallback { from, to }),
                }),
            ));
        }
    }
    shared.apply(updates);
}

/// Ends the streaming of the reply's entries and counts its tokens.
fn finish(shared: &Shared, streamed: &Streamed, reply: &Reply) {
    let now = SystemTime::now();
    let mut updates = Vec::new();
    if let Some(key) = &streamed.message {
        updates.push(Update::Finish {
            key: key.clone(),
            text: Some(reply.message.text()),
            at: now,
        });
    }
    if let Some(key) = &streamed.thought {
        updates.push(Update::Finish {
            key: key.clone(),
            text: None,
            at: now,
        });
    }
    let kept: HashSet<&str> = reply.message.tool_uses().map(|(id, _, _)| id).collect();
    updates.extend(
        streamed
            .tools
            .iter()
            .filter(|key| !kept.contains(key.as_str()))
            .map(|key| Update::Remove { key: key.clone() }),
    );
    count(shared, &reply.usage);
    let used = reply.usage.prompt() + reply.usage.output;
    let compacted = {
        let mut state = shared.lock();
        state.context_used = Some(used);
        state.compacted.take()
    };
    let mut patch = usage(shared);
    patch.context_used = Some(used);
    updates.push(Update::Usage(patch));
    if let Some(compacted) = compacted {
        updates.push(Update::Put {
            lane: Lane::Main,
            key: Some(compacted.key),
            at: now,
            entry: EntryKind::Notice(Notice::Compaction {
                manual: compacted.manual,
                tokens_before: compacted.tokens_before,
                tokens_after: Some(reply.usage.prompt()),
            }),
        });
    }
    shared.apply(updates);
}

/// Adds what a request used to the thread's totals and its model's.
pub(crate) fn count(shared: &Shared, usage: &TurnUsage) {
    let mut state = shared.lock();
    let price = shared
        .prices
        .price(state.provider.endpoint().kind, &state.model, state.listed_price);
    let cost = crate::prices::cost(usage, price.as_ref());
    let model = state.model.clone();
    add(&mut state.totals, usage, cost);
    add(state.by_model.entry(model).or_default(), usage, cost);
}

fn add(totals: &mut Totals, usage: &TurnUsage, cost: Option<f64>) {
    let first = totals.input + totals.output + totals.cache_read + totals.cache_write == 0;
    // A cost is shown only while every request had a price; a sum that leaves some out would read as complete.
    totals.cost_usd = match (totals.cost_usd, cost) {
        (Some(sum), Some(cost)) => Some(sum + cost),
        (None, Some(cost)) if first => Some(cost),
        _ => None,
    };
    totals.input += usage.input;
    totals.output += usage.output;
    totals.cache_read += usage.cache_read;
    totals.cache_write += usage.cache_write;
}

/// The thread's usage as the totals have it; by model only while every model's cost is known.
pub(crate) fn usage(shared: &Shared) -> UsagePatch {
    let state = shared.lock();
    let totals = state.totals;
    let mut models: Option<Vec<ModelCost>> = state
        .by_model
        .iter()
        .map(|(model, totals)| {
            Some(ModelCost {
                model: model.clone(),
                cost_usd: totals.cost_usd?,
                tokens: Tokens {
                    input: totals.input,
                    output: totals.output,
                    cache_read: totals.cache_read,
                    cache_write: totals.cache_write,
                },
                context_window: state.context_size.filter(|_| *model == state.model),
            })
        })
        .collect();
    if let Some(models) = &mut models {
        models.sort_by(|a, b| b.cost_usd.total_cmp(&a.cost_usd));
    }
    UsagePatch {
        input_tokens: Some(totals.input + totals.cache_read + totals.cache_write),
        output_tokens: Some(totals.output),
        cache_read_tokens: Some(totals.cache_read),
        cache_write_tokens: Some(totals.cache_write),
        cost_usd: totals.cost_usd,
        context_used: None,
        context_size: state.context_size,
        models: models.filter(|m| !m.is_empty()),
        ..UsagePatch::default()
    }
}

/// Answers calls whose input the length limit cut off, so the model can try again with less.
fn cut_off(shared: &Shared, calls: &[(String, String, Value, Option<String>)]) -> Vec<Block> {
    calls
        .iter()
        .map(|(id, name, _, _)| {
            fail_card(shared, id, &ToolNote::CutOff);
            result(id, name, &ToolNote::CutOff.for_model(), true, Some(ToolNote::CutOff))
        })
        .collect()
}

fn result(id: &str, name: &str, content: &str, is_error: bool, note: Option<ToolNote>) -> Block {
    let total = content.chars().count();
    let mut content: String = content.chars().take(MAX_RESULT).collect();
    if total > MAX_RESULT {
        content.push_str("\n[… cut]");
    }
    Block::ToolResult {
        tool_use_id: id.to_string(),
        name: name.to_string(),
        content,
        is_error,
        note,
    }
}

fn fail_card(shared: &Shared, id: &str, note: &ToolNote) {
    shared.apply(vec![Update::Tool {
        key: id.to_string(),
        patch: ToolPatch {
            status: Some(ToolStatus::Failed),
            output: Some(ToolOutput::Text(shared.wording.tool(note))),
            ended: Some(SystemTime::now()),
            ..ToolPatch::default()
        },
    }]);
}

/// Read-only calls may overlap within a group; every other call is an ordering barrier.
fn run_tools(shared: &Shared, cancel: &Cancel, calls: &[(String, String, Value, Option<String>)]) -> Vec<Block> {
    let mut results = Vec::new();
    let mut at = 0;
    while at < calls.len() {
        let end = calls[at..]
            .iter()
            .take(4)
            .take_while(|(_, name, input, invalid)| {
                invalid.is_none()
                    && matches!(
                        name.as_str(),
                        "read_file" | "grep" | "glob" | "diagnostics" | "web_fetch"
                    )
                    && tools::action(name, input, &shared.workspace)
                        .is_ok_and(|action| shared.lock().policy.decide(&action) == Verdict::Allow)
            })
            .count()
            + at;
        if end > at + 1 && !cancel.is_cancelled() {
            results.extend(read_group(shared, cancel, &calls[at..end]));
            at = end;
        } else {
            let (id, name, input, invalid) = &calls[at];
            let outcome = if cancel.is_cancelled() {
                rejected(shared, id, ToolNote::Cancelled)
            } else if let Some(raw) = invalid {
                let mut outcome = rejected(shared, id, ToolNote::InvalidJson);
                outcome.content = json!({"INVALID_JSON": raw}).to_string();
                outcome
            } else {
                call(shared, cancel, id, name, input)
            };
            results.push(result(id, name, &outcome.content, outcome.is_error, outcome.note));
            at += 1;
        }
    }
    results
}

fn rejected(shared: &Shared, id: &str, note: ToolNote) -> Outcome {
    fail_card(shared, id, &note);
    Outcome::failed(note)
}

/// Permissions are settled in order before workers start, so parallel reads never race approval status.
fn read_group(shared: &Shared, cancel: &Cancel, calls: &[(String, String, Value, Option<String>)]) -> Vec<Block> {
    let prepared: Vec<_> = calls
        .iter()
        .map(|(id, name, input, _)| {
            if cancel.is_cancelled() {
                Err(ToolNote::Cancelled)
            } else {
                prepare(shared, cancel, id, name, input)
            }
        })
        .collect();
    std::thread::scope(|scope| {
        let workers: Vec<_> = calls
            .iter()
            .zip(prepared)
            .map(|((id, name, input, _), prepared)| {
                scope.spawn(move || match prepared {
                    Err(note) => rejected(shared, id, note),
                    Ok(()) if cancel.is_cancelled() => rejected(shared, id, ToolNote::Cancelled),
                    Ok(()) => execute(shared, cancel, id, name, input),
                })
            })
            .collect();
        workers
            .into_iter()
            .zip(calls)
            .map(|(worker, (id, name, _, _))| {
                let outcome = worker
                    .join()
                    .unwrap_or_else(|_| rejected(shared, id, ToolNote::NotStarted("tool worker panicked".into())));
                result(id, name, &outcome.content, outcome.is_error, outcome.note)
            })
            .collect()
    })
}

/// One call: its card, the policy's verdict, the user's answer where needed, and running it.
fn call(shared: &Shared, cancel: &Cancel, id: &str, name: &str, input: &Value) -> Outcome {
    match prepare(shared, cancel, id, name, input) {
        Err(note) => rejected(shared, id, note),
        Ok(()) if cancel.is_cancelled() => rejected(shared, id, ToolNote::Cancelled),
        Ok(()) => execute(shared, cancel, id, name, input),
    }
}

fn prepare(shared: &Shared, cancel: &Cancel, id: &str, name: &str, input: &Value) -> Result<(), ToolNote> {
    let (kind, title, view) = tools::card(name, input, &shared.workspace);
    let known = shared.lock().thread.entry_by_key(id).is_some();
    if !known {
        let mut card = ToolCall::new(id, name, kind);
        card.title = title.clone();
        shared.apply(vec![Update::Put {
            lane: Lane::Main,
            key: Some(id.to_string()),
            at: SystemTime::now(),
            entry: EntryKind::ToolCall(card),
        }]);
    }
    shared.apply(vec![Update::Tool {
        key: id.to_string(),
        patch: ToolPatch {
            input: Some(input.clone()),
            title: Some(title.clone()),
            view: Some(view),
            ..ToolPatch::default()
        },
    }]);
    let action = tools::action(name, input, &shared.workspace)?;
    let verdict = shared.lock().policy.decide(&action);
    match verdict {
        Verdict::Allow => Ok(()),
        Verdict::Deny(refusal) => Err(ToolNote::Refused(refusal)),
        verdict @ Verdict::Ask { .. } => ask(
            shared,
            cancel,
            &Asked {
                id,
                name,
                title: &title,
                input,
            },
            verdict,
        ),
    }
}

fn execute(shared: &Shared, cancel: &Cancel, id: &str, name: &str, input: &Value) -> Outcome {
    shared.apply(vec![Update::Tool {
        key: id.to_string(),
        patch: ToolPatch {
            status: Some(ToolStatus::InProgress),
            started: Some(SystemTime::now()),
            ..ToolPatch::default()
        },
    }]);
    let sandbox = shared.lock().policy.sandbox;
    let context = Context {
        workspace: &shared.workspace,
        sandbox,
        cancel,
    };
    let outcome = tools::run(name, input, &context);
    let mut updates = vec![Update::Tool {
        key: id.to_string(),
        patch: ToolPatch {
            status: Some(if outcome.is_error {
                ToolStatus::Failed
            } else {
                ToolStatus::Completed
            }),
            output: outcome.output.clone().or_else(|| {
                outcome
                    .note
                    .as_ref()
                    .map(|note| ToolOutput::Text(shared.wording.tool(note)))
            }),
            ended: Some(SystemTime::now()),
            results: outcome.results,
            ..ToolPatch::default()
        },
    }];
    if let Some(todos) = &outcome.todos {
        updates.push(Update::Todos(todos.clone()));
    }
    shared.apply(updates);
    outcome
}

/// The call a permission request is about.
struct Asked<'a> {
    id: &'a str,
    name: &'a str,
    title: &'a str,
    input: &'a Value,
}

/// Asks the user about a call and waits for the answer; `Err` with why it may not run.
fn ask(shared: &Shared, cancel: &Cancel, call: &Asked<'_>, verdict: Verdict) -> Result<(), ToolNote> {
    let Verdict::Ask {
        rule,
        reason,
        blocked_path,
        risky,
    } = verdict
    else {
        return Ok(());
    };
    let request = format!("p:{}", call.id);
    let mut options = vec![PermissionOption::AllowOnce];
    if let Some(rule) = &rule {
        options.push(PermissionOption::AllowAlways {
            suggestion: 0,
            rule: shared.wording.rule(rule),
            scope: "project".into(),
        });
    }
    options.push(PermissionOption::Deny);
    let (sender, receiver) = channel();
    shared.lock().asks.insert(request.clone(), sender);
    shared.apply(vec![
        Update::Put {
            lane: Lane::Main,
            key: Some(request.clone()),
            at: SystemTime::now(),
            entry: EntryKind::PermissionRequest(PermissionRequest {
                request_id: request.clone(),
                tool_call: Some(call.id.to_string()),
                tool_name: call.name.to_string(),
                title: None,
                description: Some(call.title.to_string()).filter(|t| t != call.name),
                reason: reason.map(|r| shared.wording.reason(&r)),
                blocked_path,
                input: call.input.clone(),
                options: options.clone(),
                default_to_no: risky,
                state: RequestState::Pending,
            }),
        },
        Update::Status(Status::RequiresAction),
    ]);
    let choice = loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(choice) => break Some(choice),
            Err(RecvTimeoutError::Timeout) if !cancel.is_cancelled() => {}
            Err(_) => break None,
        }
    };
    shared.lock().asks.remove(&request);
    let picked = match choice {
        Some(Choice::Option(n)) => options.get(n).cloned(),
        Some(Choice::Dismiss) | None => None,
    };
    let (state, verdict) = match (&picked, choice) {
        (_, None) => (RequestState::Withdrawn, Err(ToolNote::Withdrawn)),
        (Some(PermissionOption::AllowOnce), _) => (
            RequestState::Decided(Decision::Allowed {
                always: false,
                edited: false,
            }),
            Ok(()),
        ),
        (Some(PermissionOption::AllowAlways { .. }), _) => {
            if let Some(rule) = rule {
                shared.lock().policy.rules.push(rule.clone());
                (shared.sink)(Event::RuleAdded(rule));
            }
            (
                RequestState::Decided(Decision::Allowed {
                    always: true,
                    edited: false,
                }),
                Ok(()),
            )
        }
        _ => (
            RequestState::Decided(Decision::Denied { message: None }),
            Err(ToolNote::Declined),
        ),
    };
    shared.apply(vec![
        Update::Resolve { key: request, state },
        Update::Status(Status::Running),
    ]);
    verdict
}
