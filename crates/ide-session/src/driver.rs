//! One session's driver (E165). It lives on the main thread, as an AI tab does; its session reports on threads of its
//! own into the driver's inbox, and the app's hop to the main thread has it take the inbox in (`flush`). It holds the
//! prompts that wait, takes the checkpoint before each one, answers requests by its rule, ends every turn with a
//! typed result and fans everything out to its subscribers.

mod claude;
mod requests;

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ide_process::Supervisor;
use ide_thread::{EntryId, Status, Thread, UserMessage};
use serde_json::Value;

use crate::backend::{Backend, SendFailure};
use crate::checkpoint::Checkpointer;
use crate::feed::{Batch, Feeds, Signal};
use crate::id::{SessionId, SubscriberId, TurnId};
use crate::inbox::{Inbound, Inbox};
use crate::launch::{Contract, Launch, LaunchError, PathKind};
use crate::registry::Wake;
use crate::rule::RequestRule;
use crate::turn::{self, Outcome, TurnEnd};

pub use requests::AnswerError;

/// A prompt for the agent.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prompt {
    /// The message.
    pub message: UserMessage,
    /// A JSON schema the final answer follows (C63); only Codex takes one.
    pub schema: Option<Value>,
    /// Unsaved editor contents by path, which the checkpoint before the prompt keeps.
    pub buffers: Vec<(PathBuf, Vec<u8>)>,
}

impl Prompt {
    /// A prompt of `text`.
    pub fn new(text: impl Into<String>) -> Prompt {
        Prompt {
            message: UserMessage {
                text: text.into(),
                ..UserMessage::default()
            },
            ..Prompt::default()
        }
    }
}

/// Why a prompt was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// The session has ended or is being stopped.
    Ended,
    /// The path takes no schema for an answer.
    NoSchema(PathKind),
    /// The path refused the message, in its own words.
    Refused(String),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Ended => f.write_str("the session has ended"),
            SendError::NoSchema(path) => write!(f, "{path:?} takes no schema for its answer"),
            SendError::Refused(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for SendError {}

/// Where the path's event stream stands (C62).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamState {
    /// Every event arrives.
    Live,
    /// It broke off; events may be missing.
    Gap,
    /// Messages, requests and usage are being read again.
    Resyncing,
    /// Reading them again failed; no prompt goes out until `Driver::resync` succeeds.
    Paused(String),
}

/// Where a turn stands before it ends.
enum Phase {
    /// The checkpoint before its prompt is being taken.
    Checkpointing(Prompt),
    /// The session was still starting the turn before; the prompt goes out with the next flush.
    Retrying(Prompt),
    /// Its prompt went out.
    Running,
}

/// The turn that runs.
struct Open {
    turn: TurnId,
    phase: Phase,
    /// The newest entry before the prompt; the turn's entries come after it.
    since: Option<EntryId>,
    /// Whether the session was seen working on it.
    running_seen: bool,
    /// Whether the user or the app stopped it.
    cancel: bool,
    /// Whether `Signal::TurnStarted` went out for it.
    announced: bool,
    checkpoint: Option<ide_git::Checkpoint>,
}

impl Open {
    fn new(turn: TurnId, phase: Phase) -> Open {
        Open {
            turn,
            phase,
            since: None,
            running_seen: false,
            cancel: false,
            announced: false,
            checkpoint: None,
        }
    }
}

/// The driver of one session.
pub struct Driver {
    id: SessionId,
    kind: PathKind,
    folder: PathBuf,
    contract: Contract,
    backend: Backend,
    inbox: Arc<Inbox>,
    thread: Thread,
    feeds: Feeds,
    rule: Box<dyn RequestRule>,
    checkpointer: Option<Checkpointer>,
    next_turn: u64,
    open: Option<Open>,
    queue: VecDeque<(TurnId, Prompt)>,
    last_end: Option<TurnEnd>,
    /// The requests the rule saw, and those of them it left to the user.
    handled: HashSet<String>,
    left: HashSet<String>,
    stream: StreamState,
    ready: bool,
    ended: bool,
    stopping: bool,
    /// Signals of calls outside a flush, which the next flush publishes.
    outbox: Vec<Signal>,
}

impl Driver {
    /// Starts the session `launch` describes as `id`, answering its requests by `rule`; `wake` brings the driver's
    /// flush onto the main thread.
    pub(crate) fn start(
        id: SessionId,
        launch: Launch,
        rule: Box<dyn RequestRule>,
        supervisor: &Supervisor,
        wake: Arc<dyn Wake>,
    ) -> Result<Driver, LaunchError> {
        let inbox = Inbox::new(id, wake);
        let kind = launch.path.kind();
        let folder = launch.folder.clone();
        let contract = launch.contract.clone();
        let git = launch.git.clone();
        let backend = crate::start::start(launch, supervisor, &inbox)?;
        let thread = backend.snapshot();
        let checkpointer = git.map(|git| {
            let name = match contract.order() {
                Some(order) => order.checkpoint_thread(),
                None => thread.id().to_string(),
            };
            Checkpointer::new(git, folder.clone(), name)
        });
        inbox.poke();
        Ok(Driver {
            id,
            kind,
            folder,
            contract,
            backend,
            inbox,
            thread,
            feeds: Feeds::default(),
            rule,
            checkpointer,
            next_turn: 0,
            open: None,
            queue: VecDeque::new(),
            last_end: None,
            handled: HashSet::new(),
            left: HashSet::new(),
            stream: StreamState::Live,
            ready: false,
            ended: false,
            stopping: false,
            outbox: Vec::new(),
        })
    }

    /// The registry's id of the session.
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// The path the session takes.
    pub fn kind(&self) -> PathKind {
        self.kind
    }

    /// The folder the agent works in.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Whom the session works for.
    pub fn contract(&self) -> &Contract {
        &self.contract
    }

    /// The thread as of the last flush.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    /// The agent's id of the conversation, which resuming names, once it has one.
    pub fn session_id(&self) -> Option<&str> {
        self.thread.session()
    }

    /// Whether the session is open and takes prompts.
    pub fn is_ready(&self) -> bool {
        self.ready && !self.ended && !self.stopping
    }

    /// Whether the agent's process ended or the session was stopped.
    pub fn is_ended(&self) -> bool {
        self.ended || self.stopping
    }

    /// The turn that runs, from its checkpoint to its end.
    pub fn turn(&self) -> Option<TurnId> {
        self.open.as_ref().map(|open| open.turn)
    }

    /// How many prompts wait for the turn that runs.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// The end of the last turn that ended.
    pub fn last_end(&self) -> Option<&TurnEnd> {
        self.last_end.as_ref()
    }

    /// Where the event stream stands.
    pub fn stream(&self) -> &StreamState {
        &self.stream
    }

    /// How many subagents and background items run.
    pub fn running_work(&self) -> usize {
        turn::running_work(&self.thread)
    }

    /// Whether anything of the session works: a turn, a prompt that waits, a subagent or background item; what the
    /// question before quitting and the activity view count (C58).
    pub fn is_working(&self) -> bool {
        !self.is_ended() && (self.open.is_some() || !self.queue.is_empty() || self.running_work() > 0)
    }

    /// Hands `prompt` over: it goes out once the session takes prompts and no turn runs, after the checkpoint.
    pub fn send(&mut self, prompt: Prompt) -> Result<TurnId, SendError> {
        if self.is_ended() {
            return Err(SendError::Ended);
        }
        if prompt.schema.is_some() && !self.backend.takes_schema() {
            return Err(SendError::NoSchema(self.kind));
        }
        self.next_turn += 1;
        let turn = TurnId(self.next_turn);
        self.queue.push_back((turn, prompt));
        let mut signals = Vec::new();
        self.advance(&mut signals);
        self.later(signals);
        Ok(turn)
    }

    /// Stops the turn that runs; background work goes on (E23). A prompt still waiting for its checkpoint does not
    /// go out. For the user, it also stops a turn the agent began by itself, as an AI tab does.
    pub fn stop_turn(&mut self) {
        let Some(open) = self.open.as_mut() else {
            if matches!(self.contract, Contract::User(_)) {
                self.backend.cancel();
            }
            return;
        };
        match open.phase {
            Phase::Checkpointing(_) | Phase::Retrying(_) => {
                let mut signals = Vec::new();
                self.end_turn(Outcome::Cancelled, &mut signals);
                self.later(signals);
            }
            Phase::Running => {
                open.cancel = true;
                self.backend.cancel();
            }
        }
    }

    /// Drops the prompts that wait; each of their turns ends as cancelled. How many there were.
    pub fn clear_queue(&mut self) -> usize {
        let dropped: Vec<Signal> = self
            .queue
            .drain(..)
            .map(|(turn, _)| cancelled(turn, &self.thread))
            .collect();
        let count = dropped.len();
        self.later(dropped);
        count
    }

    /// Stops the subagents and background items that run, where the path can; how many it asked to stop.
    pub fn stop_background(&mut self) -> usize {
        let subagents = (self.thread.subagents().iter())
            .filter(|s| !s.status.is_final())
            .filter_map(|s| s.task_id.clone());
        let background = (self.thread.background().iter())
            .filter(|b| !b.status.is_final() && !b.ambient)
            .map(|b| b.id.clone());
        let tasks: Vec<String> = subagents.chain(background).collect();
        tasks.iter().filter(|task| self.backend.stop_task(task)).count()
    }

    /// Stops the subagent or background item `task` alone; false where the path cannot.
    pub fn stop_task(&mut self, task: &str) -> bool {
        self.backend.stop_task(task)
    }

    /// Switches the session's model while it runs (E184); the path's words where it cannot.
    pub fn set_model(&mut self, model: &str) -> Result<(), String> {
        if self.is_ended() {
            return Err(SendError::Ended.to_string());
        }
        self.backend.set_model(model)
    }

    /// What Claude Code offers, such as the models a view lets the user switch to; `None` on the other paths.
    pub fn claude_capabilities(&self) -> Option<ide_claude::Capabilities> {
        self.backend.claude_capabilities()
    }

    /// Ends the session and its process; the turn that runs and the prompts that wait end as cancelled.
    pub fn stop(&mut self) {
        if self.stopping {
            return;
        }
        self.stopping = true;
        self.backend.stop();
        let mut signals = Vec::new();
        if self.open.is_some() {
            self.end_turn(Outcome::Cancelled, &mut signals);
        }
        signals.extend(self.queue.drain(..).map(|(turn, _)| cancelled(turn, &self.thread)));
        self.later(signals);
    }

    /// Reads messages, waiting requests and usage from the agent again, as after a gap; for an order paused by a
    /// failed attempt (C62).
    pub fn resync(&mut self) {
        self.begin_resync();
    }

    /// A new subscriber; its first batch, which `take` hands out at once, is the snapshot.
    pub fn subscribe(&mut self) -> SubscriberId {
        self.feeds.subscribe()
    }

    /// What waits for `subscriber`, emptied; `None` for one that unsubscribed.
    pub fn take(&mut self, subscriber: SubscriberId) -> Option<Batch> {
        self.feeds.take(subscriber, &self.thread)
    }

    /// Ends `subscriber`'s feed, as its view closes; the session runs on.
    pub fn unsubscribe(&mut self, subscriber: SubscriberId) -> bool {
        self.feeds.unsubscribe(subscriber)
    }

    /// How many subscribers follow the session.
    pub fn subscribers(&self) -> usize {
        self.feeds.len()
    }

    /// Takes in what the session and the workers reported since the last flush, on the main thread; the subscribers
    /// whose feed was empty and now holds a batch, which the app tells.
    pub fn flush(&mut self) -> Vec<SubscriberId> {
        let mut pending = self.inbox.take();
        let mut signals = std::mem::take(&mut self.outbox);
        for inbound in pending.inbound.drain(..) {
            self.take_in(inbound, &mut signals);
        }
        self.thread = self.backend.snapshot();
        if !self.ready && !self.is_ended() && self.backend.is_ready() {
            self.ready = true;
            signals.push(Signal::Ready);
        }
        self.decide_requests(&mut signals);
        self.check_turn(&mut signals);
        self.retry(&mut signals);
        self.advance(&mut signals);
        let changes = pending.changes.take();
        self.feeds.publish(&changes, &signals)
    }

    /// Publishes `signals` with the next flush.
    fn later(&mut self, signals: Vec<Signal>) {
        if !signals.is_empty() {
            self.outbox.extend(signals);
            self.inbox.poke();
        }
    }

    fn take_in(&mut self, inbound: Inbound, signals: &mut Vec<Signal>) {
        match inbound {
            Inbound::Capabilities => signals.push(Signal::Capabilities),
            Inbound::Failed(message) => signals.push(Signal::Failed(message)),
            Inbound::Ended { stderr } => {
                self.ended = true;
                signals.push(Signal::Ended { stderr });
            }
            Inbound::Gap => {
                if !matches!(self.stream, StreamState::Paused(_)) {
                    self.stream = StreamState::Gap;
                }
                signals.push(Signal::StreamGap);
            }
            Inbound::Resumed => self.begin_resync(),
            Inbound::Resynced(Ok(())) => {
                if self.stream == StreamState::Resyncing {
                    self.stream = StreamState::Live;
                }
                signals.push(Signal::Resynced);
            }
            Inbound::Resynced(Err(message)) => {
                self.stream = StreamState::Paused(message.clone());
                signals.push(Signal::NeedsPause(message));
            }
            Inbound::Checkpointed { turn, result } => self.checkpointed(turn, result, signals),
            Inbound::FinishFailed { turn, message } => signals.push(Signal::CheckpointFailed { turn, message }),
            Inbound::Finished {
                turn,
                started,
                finished,
            } => signals.push(Signal::CheckpointFinished {
                turn,
                started,
                finished,
            }),
            Inbound::ClaudeFailed(error) => signals.push(Signal::ClaudeFailed(error)),
            Inbound::RemoteControl => signals.push(Signal::RemoteControl),
            Inbound::Other => {}
        }
    }

    fn begin_resync(&mut self) {
        if self.stream == StreamState::Resyncing || self.is_ended() {
            return;
        }
        let inbox = self.inbox.clone();
        self.stream = if self.backend.resync(move |result| inbox.push(Inbound::Resynced(result))) {
            StreamState::Resyncing
        } else {
            StreamState::Live
        };
    }

    /// Ends the turn that runs once the session rests after it, or its process is gone.
    fn check_turn(&mut self, signals: &mut Vec<Signal>) {
        let Some(open) = self.open.as_mut() else { return };
        if !matches!(open.phase, Phase::Running) {
            return;
        }
        let status = self.thread.status();
        let busy = self.backend.busy();
        if status != Status::Idle || busy {
            open.running_seen = true;
        }
        if self.ended || self.backend.is_ended() {
            let outcome = match turn::outcome(&self.thread, open.since) {
                failed @ Outcome::Failed(_) => failed,
                _ if open.cancel => Outcome::Cancelled,
                Outcome::Cancelled => Outcome::Cancelled,
                _ => Outcome::ConnectionLost,
            };
            return self.end_turn(outcome, signals);
        }
        if self.stream != StreamState::Live {
            return;
        }
        let rests = status == Status::Idle && !busy && self.thread.pending_requests().is_empty();
        if rests && (open.running_seen || turn::answered(&self.thread, open.since)) {
            let outcome = turn::outcome(&self.thread, open.since);
            self.end_turn(outcome, signals);
        }
    }

    fn end_turn(&mut self, outcome: Outcome, signals: &mut Vec<Signal>) {
        let Some(open) = self.open.take() else { return };
        if let (Some(checkpoint), Some(checkpointer)) = (&open.checkpoint, &self.checkpointer) {
            checkpointer.finish(open.turn, checkpoint.clone(), self.inbox.clone());
        }
        let end = TurnEnd {
            turn: open.turn,
            outcome,
            checkpoint: open.checkpoint,
            running_work: turn::running_work(&self.thread),
        };
        self.last_end = Some(end.clone());
        signals.push(Signal::TurnEnded(end));
    }

    /// Starts the next prompt that waits, when nothing keeps it: a turn that runs, a session not ready, a gap.
    fn advance(&mut self, signals: &mut Vec<Signal>) {
        if self.open.is_some() || self.queue.is_empty() || !self.is_ready() {
            return;
        }
        match self.stream {
            StreamState::Live => {}
            StreamState::Gap => return self.begin_resync(),
            StreamState::Resyncing | StreamState::Paused(_) => return,
        }
        let Some((turn, mut prompt)) = self.queue.pop_front() else {
            return;
        };
        match self.checkpointer.clone() {
            Some(checkpointer) => {
                let label = match self.contract {
                    Contract::User(_) => claude::label(&prompt.message.text),
                    Contract::Order(_) => first_line(&prompt.message.text, 80),
                };
                let buffers = std::mem::take(&mut prompt.buffers);
                checkpointer.create(turn, label, buffers, self.inbox.clone());
                self.open = Some(Open::new(turn, Phase::Checkpointing(prompt)));
            }
            None => {
                self.open = Some(Open::new(turn, Phase::Running));
                self.deliver(prompt, None, signals);
            }
        }
    }

    fn checkpointed(&mut self, turn: TurnId, result: Result<ide_git::Checkpoint, String>, signals: &mut Vec<Signal>) {
        let prompt = match self.open.as_mut() {
            Some(open) if open.turn == turn && matches!(open.phase, Phase::Checkpointing(_)) => {
                match std::mem::replace(&mut open.phase, Phase::Running) {
                    Phase::Checkpointing(prompt) | Phase::Retrying(prompt) => prompt,
                    Phase::Running => return,
                }
            }
            _ => return,
        };
        match result {
            Ok(checkpoint) => self.deliver(prompt, Some(checkpoint), signals),
            Err(message) if self.contract.order().is_some() => {
                let why = format!("no checkpoint before the prompt: {message}");
                self.end_turn(Outcome::Failed(why), signals);
            }
            Err(message) => {
                signals.push(Signal::CheckpointFailed { turn, message });
                self.deliver(prompt, None, signals);
            }
        }
    }

    /// Sends the prompt of the open turn; a session still starting the turn before gets it with the next flush.
    fn deliver(&mut self, prompt: Prompt, checkpoint: Option<ide_git::Checkpoint>, signals: &mut Vec<Signal>) {
        if self.open.is_none() {
            return;
        }
        let since = turn::last_entry(&self.backend.snapshot());
        let sent = self.backend.send(prompt.message.clone(), prompt.schema.clone());
        let working = self.backend.busy() || self.backend.snapshot().status() != Status::Idle;
        let Some(open) = self.open.as_mut() else { return };
        open.checkpoint = checkpoint;
        let sent = match sent {
            Err(SendFailure::Busy) => {
                open.phase = Phase::Retrying(prompt);
                self.inbox.poke();
                return;
            }
            Ok(id) => Ok(id),
            Err(SendFailure::Failed(message)) => Err(message),
        };
        open.phase = Phase::Running;
        open.since = since;
        open.running_seen = working;
        if !std::mem::replace(&mut open.announced, true) {
            signals.push(Signal::TurnStarted(open.turn));
        }
        signals.push(Signal::Delivered {
            turn: open.turn,
            sent: sent.clone(),
            checkpoint: open.checkpoint.clone(),
        });
        if let Err(message) = sent {
            self.end_turn(Outcome::Failed(message), signals);
        }
    }

    /// Sends a prompt again that the session was not ready to take.
    fn retry(&mut self, signals: &mut Vec<Signal>) {
        let Some(open) = self.open.as_mut() else { return };
        if !matches!(open.phase, Phase::Retrying(_)) {
            return;
        }
        let Phase::Retrying(prompt) = std::mem::replace(&mut open.phase, Phase::Running) else {
            return;
        };
        let checkpoint = open.checkpoint.take();
        self.deliver(prompt, checkpoint, signals);
    }
}

/// The end of a turn whose prompt never went out.
fn cancelled(turn: TurnId, thread: &Thread) -> Signal {
    Signal::TurnEnded(TurnEnd {
        turn,
        outcome: Outcome::Cancelled,
        checkpoint: None,
        running_work: turn::running_work(thread),
    })
}

/// The first line of `text`, cut to `most` characters, as a checkpoint's label.
fn first_line(text: &str, most: usize) -> String {
    let line = text.lines().next().unwrap_or_default();
    line.chars().take(most).collect()
}
