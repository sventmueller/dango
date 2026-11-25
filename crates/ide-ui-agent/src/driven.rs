//! A session of the app's registry (E165) that an AI tab follows: one it did not start, such as a work order's
//! (docs/phase-13.md, B37b, E184), or its own session on any path, which it starts there. The tab subscribes to the
//! session's feed, whose first batch is the snapshot, hands the user's messages to the driver and answers requests
//! through it. On an order's session the runtime hears of each such step and logs it, and closing the tab ends its
//! feed, never the session; the tab's own session ends with the tab.

use std::rc::Rc;

use ide_core::explain::Explained;
use ide_session::{
    AnswerError, Batch, Driver, Launch, LaunchError, PathKind, PlanReply, Prompt, SessionId, SubscriberId, TurnId,
};
use ide_thread::{Answer, Thread, UserMessage};

/// The app's session registry as an AI tab reaches it, by the session's id; the calls after `end` tell the runtime
/// what the user did in an order's session, which it records for the order (E184).
pub trait Sessions {
    /// Runs `f` with the driver of `session`; false, without running it, once the registry let the session go.
    fn with_driver(&self, session: SessionId, f: &mut dyn FnMut(&mut Driver)) -> bool;
    /// Starts the session `launch` describes for the user, whose requests all go to the user.
    fn start(&self, launch: Launch) -> Result<SessionId, LaunchError>;
    /// Ends the user's session `session` and lets it go; an order's session is left to its order.
    fn end(&self, session: SessionId);
    /// The user wrote a message into `session`, which the driver queued: an intervention.
    fn intervened(&self, session: SessionId);
    /// The user switched the model of `session` to `model`; no change of agent (E170).
    fn model_switched(&self, session: SessionId, model: &str);
    /// The user answered the request `request` of `session` in the AI tab.
    fn answered(&self, session: SessionId, request: &str);
}

/// What an AI tab attaches to: a running session of the registry, the badge that says whom it works for, and the
/// title the tab and the AI panel show.
pub struct Attach {
    /// The session.
    pub session: SessionId,
    /// The words of the badge over the conversation, such as "Auftrag CSV-Export · Builder".
    pub badge: String,
    /// The tab's title.
    pub title: String,
    /// The registry the session runs in.
    pub sessions: Rc<dyn Sessions>,
}

/// An AI tab's subscription to a session of the registry.
#[derive(Clone)]
pub(crate) struct Driven {
    session: SessionId,
    subscriber: SubscriberId,
    badge: String,
    sessions: Rc<dyn Sessions>,
    /// Whether the tab started the session for the user, so it ends with the tab and nobody logs what happens in it.
    own: bool,
    /// The path the session takes.
    kind: PathKind,
}

impl Driven {
    /// Subscribes to the session `attach` names; `None` when it is gone or has ended.
    pub(crate) fn subscribe(attach: &Attach) -> Option<Driven> {
        let (subscriber, kind) = subscribe(attach.sessions.as_ref(), attach.session)?;
        Some(Driven {
            session: attach.session,
            subscriber,
            badge: attach.badge.clone(),
            sessions: attach.sessions.clone(),
            own: false,
            kind,
        })
    }

    /// Starts the tab's own session `launch` describes in `sessions` and subscribes to it; with the first batch, the
    /// snapshot.
    pub(crate) fn start(sessions: &Rc<dyn Sessions>, launch: Launch) -> Result<(Driven, Option<Batch>), LaunchError> {
        let session = sessions.start(launch)?;
        let Some((subscriber, kind)) = subscribe(sessions.as_ref(), session) else {
            sessions.end(session);
            return Err(LaunchError::Start(String::new()));
        };
        let driven = Driven {
            session,
            subscriber,
            badge: String::new(),
            sessions: sessions.clone(),
            own: true,
            kind,
        };
        let first = driven.take();
        Ok((driven, first))
    }

    /// What `f` gives of the session's driver, while the registry holds it.
    fn with<R>(&self, f: impl FnOnce(&mut Driver) -> R) -> Option<R> {
        let mut f = Some(f);
        let mut out = None;
        self.sessions.with_driver(self.session, &mut |driver| {
            if let Some(f) = f.take() {
                out = Some(f(driver));
            }
        });
        out
    }

    /// The registry's id of the session.
    pub(crate) fn session(&self) -> SessionId {
        self.session
    }

    /// The badge over the conversation.
    pub(crate) fn badge(&self) -> &str {
        &self.badge
    }

    /// The path the session takes.
    pub(crate) fn kind(&self) -> PathKind {
        self.kind
    }

    /// What waits in the tab's feed; `None` once the session is gone.
    pub(crate) fn take(&self) -> Option<Batch> {
        self.with(|driver| driver.take(self.subscriber)).flatten()
    }

    /// The thread as of the driver's last flush.
    pub(crate) fn snapshot(&self) -> Option<Thread> {
        self.with(|driver| driver.thread().clone())
    }

    /// Whether the session takes prompts: an order's until it ended, the tab's own while its agent runs.
    pub(crate) fn is_live(&self) -> bool {
        let own = self.own;
        self.with(|driver| {
            if own {
                driver.takes_prompts()
            } else {
                !driver.is_ended()
            }
        })
        .unwrap_or(false)
    }

    /// Whether a recording plays instead of an agent.
    pub(crate) fn is_replay(&self) -> bool {
        self.with(|driver| driver.is_replay()).unwrap_or(false)
    }

    /// What Claude Code offers in the session, such as its models.
    pub(crate) fn capabilities(&self) -> Option<ide_claude::Capabilities> {
        self.with(|driver| driver.claude_capabilities()).flatten()
    }

    /// The Claude session behind the driver, for what only Claude Code does.
    pub(crate) fn claude(&self) -> Option<ide_claude::Session> {
        self.with(|driver| driver.claude().cloned()).flatten()
    }

    pub(crate) fn parkable_claude(&self) -> Option<ide_claude::Session> {
        self.with(|driver| {
            if !self.own || !driver.is_ready() || driver.is_working() {
                return None;
            }
            driver.claude().filter(|session| session.can_park()).cloned()
        })
        .flatten()
    }

    /// The Codex session behind the driver, for what only Codex does.
    pub(crate) fn codex(&self) -> Option<ide_codex::Session> {
        self.with(|driver| driver.codex().cloned()).flatten()
    }

    /// The OpenCode session behind the driver, for what only OpenCode does.
    pub(crate) fn opencode(&self) -> Option<ide_opencode::Session> {
        self.with(|driver| driver.opencode().cloned()).flatten()
    }

    /// The ACP session behind the driver, for what only an ACP agent does.
    pub(crate) fn acp(&self) -> Option<ide_acp::Session> {
        self.with(|driver| driver.acp().cloned()).flatten()
    }

    /// The own agent's session behind the driver, for its profiles and settings.
    pub(crate) fn own_agent(&self) -> Option<ide_agent::Session> {
        self.with(|driver| driver.own().cloned()).flatten()
    }

    /// Hands `message` to the driver's queue, which sends it after the turn that runs and its checkpoint.
    pub(crate) fn send(&self, message: UserMessage) -> Result<(), Explained> {
        let prompt = Prompt {
            message,
            ..Prompt::default()
        };
        self.prompt(prompt).map(|_| ())
    }

    /// Hands `prompt` to the driver, which sends it once no turn runs, after a checkpoint with its buffers; the turn
    /// it begins.
    pub(crate) fn prompt(&self, prompt: Prompt) -> Result<TurnId, Explained> {
        match self.with(|driver| driver.send(prompt)) {
            Some(Ok(turn)) => {
                if !self.own {
                    self.sessions.intervened(self.session);
                }
                Ok(turn)
            }
            Some(Err(error)) => Err(Explained(error.to_string())),
            None => Err(gone()),
        }
    }

    /// Hands `message` to the turn that runs, at once; the agent's id of it.
    pub(crate) fn steer(&self, message: UserMessage) -> Result<Option<String>, Explained> {
        match self.with(|driver| driver.steer(message)) {
            Some(Ok(id)) => Ok(id),
            Some(Err(error)) => Err(Explained(error.to_string())),
            None => Err(gone()),
        }
    }

    /// Stops the turn that runs; the station's work goes on.
    pub(crate) fn stop_turn(&self) {
        self.with(Driver::stop_turn);
    }

    /// Stops the subagent or background item `task`.
    pub(crate) fn stop_task(&self, task: &str) {
        self.with(|driver| driver.stop_task(task));
    }

    /// Switches the session's model, which the runtime records as a model switch on an order's session (E184).
    pub(crate) fn set_model(&self, model: &str) {
        match self.with(|driver| driver.set_model(model)) {
            Some(Ok(())) if !self.own => self.sessions.model_switched(self.session, model),
            Some(Err(why)) if !self.own => {
                ide_ui_common::diag::report("switching the model of an order's session", why)
            }
            _ => {}
        }
    }

    /// Allows the permission request `request` once or refuses it, telling the agent `reason`.
    pub(crate) fn answer_permission(&self, request: &str, allow: bool, reason: Option<&str>) -> Result<(), Explained> {
        let answered = self.with(|driver| driver.answer_permission(request, allow, reason));
        self.after_answer(request, answered)
    }

    /// Answers the permission request `request` with the input the user edited and the rules to keep.
    pub(crate) fn answer_permission_as(
        &self,
        request: &str,
        answer: ide_claude::PermissionAnswer,
    ) -> Result<(), Explained> {
        let answered = self.with(|driver| driver.answer_permission_as(request, answer));
        self.after_answer(request, answered)
    }

    /// Answers the permission request `request` with the option at `option` in its list, or dismisses it.
    pub(crate) fn answer_option(&self, request: &str, option: Option<usize>) -> Result<(), Explained> {
        let answered = self.with(|driver| driver.answer_option(request, option));
        self.after_answer(request, answered)
    }

    /// Answers the questions of `request`.
    pub(crate) fn answer_questions(&self, request: &str, answers: Vec<Answer>) -> Result<(), Explained> {
        let answered = self.with(|driver| driver.answer_questions(request, answers));
        self.after_answer(request, answered)
    }

    /// Answers the plan `request`.
    pub(crate) fn answer_plan(&self, request: &str, reply: PlanReply) -> Result<(), Explained> {
        let answered = self.with(|driver| driver.answer_plan(request, reply));
        self.after_answer(request, answered)
    }

    fn after_answer(&self, request: &str, answered: Option<Result<(), AnswerError>>) -> Result<(), Explained> {
        match answered {
            Some(Ok(())) => {
                if !self.own {
                    self.sessions.answered(self.session, request);
                }
                Ok(())
            }
            Some(Err(error)) => Err(Explained(error.to_string())),
            None => Err(gone()),
        }
    }

    /// Leaves the session: the tab's own ends, an order's runs on without the tab's feed.
    pub(crate) fn leave(&self) {
        if self.own {
            self.sessions.end(self.session);
        } else {
            self.with(|driver| driver.unsubscribe(self.subscriber));
        }
    }
}

#[cfg(feature = "selftest")]
impl Driven {
    /// Plays recorded `lines` into the recording the session plays; what they changed, with the thread then, which
    /// the tab takes in itself, so its feed is emptied.
    pub(crate) fn feed(&self, lines: &str) -> (Vec<ide_thread::Change>, Option<std::sync::Arc<Thread>>) {
        self.with(|driver| {
            if !driver.is_replay() {
                return (Vec::new(), Some(std::sync::Arc::new(driver.thread().clone())));
            }
            let changes = driver.feed(lines);
            let thread = driver.take(self.subscriber).map(|batch| batch.thread);
            (changes, thread)
        })
        .unwrap_or_default()
    }

    /// Answers the plan `request` of the recording the session plays; what changed, which the tab takes in itself.
    pub(crate) fn replay_plan(&self, request: &str, reply: PlanReply) -> Vec<ide_thread::Change> {
        self.with(|driver| {
            let changes = driver.replay_plan(request, reply);
            driver.take(self.subscriber);
            changes
        })
        .unwrap_or_default()
    }
}

/// A new subscriber to `session`, with the path it takes, unless it is gone or has ended.
fn subscribe(sessions: &dyn Sessions, session: SessionId) -> Option<(SubscriberId, PathKind)> {
    let mut subscriber = None;
    sessions.with_driver(session, &mut |driver| {
        if !driver.is_ended() {
            subscriber = Some((driver.subscribe(), driver.kind()));
        }
    });
    subscriber
}

/// Why a call did not reach the session: it has ended.
pub(crate) fn gone() -> Explained {
    Explained::of(&ide_claude::SessionError::Closed)
}
