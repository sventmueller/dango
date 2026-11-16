//! A session of the app's registry (E165) that an AI tab follows and writes into without having started it, such as
//! a work order's (docs/phase-13.md, B37b, E184). The tab subscribes to the session's feed, whose first batch is the
//! snapshot, hands the user's messages to the driver's queue and answers requests through the driver; the runtime
//! hears of each such step and logs it. Closing the tab ends its feed, never the session.

use std::rc::Rc;

use ide_core::explain::Explained;
use ide_session::{AnswerError, Batch, Driver, PlanReply, Prompt, SessionId, SubscriberId};
use ide_thread::{Answer, Thread, UserMessage};

/// The app's session registry as an attached AI tab reaches it, by the session's id; the other calls tell the runtime
/// what the user did in the session, which it records for the order (E184).
pub trait Sessions {
    /// Runs `f` with the driver of `session`; false, without running it, once the registry let the session go.
    fn with_driver(&self, session: SessionId, f: &mut dyn FnMut(&mut Driver)) -> bool;
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
}

impl Driven {
    /// Subscribes to the session `attach` names; `None` when it is gone or has ended.
    pub(crate) fn subscribe(attach: &Attach) -> Option<Driven> {
        let mut subscriber = None;
        attach.sessions.with_driver(attach.session, &mut |driver| {
            if !driver.is_ended() {
                subscriber = Some(driver.subscribe());
            }
        });
        Some(Driven {
            session: attach.session,
            subscriber: subscriber?,
            badge: attach.badge.clone(),
            sessions: attach.sessions.clone(),
        })
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

    /// What waits in the tab's feed; `None` once the session is gone.
    pub(crate) fn take(&self) -> Option<Batch> {
        self.with(|driver| driver.take(self.subscriber)).flatten()
    }

    /// The thread as of the driver's last flush.
    pub(crate) fn snapshot(&self) -> Option<Thread> {
        self.with(|driver| driver.thread().clone())
    }

    /// Whether the session takes prompts.
    pub(crate) fn is_live(&self) -> bool {
        self.with(|driver| !driver.is_ended()).unwrap_or(false)
    }

    /// What Claude Code offers in the session, such as its models.
    pub(crate) fn capabilities(&self) -> Option<ide_claude::Capabilities> {
        self.with(|driver| driver.claude_capabilities()).flatten()
    }

    /// Hands `message` to the driver's queue, which sends it after the turn that runs and its checkpoint.
    pub(crate) fn send(&self, message: UserMessage) -> Result<(), Explained> {
        let prompt = Prompt {
            message,
            ..Prompt::default()
        };
        match self.with(|driver| driver.send(prompt)) {
            Some(Ok(_)) => {
                self.sessions.intervened(self.session);
                Ok(())
            }
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

    /// Switches the session's model, which the runtime records as a model switch (E184).
    pub(crate) fn set_model(&self, model: &str) {
        match self.with(|driver| driver.set_model(model)) {
            Some(Ok(())) => self.sessions.model_switched(self.session, model),
            Some(Err(why)) => ide_ui_common::diag::report("switching the model of an order's session", why),
            None => {}
        }
    }

    /// Allows the permission request `request` once or refuses it, telling the agent `reason`.
    pub(crate) fn answer_permission(&self, request: &str, allow: bool, reason: Option<&str>) -> Result<(), Explained> {
        let answered = self.with(|driver| driver.answer_permission(request, allow, reason));
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
                self.sessions.answered(self.session, request);
                Ok(())
            }
            Some(Err(error)) => Err(Explained(error.to_string())),
            None => Err(gone()),
        }
    }

    /// Ends the tab's feed; the session runs on.
    pub(crate) fn leave(&self) {
        self.with(|driver| driver.unsubscribe(self.subscriber));
    }
}

fn gone() -> Explained {
    Explained::of(&ide_claude::SessionError::Closed)
}
