//! The five sessions behind one set of calls: send, stop, answer, read the thread. What a path cannot do is said
//! as an error in plain words, never guessed at.

use std::time::Duration;

use ide_core::explain::Explain;
use ide_thread::{Answer, PermissionOption, PermissionRequest, Thread, UserMessage};
use serde_json::Value;

/// What the model reads when a refusal carries no reason of its own; for the model, so not translated.
const DENIED: &str = "The user denied this tool call.";

/// How long a stopped process gets to end on its own before the supervisor's signals.
const GRACE: Duration = Duration::from_secs(2);

/// The answer to a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanReply {
    /// Carry it out.
    Approve,
    /// Plan again, with what to change.
    Revise(Option<String>),
    /// Do not carry it out, and end the turn.
    Reject,
}

/// Why a prompt did not go out.
pub(crate) enum SendFailure {
    /// The session still starts the turn before, whose end already showed; the prompt goes out a moment later.
    Busy,
    /// The path refused it, in its own words.
    Failed(String),
}

pub(crate) enum Backend {
    Claude(ide_claude::Session),
    Codex(ide_codex::Session),
    OpenCode(ide_opencode::Session),
    Acp(ide_acp::Session),
    Own(ide_agent::Session),
    #[cfg(any(test, feature = "selftest"))]
    Script(crate::script::Session),
    #[cfg(any(test, feature = "selftest"))]
    Replay(std::cell::RefCell<ide_claude::replay::Replay>),
}

/// The option of `request` that allows it once, or that refuses it.
fn option(request: &PermissionRequest, allow: bool) -> Option<usize> {
    let wanted = if allow {
        PermissionOption::AllowOnce
    } else {
        PermissionOption::Deny
    };
    request.options.iter().position(|o| *o == wanted)
}

/// `reply` in Claude's terms.
fn plan_answer(reply: PlanReply) -> ide_claude::PlanAnswer {
    match reply {
        PlanReply::Approve => ide_claude::PlanAnswer::Approve,
        PlanReply::Revise(feedback) => ide_claude::PlanAnswer::Revise(feedback),
        PlanReply::Reject => ide_claude::PlanAnswer::Reject,
    }
}

fn unsupported(what: &str) -> String {
    format!("this agent takes no {what}")
}

impl Backend {
    pub(crate) fn snapshot(&self) -> Thread {
        match self {
            Backend::Claude(s) => s.snapshot(),
            Backend::Codex(s) => s.snapshot(),
            Backend::OpenCode(s) => s.snapshot(),
            Backend::Acp(s) => s.snapshot(),
            Backend::Own(s) => s.snapshot(),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => s.snapshot(),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Replay(r) => r.borrow().thread().clone(),
        }
    }

    /// Whether the session is open and takes prompts.
    pub(crate) fn is_ready(&self) -> bool {
        match self {
            Backend::Claude(s) => !s.is_ended(),
            Backend::Codex(s) => s.is_ready(),
            Backend::OpenCode(s) => s.is_ready(),
            Backend::Acp(s) => s.is_ready(),
            Backend::Own(_) => true,
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => !s.is_ended(),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Replay(_) => false,
        }
    }

    /// Whether the agent's process has ended, which the end event may not have told yet.
    pub(crate) fn is_ended(&self) -> bool {
        match self {
            Backend::Claude(s) => s.is_ended(),
            Backend::Codex(s) => s.is_ended(),
            Backend::OpenCode(s) => s.is_ended(),
            Backend::Acp(s) => s.is_ended(),
            Backend::Own(_) => false,
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => s.is_ended(),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Replay(_) => false,
        }
    }

    /// Whether a turn runs that the thread's status may not show yet: the own agent sets it from its turn's thread.
    pub(crate) fn busy(&self) -> bool {
        matches!(self, Backend::Own(s) if s.is_running())
    }

    /// Whether a prompt can carry a schema for its answer (C63).
    pub(crate) fn takes_schema(&self) -> bool {
        matches!(self, Backend::Codex(_))
    }

    /// Sends `message`; the agent's id for it, where the path gives one (Claude's, which rewinding names).
    pub(crate) fn send(&self, message: UserMessage, schema: Option<Value>) -> Result<Option<String>, SendFailure> {
        let codex = |e: ide_codex::SessionError| match e {
            ide_codex::SessionError::Busy => SendFailure::Busy,
            other => SendFailure::Failed(other.to_string()),
        };
        let none = |()| None;
        match (self, schema) {
            (Backend::Codex(s), Some(schema)) => s.send_with_schema(message, schema).map(none).map_err(codex),
            (_, Some(_)) => Err(SendFailure::Failed(unsupported("schema for its answer"))),
            (Backend::Claude(s), None) => s.send(message).map(Some).map_err(|e| SendFailure::Failed(e.explain())),
            (Backend::Codex(s), None) => s.send(message).map(none).map_err(codex),
            (Backend::OpenCode(s), None) => s.send(message).map(none).map_err(|e| match e {
                ide_opencode::SessionError::Busy => SendFailure::Busy,
                other => SendFailure::Failed(other.to_string()),
            }),
            (Backend::Acp(s), None) => s.send(message).map(none).map_err(|e| SendFailure::Failed(e.explain())),
            (Backend::Own(s), None) => s.send(message).map(none).map_err(|e| match e {
                ide_agent::SessionError::Busy => SendFailure::Busy,
                other => SendFailure::Failed(other.explain()),
            }),
            #[cfg(any(test, feature = "selftest"))]
            (Backend::Script(s), None) => s.send(message).map(none).map_err(SendFailure::Failed),
            #[cfg(any(test, feature = "selftest"))]
            (Backend::Replay(_), None) => Err(SendFailure::Failed(unsupported("prompts"))),
        }
    }

    /// Stops the running turn; background work goes on.
    pub(crate) fn cancel(&self) {
        match self {
            Backend::Claude(s) => s.interrupt().then(drop),
            Backend::Codex(s) => s.cancel(),
            Backend::OpenCode(s) => s.cancel(),
            Backend::Acp(s) => s.cancel(),
            Backend::Own(s) => s.cancel(),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => s.cancel(),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Replay(_) => {}
        }
    }

    /// Ends the session and its process.
    pub(crate) fn stop(&self) {
        match self {
            Backend::Claude(s) => s.stop(GRACE),
            Backend::Codex(s) => s.stop(GRACE),
            Backend::OpenCode(s) => s.stop(GRACE),
            Backend::Acp(s) => s.stop(GRACE),
            Backend::Own(s) => s.stop(),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => s.stop(),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Replay(_) => {}
        }
    }

    /// The Claude session, for what only Claude Code offers; `None` on the other paths.
    pub(crate) fn claude(&self) -> Option<&ide_claude::Session> {
        match self {
            Backend::Claude(s) => Some(s),
            _ => None,
        }
    }

    /// What Claude Code offers: its models, commands and account; `None` on the other paths.
    pub(crate) fn claude_capabilities(&self) -> Option<ide_claude::Capabilities> {
        match self {
            Backend::Claude(s) => Some(s.capabilities()),
            _ => None,
        }
    }

    /// Switches the model of the running session, where the path can do that without starting again.
    pub(crate) fn set_model(&self, model: &str) -> Result<(), String> {
        match self {
            Backend::Claude(s) => {
                s.set_model(Some(model)).then(drop);
                Ok(())
            }
            _ => Err(unsupported("switch of the model")),
        }
    }

    /// Stops the background task or subagent `task`; false where the path cannot.
    pub(crate) fn stop_task(&self, task: &str) -> bool {
        match self {
            Backend::Claude(s) => {
                s.stop_task(task).then(drop);
                true
            }
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => s.stop_task(task),
            _ => false,
        }
    }

    /// Allows `request` once, or refuses it with `reason` where the path tells the agent one.
    pub(crate) fn permission(
        &self,
        request: &PermissionRequest,
        allow: bool,
        reason: Option<&str>,
    ) -> Result<(), String> {
        let id = request.request_id.as_str();
        let picked = option(request, allow);
        if allow && picked.is_none() && !matches!(self, Backend::Claude(_)) {
            return Err("the request offers no way to allow it once".into());
        }
        match self {
            Backend::Claude(s) => {
                let answer = if allow {
                    ide_claude::PermissionAnswer::Allow {
                        updated_input: None,
                        always: Vec::new(),
                    }
                } else {
                    ide_claude::PermissionAnswer::Deny {
                        message: reason.unwrap_or(DENIED).to_string(),
                        interrupt: false,
                    }
                };
                s.answer(id, answer).map_err(|e| e.explain())
            }
            Backend::Codex(s) => {
                let choice = picked.map_or(ide_codex::Choice::Dismiss, ide_codex::Choice::Option);
                s.answer(id, choice).map_err(|e| e.to_string())
            }
            Backend::OpenCode(s) => {
                let choice = picked.map_or(ide_opencode::Choice::Dismiss, ide_opencode::Choice::Option);
                s.answer(id, choice).map_err(|e| e.to_string())
            }
            Backend::Acp(s) => {
                let choice = picked.map_or(ide_acp::Choice::Dismiss, ide_acp::Choice::Option);
                s.answer(id, choice).map_err(|e| e.explain())
            }
            Backend::Own(s) => {
                let choice = picked.map_or(ide_agent::Choice::Dismiss, ide_agent::Choice::Option);
                s.answer(id, choice).map_err(|e| e.explain())
            }
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => s.answer(id, allow, reason),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Replay(_) => Err(unsupported("answers")),
        }
    }

    /// Answers the questions of `request`.
    pub(crate) fn questions(&self, request: &str, answers: Vec<Answer>) -> Result<(), String> {
        match self {
            Backend::Claude(s) => s.answer_questions(request, answers).map_err(|e| e.explain()),
            Backend::Codex(s) => {
                let form: serde_json::Map<String, Value> = answers
                    .iter()
                    .map(|a| (a.question.clone(), Value::from(a.answer.clone())))
                    .collect();
                s.answer_questions(request, answers)
                    .or_else(|_| s.answer_form(request, Value::Object(form)))
                    .map_err(|e| e.to_string())
            }
            Backend::OpenCode(s) => s.answer_questions(request, answers).map_err(|e| e.to_string()),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => s.answer_questions(request, answers),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Replay(_) => Err(unsupported("answers")),
            Backend::Acp(_) | Backend::Own(_) => Err(unsupported("answers to questions")),
        }
    }

    /// Answers the plan `request`.
    pub(crate) fn plan(&self, request: &str, reply: PlanReply) -> Result<(), String> {
        match self {
            Backend::Claude(s) => s.answer_plan(request, plan_answer(reply)).map_err(|e| e.explain()),
            #[cfg(any(test, feature = "selftest"))]
            Backend::Script(s) => s.answer_plan(request, reply),
            _ => Err(unsupported("plans")),
        }
    }

    /// Reads messages, waiting requests and usage from the agent again and hands the outcome to `done` on another
    /// thread; false where the path's stream cannot break off.
    pub(crate) fn resync(&self, done: impl FnOnce(Result<(), String>) + Send + 'static) -> bool {
        match self {
            Backend::OpenCode(s) => {
                s.resync().then(move |result| done(result.map_err(|e| e.to_string())));
                true
            }
            _ => false,
        }
    }

    /// Answers `request` as the user does in an AI tab: with the input they edited and the suggested rules to keep,
    /// which only Claude takes; the other paths take an answer without them.
    pub(crate) fn permission_as(
        &self,
        request: &PermissionRequest,
        answer: ide_claude::PermissionAnswer,
    ) -> Result<(), String> {
        if let Backend::Claude(s) = self {
            return s.answer(&request.request_id, answer).map_err(|e| e.explain());
        }
        match answer {
            ide_claude::PermissionAnswer::Allow { updated_input, always }
                if updated_input.is_none() && always.is_empty() =>
            {
                self.permission(request, true, None)
            }
            ide_claude::PermissionAnswer::Allow { .. } => Err(unsupported("edited input or rule to keep")),
            ide_claude::PermissionAnswer::Deny { message, .. } => self.permission(request, false, Some(&message)),
        }
    }

    /// Whether a recording plays instead of an agent.
    pub(crate) fn is_replay(&self) -> bool {
        #[cfg(any(test, feature = "selftest"))]
        {
            matches!(self, Backend::Replay(_))
        }
        #[cfg(not(any(test, feature = "selftest")))]
        {
            false
        }
    }
}

#[cfg(any(test, feature = "selftest"))]
impl Backend {
    /// Plays recorded `lines` into a recording's thread; what changed, `None` on the other paths.
    pub(crate) fn feed(&self, lines: &str) -> Option<Vec<ide_thread::Change>> {
        match self {
            Backend::Replay(replay) => Some(replay.borrow_mut().feed_all(lines)),
            _ => None,
        }
    }

    /// Answers the plan `request` of a recording with the frame a session would send; what changed, `None` on the
    /// other paths.
    pub(crate) fn replay_plan(&self, request: &str, reply: PlanReply) -> Option<Vec<ide_thread::Change>> {
        let Backend::Replay(replay) = self else { return None };
        Some(replay.borrow_mut().answer_plan(request, &plan_answer(reply)))
    }
}
