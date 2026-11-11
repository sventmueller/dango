//! A thread of the own agent: the conversation, the shared thread model it fills, and the turn that runs on a thread
//! of its own while the user waits, approves or cancels. It is driven like an ACP session, so the AI tab can show
//! both alike.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use base64::Engine;
use ide_thread::{Change, EntryKind, Lane, Status, Thread, ThreadId, Update, UserMessage};

use crate::Provider;
use crate::chat::Cancel;
use crate::conversation::{Block, Message, Role};
use crate::notes::Wording;
use crate::policy::{Approval, Policy, Profile, Rule};
use crate::prices::{Price, PriceTable};
use crate::store::{self, LoadError, Stored, Totals};
use crate::workspace::Workspace;

/// The agent's id in the thread index.
pub const AGENT_ID: &str = "own-agent";

/// What a thread starts with.
#[derive(Clone)]
pub struct Config {
    /// The thread's id.
    pub thread: ThreadId,
    /// The endpoint and key it talks to.
    pub provider: Provider,
    /// The model.
    pub model: String,
    /// How hard the model thinks, where it takes that.
    pub effort: Option<String>,
    /// Where it works.
    pub workspace: Workspace,
    /// What it may do.
    pub policy: Policy,
    /// The user's prices.
    pub prices: PriceTable,
    /// The model's price as its endpoint lists it.
    pub listed_price: Option<Price>,
    /// How many tokens the model's context window holds.
    pub context_size: Option<u64>,
    /// The most tokens one answer may take.
    pub max_tokens: Option<u64>,
    /// Where the conversation is kept for resuming; nowhere when not set.
    pub store: Option<PathBuf>,
    /// The words for what the agent tells the user (E41).
    pub wording: Arc<dyn Wording>,
}

/// What a session tells the UI, on one of its threads.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The thread changed; read it with `Session::snapshot`.
    Changed(Vec<Change>),
    /// The user stored a rule for the project, which the app keeps.
    RuleAdded(Rule),
    /// Something went wrong outside a turn, such as saving the conversation.
    Failed(String),
}

/// The user's answer to a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// The option at this place in the request's list.
    Option(usize),
    /// No option: the call is declined.
    Dismiss,
}

/// Why a session cannot do what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// A turn is running.
    Busy,
    /// There is no request with that id waiting.
    NoRequest,
    /// The session was stopped.
    Closed,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SessionError::Busy => "the agent is still working",
            SessionError::NoRequest => "no such request is waiting",
            SessionError::Closed => "the session has ended",
        })
    }
}

impl std::error::Error for SessionError {}

/// What changes while the session lives.
pub(crate) struct State {
    pub(crate) thread: Thread,
    pub(crate) conversation: Vec<Message>,
    pub(crate) provider: Provider,
    pub(crate) model: String,
    pub(crate) effort: Option<String>,
    pub(crate) policy: Policy,
    pub(crate) listed_price: Option<Price>,
    pub(crate) context_size: Option<u64>,
    /// How many tokens the context window held at the last answer; not known after compacting or resuming.
    pub(crate) context_used: Option<u64>,
    /// The compaction whose notice waits for the next answer to learn the tokens left.
    pub(crate) compacted: Option<Compacted>,
    pub(crate) totals: Totals,
    pub(crate) by_model: BTreeMap<String, Totals>,
    pub(crate) running: bool,
    pub(crate) closed: bool,
    pub(crate) cancel: Cancel,
    pub(crate) asks: HashMap<String, Sender<Choice>>,
    /// Numbers the entries of the timeline.
    pub(crate) counter: u64,
}

/// A compaction's notice, until the next answer tells how full the context window is after it.
#[derive(Debug, Clone)]
pub(crate) struct Compacted {
    pub(crate) key: String,
    pub(crate) manual: bool,
    pub(crate) tokens_before: u64,
}

type Sink = Box<dyn Fn(Event) + Send + Sync>;

pub(crate) struct Shared {
    pub(crate) state: Mutex<State>,
    pub(crate) sink: Sink,
    pub(crate) workspace: Workspace,
    pub(crate) prices: PriceTable,
    pub(crate) max_tokens: Option<u64>,
    pub(crate) store: Option<PathBuf>,
    pub(crate) rules: Vec<(String, String)>,
    pub(crate) wording: Arc<dyn Wording>,
}

impl Shared {
    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Applies `updates` to the thread and tells the UI what changed.
    pub(crate) fn apply(&self, updates: Vec<Update>) {
        let changes = self.lock().thread.apply_all(updates);
        if !changes.is_empty() {
            (self.sink)(Event::Changed(changes));
        }
    }

    /// A key no entry has yet.
    pub(crate) fn key(&self, prefix: &str) -> String {
        let mut state = self.lock();
        state.counter += 1;
        format!("{prefix}{}", state.counter)
    }

    /// Keeps the conversation where resuming finds it.
    pub(crate) fn save(&self) {
        let Some(path) = &self.store else { return };
        let stored = {
            let state = self.lock();
            Stored {
                version: 0,
                title: state.thread.title().to_string(),
                endpoint: state.provider.endpoint().id.clone(),
                model: state.model.clone(),
                messages: state.conversation.clone(),
                totals: state.totals,
                by_model: state.by_model.clone(),
            }
        };
        if let Err(error) = store::save(path, &stored) {
            (self.sink)(Event::Failed(error));
        }
    }
}

/// A running thread of the own agent; clones share it.
#[derive(Clone)]
pub struct Session {
    shared: Arc<Shared>,
}

impl Session {
    /// A new thread, empty until the first message.
    pub fn start(config: Config, sink: impl Fn(Event) + Send + Sync + 'static) -> Session {
        Session::open(config, Stored::default(), sink)
    }

    /// The thread kept in the configuration's store, its conversation shown again and continued.
    pub fn resume(config: Config, sink: impl Fn(Event) + Send + Sync + 'static) -> Result<Session, LoadError> {
        let path = config.store.clone().ok_or(LoadError::NoStore)?;
        let stored = store::load(&path)?;
        Ok(Session::open(config, stored, sink))
    }

    fn open(config: Config, stored: Stored, sink: impl Fn(Event) + Send + Sync + 'static) -> Session {
        let thread = Thread::new(config.thread.clone(), AGENT_ID, config.workspace.root());
        let rules = crate::prompt::project_rules(config.workspace.root());
        let state = State {
            thread,
            conversation: Vec::new(),
            provider: config.provider,
            model: config.model.clone(),
            effort: config.effort.clone(),
            policy: config.policy,
            listed_price: config.listed_price,
            context_size: config.context_size,
            context_used: None,
            compacted: None,
            totals: stored.totals,
            by_model: stored.by_model.clone(),
            running: false,
            closed: false,
            cancel: Cancel::new(),
            asks: HashMap::new(),
            counter: 0,
        };
        let session = Session {
            shared: Arc::new(Shared {
                state: Mutex::new(state),
                sink: Box::new(sink),
                workspace: config.workspace,
                prices: config.prices,
                max_tokens: config.max_tokens,
                store: config.store,
                rules,
                wording: config.wording,
            }),
        };
        let mut updates = vec![
            Update::Session(config.thread.as_str().to_string()),
            Update::Model(config.model),
            Update::Effort(config.effort),
            Update::Mode(mode_id(session.shared.lock().policy.approval).into()),
            Update::Status(Status::Idle),
        ];
        if !stored.title.is_empty() {
            updates.push(Update::Title(stored.title.clone()));
        }
        updates.extend(crate::replay::updates(
            &stored.messages,
            &session.shared.workspace,
            &*session.shared.wording,
        ));
        session.shared.lock().conversation = stored.messages;
        updates.push(Update::Usage(crate::turn::usage(&session.shared)));
        session.shared.apply(updates);
        session
    }

    /// The thread as it is now.
    pub fn snapshot(&self) -> Thread {
        self.shared.lock().thread.clone()
    }

    /// Reads the thread without copying it.
    pub fn with_thread<R>(&self, read: impl FnOnce(&Thread) -> R) -> R {
        read(&self.shared.lock().thread)
    }

    /// Whether a turn runs now.
    pub fn is_running(&self) -> bool {
        self.shared.lock().running
    }

    /// Sends the user's message and runs the turn it starts on a thread of its own; `/compact`, with what to keep
    /// in mind after it, compacts the conversation instead (E42).
    pub fn send(&self, message: UserMessage) -> Result<(), SessionError> {
        if let Some(focus) = compact_command(&message.text) {
            return self.compact(focus);
        }
        let cancel = self.begin()?;
        let text = with_attachments(&message, &self.shared.workspace);
        let key = self.shared.key("u");
        let mut updates = vec![Update::Put {
            lane: Lane::Main,
            key: Some(key),
            at: SystemTime::now(),
            entry: EntryKind::UserMessage(message.clone()),
        }];
        if self.shared.lock().thread.title().is_empty() {
            updates.push(Update::Title(ide_thread::title_from(&message.text)));
        }
        self.shared.apply(updates);
        let mut content = vec![Block::text(&text)];
        content.extend(message.attachments.iter().filter_map(|attachment| match attachment {
            ide_thread::Attachment::Image { media_type, data, .. } => Some(Block::Image {
                media_type: media_type.clone(),
                data: base64::engine::general_purpose::STANDARD.encode(data),
            }),
            _ => None,
        }));
        {
            // A turn that failed before any answer, or a compaction, leaves a user message last; providers want the
            // roles to alternate, so the new message joins it.
            let mut state = self.shared.lock();
            match state.conversation.last_mut() {
                Some(last) if last.role == Role::User => last.content.extend(content),
                _ => state.conversation.push(Message {
                    role: Role::User,
                    content,
                }),
            }
        }
        self.shared.save();
        self.spawn("own-agent-turn", crate::turn::run, cancel);
        Ok(())
    }

    /// Compacts the conversation now, keeping `focus` in mind (E42).
    pub fn compact(&self, focus: Option<String>) -> Result<(), SessionError> {
        let cancel = self.begin()?;
        self.spawn(
            "own-agent-compact",
            move |shared, cancel| crate::turn::compact(shared, cancel, focus.as_deref()),
            cancel,
        );
        Ok(())
    }

    /// Marks the session busy for a turn of its own; why it cannot start one.
    fn begin(&self) -> Result<Cancel, SessionError> {
        let mut state = self.shared.lock();
        if state.closed {
            return Err(SessionError::Closed);
        }
        if state.running {
            return Err(SessionError::Busy);
        }
        state.running = true;
        state.cancel = Cancel::new();
        Ok(state.cancel.clone())
    }

    fn spawn(&self, name: &str, work: impl FnOnce(&Shared, &Cancel) + Send + 'static, cancel: Cancel) {
        let shared = self.shared.clone();
        let started = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || work(&shared, &cancel));
        if started.is_err() {
            self.shared.lock().running = false;
        }
    }

    /// Stops the running turn: the stream ends, a waiting request is withdrawn, a command is stopped.
    pub fn cancel(&self) {
        self.shared.lock().cancel.cancel();
    }

    /// Answers the permission request `request` with `choice`.
    pub fn answer(&self, request: &str, choice: Choice) -> Result<(), SessionError> {
        let sender = self.shared.lock().asks.remove(request).ok_or(SessionError::NoRequest)?;
        sender.send(choice).map_err(|_| SessionError::NoRequest)
    }

    /// Changes the model for the next request.
    pub fn set_model(&self, model: &str, listed_price: Option<Price>, context_size: Option<u64>) {
        {
            let mut state = self.shared.lock();
            state.model = model.to_string();
            state.listed_price = listed_price;
            state.context_size = context_size;
        }
        self.shared.apply(vec![Update::Model(model.to_string())]);
    }

    /// Changes how hard the model thinks, from the next request on.
    pub fn set_effort(&self, effort: Option<&str>) {
        self.shared.lock().effort = effort.map(str::to_string);
        self.shared.apply(vec![Update::Effort(effort.map(str::to_string))]);
    }

    /// Changes which tools the model is offered.
    pub fn set_profile(&self, profile: Profile) {
        self.shared.lock().policy.profile = profile;
    }

    /// Changes what runs without asking.
    pub fn set_approval(&self, approval: Approval) {
        self.shared.lock().policy.approval = approval;
        self.shared.apply(vec![Update::Mode(mode_id(approval).into())]);
    }

    /// Turns the thread's sandbox on or off (E19).
    pub fn set_sandbox(&self, on: bool) {
        self.shared.lock().policy.sandbox = on;
    }

    /// The thread's policy as it is now.
    pub fn policy(&self) -> Policy {
        self.shared.lock().policy.clone()
    }

    /// Ends the session; a running turn is cancelled.
    pub fn stop(&self) {
        let mut state = self.shared.lock();
        state.closed = true;
        state.cancel.cancel();
        state.asks.clear();
    }
}

/// The id of an approval mode, as the thread's mode.
pub fn mode_id(approval: Approval) -> &'static str {
    match approval {
        Approval::Manual => "manual",
        Approval::Edits => "edits",
        Approval::All => "all",
    }
}

/// What to keep in mind when the message is `/compact`; `None` when it is another message.
fn compact_command(text: &str) -> Option<Option<String>> {
    let rest = text.trim().strip_prefix("/compact")?;
    if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        return None;
    }
    let focus = rest.trim();
    Some((!focus.is_empty()).then(|| focus.to_string()))
}

/// The message as the model reads it: the text, then the files and selections attached to it.
fn with_attachments(message: &UserMessage, workspace: &Workspace) -> String {
    let mut text = message.text.clone();
    for attachment in &message.attachments {
        match attachment {
            ide_thread::Attachment::File { path, lines } => {
                let shown = workspace.display(path);
                let Ok(content) = std::fs::read_to_string(path) else {
                    continue;
                };
                let content: String = match lines {
                    Some((first, last)) => content
                        .lines()
                        .skip(usize::try_from(first.saturating_sub(1)).unwrap_or(0))
                        .take(usize::try_from(last.saturating_sub(*first) + 1).unwrap_or(0))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    None => content.chars().take(100_000).collect(),
                };
                text.push_str(&format!("\n\n<file path=\"{shown}\">\n{content}\n</file>"));
            }
            ide_thread::Attachment::Selection {
                path,
                lines,
                text: selected,
            } => {
                text.push_str(&format!(
                    "\n\n<selection path=\"{}\" lines=\"{}-{}\">\n{selected}\n</selection>",
                    workspace.display(path),
                    lines.0,
                    lines.1
                ));
            }
            ide_thread::Attachment::External {
                source, text: outside, ..
            } => {
                text.push_str("\n\n");
                text.push_str(&ide_thread::external_text(source, outside));
            }
            ide_thread::Attachment::Image { .. } => {}
        }
    }
    text
}
