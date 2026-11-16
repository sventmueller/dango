//! What an AI tab holds and decides without AppKit (E98): its session, the queue and the attachments, the picks of
//! model, effort and mode, the checkpoints of its turns, and how sending moves them.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use ide_claude::{Capabilities, PlanAnswer};
use ide_core::explain::Explained;
use ide_git::Checkpoint;
use ide_thread::{Attachment, Change, Entry, Lane, Status, Thread, UserMessage};
use splat_i18n::{t, tf};

use super::acp::Agent;
use super::rewind::Fork;
use super::source::{Problem, Resume, Source};
use crate::own_agent::ModelChoice;

/// What the composer's menus offered, so the picked item's tag can be read.
#[derive(Default)]
pub(crate) struct Offered {
    pub(crate) commands: Vec<String>,
    pub(crate) models: Vec<String>,
    pub(crate) efforts: Vec<String>,
    pub(crate) modes: Vec<String>,
    pub(crate) past: Vec<Resume>,
}

/// What an AI tab holds besides its views: its session, the turn in flight, the user's picks and what the next
/// message takes along.
pub(crate) struct State {
    run: Run,
    thread: Rc<Thread>,
    problem: Option<Problem>,
    custom_title: Option<String>,
    queue: Vec<UserMessage>,
    attachments: Vec<Attachment>,
    sandbox: bool,
    /// Whether the user connected the thread to Remote Control or ended it; `None` follows their Claude settings.
    remote: Option<bool>,
    picks: Picks,
    bookmarked: bool,
    resume: Option<Resume>,
    /// The checkpoint taken before each user message, by the CLI's id for the message.
    checkpoints: HashMap<String, Checkpoint>,
    turn: Turn,
    offered: Offered,
    rewind: Rewind,
    /// The editor's selection, offered as context while the composer has the focus.
    offer: Option<Attachment>,
}

/// The tab's session, with what the tab knows of its agent beyond it.
struct Run {
    source: Source,
    known: Known,
}

/// What the tab learned of its agent or was given for it, by the agent it runs; it outlasts the session, which starts
/// again with it.
enum Known {
    /// What the CLI offers.
    Claude(Capabilities),
    /// What the ACP agent and its session offer, once they said.
    Acp(Option<ide_acp::Capabilities>),
    /// The endpoint and model of the own agent's session.
    Own(Option<ModelChoice>),
}

/// The model, effort and mode the user picked, which the session starts with.
pub(crate) struct Picks {
    mode: String,
    model: Option<String>,
    effort: Option<String>,
    /// The session runs with the model and mode the user picked last for its agent, where it offers them.
    applied: bool,
}

impl Picks {
    pub(crate) fn mode(&self) -> &str {
        &self.mode
    }

    pub(crate) fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub(crate) fn effort(&self) -> Option<&str> {
        self.effort.as_deref()
    }
}

/// The turn in flight: how the session stands, the prompt on its way and the checkpoint taken before it.
struct Turn {
    status: Status,
    /// A checkpoint is being taken before a prompt goes out.
    sending: bool,
    /// When the last message went out, which the stop button waits a moment after.
    sent_at: Option<Instant>,
    /// The checkpoint of the turn that runs.
    checkpoint: Option<Checkpoint>,
    /// The message the registry's driver holds until its checkpoint is taken and it goes out, with its turn.
    handed: Option<(ide_session::TurnId, UserMessage)>,
}

/// The rewind the session started with, until the agent accepted it.
#[derive(Default)]
struct Rewind {
    fork: Option<Fork>,
    /// The prompt sent last, which goes out again when the agent refuses the rewind.
    last_sent: Option<UserMessage>,
}

/// How the sandbox button looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SandboxLook {
    pub(crate) symbol: &'static str,
    /// Green while the sandbox is on, grey while it is off.
    pub(crate) on: bool,
    pub(crate) tip: &'static str,
}

impl State {
    /// The state of a tab running `agent` before its session starts, with the user's last picks for the agent.
    pub(crate) fn new(
        agent: &Agent,
        source: Source,
        thread: Rc<Thread>,
        choice: ide_core::settings::AgentChoice,
    ) -> State {
        let known = match agent {
            Agent::Claude => Known::Claude(Capabilities::default()),
            Agent::Acp(_) => Known::Acp(None),
            Agent::Own => Known::Own(None),
        };
        State {
            run: Run { source, known },
            thread,
            problem: None,
            custom_title: None,
            queue: Vec::new(),
            attachments: Vec::new(),
            sandbox: false,
            remote: None,
            picks: Picks {
                mode: choice.mode.unwrap_or_else(|| "default".into()),
                model: choice.model,
                effort: choice.effort,
                applied: false,
            },
            bookmarked: false,
            resume: None,
            checkpoints: HashMap::new(),
            turn: Turn {
                status: Status::Idle,
                sending: false,
                sent_at: None,
                checkpoint: None,
                handed: None,
            },
            offered: Offered::default(),
            rewind: Rewind::default(),
            offer: None,
        }
    }

    /// The session's title: the user's, else the agent's, else the first line of the first message, else `untitled`.
    pub(crate) fn title(&self, untitled: &str) -> String {
        let first_message = || {
            self.thread.entries(&Lane::Main).iter().find_map(|e| match &e.kind {
                ide_thread::EntryKind::UserMessage(m) => {
                    Some(ide_thread::title_from(&m.text)).filter(|t| !t.is_empty())
                }
                _ => None,
            })
        };
        self.custom_title
            .clone()
            .filter(|t| !t.is_empty())
            .or_else(|| Some(self.thread.title().to_string()).filter(|t| !t.is_empty()))
            .or_else(first_message)
            .unwrap_or_else(|| untitled.to_string())
    }

    /// Takes the first queued message off the queue, once the session rests and nothing else is on its way.
    pub(crate) fn next_queued(&mut self) -> Option<UserMessage> {
        if self.running() || !self.run.source.is_live() || self.queue.is_empty() {
            return None;
        }
        Some(self.queue.remove(0))
    }

    /// Takes in what the tab opens with: the past session `resume`, and whether the user bookmarked the thread.
    pub(crate) fn opened(&mut self, resume: Option<Resume>, bookmarked: bool) {
        self.custom_title = resume.as_ref().and_then(|r| r.title.clone());
        self.bookmarked = bookmarked;
        self.resume = resume;
    }

    pub(crate) fn thread(&self) -> &Rc<Thread> {
        &self.thread
    }

    /// The past session the tab continues.
    pub(crate) fn resume(&self) -> Option<&Resume> {
        self.resume.as_ref()
    }

    pub(crate) fn custom_title(&self) -> Option<&str> {
        self.custom_title.as_deref()
    }

    pub(crate) fn set_custom_title(&mut self, title: String) {
        self.custom_title = Some(title);
    }

    pub(crate) fn bookmarked(&self) -> bool {
        self.bookmarked
    }

    pub(crate) fn set_bookmarked(&mut self, on: bool) {
        self.bookmarked = on;
    }

    /// Bookmarks the thread or takes the bookmark off; whether it is bookmarked now.
    pub(crate) fn toggle_bookmark(&mut self) -> bool {
        self.bookmarked = !self.bookmarked;
        self.bookmarked
    }

    pub(crate) fn sandbox(&self) -> bool {
        self.sandbox
    }

    /// Switches the sandbox on or off; whether it is on now.
    pub(crate) fn toggle_sandbox(&mut self) -> bool {
        self.sandbox = !self.sandbox;
        self.sandbox
    }

    pub(crate) fn remote(&self) -> Option<bool> {
        self.remote
    }

    pub(crate) fn set_remote(&mut self, on: bool) {
        self.remote = Some(on);
    }

    pub(crate) fn problem(&self) -> Option<&Problem> {
        self.problem.as_ref()
    }

    pub(crate) fn set_problem(&mut self, problem: Option<Problem>) {
        self.problem = problem;
    }

    pub(crate) fn queue(&self) -> &[UserMessage] {
        &self.queue
    }

    pub(crate) fn enqueue(&mut self, message: UserMessage) {
        self.queue.push(message);
    }

    /// What the next message takes along.
    pub(crate) fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    pub(crate) fn attach(&mut self, attachments: impl IntoIterator<Item = Attachment>) {
        self.attachments.extend(attachments);
    }

    /// Takes the attachments off for the message that goes out.
    pub(crate) fn take_attachments(&mut self) -> Vec<Attachment> {
        std::mem::take(&mut self.attachments)
    }

    /// The editor's selection, offered as context.
    pub(crate) fn offer(&self) -> Option<&Attachment> {
        self.offer.as_ref()
    }

    /// Offers the editor's selection `offer`, unless the next message takes it along already; whether the offer
    /// changed.
    pub(crate) fn offer_selection(&mut self, offer: Option<Attachment>) -> bool {
        let offer = offer.filter(|o| !self.attachments.contains(o));
        let changed = self.offer != offer;
        self.offer = offer;
        changed
    }

    pub(crate) fn take_offer(&mut self) -> Option<Attachment> {
        self.offer.take()
    }

    /// The checkpoint taken before each user message, by the CLI's id for the message.
    pub(crate) fn checkpoints(&self) -> &HashMap<String, Checkpoint> {
        &self.checkpoints
    }

    pub(crate) fn offered(&self) -> &Offered {
        &self.offered
    }

    /// What the composer's menus offer, to be set as one of them is built.
    pub(crate) fn offered_mut(&mut self) -> &mut Offered {
        &mut self.offered
    }

    pub(crate) fn source(&self) -> &Source {
        &self.run.source
    }

    /// Notes that the session is being started.
    pub(crate) fn start(&mut self) {
        self.run.source = Source::Starting;
        self.problem = None;
        self.picks.applied = false;
    }

    /// Notes that `source` runs the session now.
    pub(crate) fn started(&mut self, source: Source) {
        self.run.source = source;
        self.problem = None;
    }

    /// Notes that the session did not start, for `problem`.
    pub(crate) fn failed(&mut self, problem: Problem) {
        self.run.source = Source::Idle;
        self.problem = Some(problem);
    }

    /// Takes the session off the tab, for the caller to end it; a message its driver still held before its checkpoint
    /// goes back to the head of the queue, for the next session.
    pub(crate) fn end(&mut self) -> Source {
        if let Some((_, message)) = self.turn.handed.take() {
            self.turn.sending = false;
            self.queue.insert(0, message);
        }
        std::mem::replace(&mut self.run.source, Source::Idle)
    }

    /// Takes in what the session says it offers.
    pub(crate) fn learn(&mut self) {
        match &mut self.run.known {
            Known::Claude(caps) => {
                if let Some(learned) = self.run.source.capabilities() {
                    *caps = learned;
                }
            }
            Known::Acp(caps) => {
                if let Some(learned) = self.run.source.acp_capabilities() {
                    *caps = Some(learned);
                }
            }
            Known::Own(_) => {}
        }
    }

    /// What the CLI offers, in a Claude tab.
    pub(crate) fn claude_caps(&self) -> Option<&Capabilities> {
        match &self.run.known {
            Known::Claude(caps) => Some(caps),
            _ => None,
        }
    }

    /// What the ACP agent offers, in an ACP tab once it said.
    pub(crate) fn acp_caps(&self) -> Option<&ide_acp::Capabilities> {
        match &self.run.known {
            Known::Acp(caps) => caps.as_ref(),
            _ => None,
        }
    }

    /// The endpoint and model of the own agent's session.
    pub(crate) fn own_choice(&self) -> Option<&ModelChoice> {
        match &self.run.known {
            Known::Own(choice) => choice.as_ref(),
            _ => None,
        }
    }

    /// Picks the endpoint and model the own agent's session runs with; other agents have none.
    pub(crate) fn pick_own(&mut self, picked: Option<ModelChoice>) {
        if let Known::Own(choice) = &mut self.run.known {
            *choice = picked;
        }
    }

    /// Answers a plan; what a recording played for the self-test changed.
    pub(crate) fn answer_plan(&mut self, request: &str, answer: PlanAnswer) -> Result<Vec<Change>, Explained> {
        self.run.source.answer_plan(request, answer)
    }

    pub(crate) fn picks(&self) -> &Picks {
        &self.picks
    }

    pub(crate) fn pick_model(&mut self, model: String) {
        self.picks.model = Some(model);
    }

    pub(crate) fn pick_effort(&mut self, level: String) {
        self.picks.effort = Some(level);
    }

    pub(crate) fn pick_mode(&mut self, mode: String) {
        self.picks.mode = mode;
    }

    /// What the ACP agent offers, for the picks the user made last for it to be put on its session; once per
    /// session, as soon as it offers modes or options.
    pub(crate) fn apply_picks(&mut self) -> Option<ide_acp::Capabilities> {
        let caps = self
            .acp_caps()
            .filter(|caps| !self.picks.applied && (!caps.modes.is_empty() || !caps.options.is_empty()))
            .cloned()?;
        self.picks.applied = true;
        Some(caps)
    }

    pub(crate) fn status(&self) -> Status {
        self.turn.status
    }

    /// Whether a checkpoint is being taken before a prompt goes out.
    pub(crate) fn is_sending(&self) -> bool {
        self.turn.sending
    }

    /// When the last message went out.
    pub(crate) fn sent_at(&self) -> Option<Instant> {
        self.turn.sent_at
    }

    /// The rewind the session starts with, until the agent accepted it.
    pub(crate) fn fork(&self) -> Option<&Fork> {
        self.rewind.fork.as_ref()
    }

    /// Takes in the thread as the session reports it, with its status.
    pub(crate) fn absorbed(&mut self, thread: Rc<Thread>) {
        self.turn.status = thread.status();
        self.thread = thread;
    }

    /// Notes that a checkpoint is being taken before a prompt goes out.
    pub(crate) fn begin_send(&mut self) {
        self.turn.sending = true;
    }

    /// Notes that the registry's driver took `message`, which begins `turn` once its checkpoint is taken.
    pub(crate) fn handed_over(&mut self, turn: ide_session::TurnId, message: UserMessage) {
        self.turn.handed = Some((turn, message));
    }

    /// The message that began `turn` and went out now, which the driver took from the tab.
    pub(crate) fn take_handed(&mut self, turn: ide_session::TurnId) -> Option<UserMessage> {
        match self.turn.handed.take() {
            Some((handed, message)) if handed == turn => Some(message),
            other => {
                self.turn.handed = other;
                None
            }
        }
    }

    /// Notes that `message` goes out now that its checkpoint is taken.
    pub(crate) fn delivering(&mut self, message: &UserMessage) {
        self.turn.sending = false;
        // A refusal comes with the result of the first prompt on the rewound session, so by the second the rewind
        // holds.
        if self.rewind.fork.as_ref().is_some_and(|f| f.prompted) {
            self.rewind.fork = None;
        } else if let Some(fork) = self.rewind.fork.as_mut() {
            fork.prompted = true;
        }
        self.rewind.last_sent = Some(message.clone());
    }

    /// Notes that the message went out now with `checkpoint` taken before it; `id` is the message's id, the agent's
    /// or the app's own.
    pub(crate) fn sent(&mut self, id: Option<String>, checkpoint: Option<Checkpoint>) {
        if let Some(checkpoint) = checkpoint {
            if let Some(id) = id {
                self.checkpoints.insert(id, checkpoint.clone());
            }
            self.turn.checkpoint = Some(checkpoint);
        }
        self.turn.sent_at = Some(Instant::now());
    }

    /// Takes the checkpoint of the turn that just ended.
    pub(crate) fn turn_ended(&mut self) -> Option<Checkpoint> {
        self.turn.checkpoint.take()
    }

    /// Puts `finished` in place of the kept checkpoints whose commit is `started`.
    pub(crate) fn checkpoint_finished(&mut self, started: &ide_git::CommitId, finished: &Checkpoint) {
        for kept in self.checkpoints.values_mut() {
            if kept.commit == *started {
                *kept = finished.clone();
            }
        }
    }

    /// Goes on with `kept`, the thread before a prompt, and that prompt's `attachments`, until the agent accepts
    /// `fork`.
    pub(crate) fn rewound(&mut self, kept: Thread, attachments: &[Attachment], fork: Fork) {
        self.restart_with(kept);
        self.resume = None;
        self.attachments = attachments.to_vec();
        self.rewind.fork = Some(fork);
    }

    /// Takes the rewind the agent refused, with the prompt that met the refusal.
    pub(crate) fn take_refused_rewind(&mut self) -> (Option<Fork>, Option<UserMessage>) {
        (self.rewind.fork.take(), self.rewind.last_sent.take())
    }

    /// Goes back to `before`, the thread of a refused rewind, with `message` first in the queue.
    pub(crate) fn rewind_undone(&mut self, before: Thread, message: Option<UserMessage>) {
        self.restart_with(before);
        if let Some(message) = message {
            self.queue.insert(0, message);
        }
    }

    fn restart_with(&mut self, thread: Thread) {
        self.thread = Rc::new(thread);
        self.turn.status = Status::Idle;
        self.turn.checkpoint = None;
    }

    pub(crate) fn remove_queued(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);
        }
    }

    pub(crate) fn remove_attachment(&mut self, index: usize) {
        if index < self.attachments.len() {
            self.attachments.remove(index);
        }
    }

    /// Whether a turn runs or a prompt is on its way, so the send button stops instead.
    pub(crate) fn running(&self) -> bool {
        self.turn.status != Status::Idle || self.turn.sending
    }

    /// Whether there is something to send with `text` in the composer.
    pub(crate) fn can_send(&self, text: &str) -> bool {
        !text.trim().is_empty() || !self.attachments.is_empty()
    }

    /// What the empty composer says.
    pub(crate) fn placeholder(&self) -> &'static str {
        if self.turn.sending {
            t("Sicherungspunkt wird angelegt …")
        } else {
            t("⌘Esc zum Fokussieren oder Verlassen")
        }
    }

    /// The model pill of a Claude session: the model as people say it, with the effort if one is set.
    pub(crate) fn model_pill(&self) -> String {
        let model_id = self
            .thread
            .model()
            .map(str::to_string)
            .or_else(|| self.picks.model.clone());
        let model = model_id
            .as_deref()
            .map(|id| {
                self.claude_caps()
                    .and_then(|caps| caps.models.iter().find(|m| m.value == id))
                    .map_or_else(|| model_name(id), |m| m.display_name.clone())
            })
            .unwrap_or_else(|| t("Standardmodell").to_string());
        let effort = self
            .thread
            .effort()
            .map(str::to_string)
            .or_else(|| self.picks.effort.clone());
        match effort {
            Some(effort) => tf("{} · {}", &[&model, &effort_name(&effort)]),
            None => model,
        }
    }

    /// The mode pill of a Claude session: its title and symbol.
    pub(crate) fn mode_pill(&self) -> (String, &'static str) {
        let mode_id = self
            .thread
            .mode()
            .map_or_else(|| self.picks.mode.clone(), str::to_string);
        let mode_id = if mode_id == "manual" {
            "default".to_string()
        } else {
            mode_id
        };
        modes()
            .iter()
            .find(|(id, _, _)| *id == mode_id)
            .map_or((mode_id.clone(), "hand.raised"), |(_, title, symbol)| {
                ((*title).to_string(), *symbol)
            })
    }

    pub(crate) fn sandbox_look(&self) -> SandboxLook {
        if self.sandbox {
            SandboxLook {
                symbol: "checkmark.shield.fill",
                on: true,
                tip: t("Sandbox an: Shell-Befehle laufen abgeschirmt; Datei- und Webwerkzeuge nicht"),
            }
        } else {
            SandboxLook {
                symbol: "shield.slash",
                on: false,
                tip: t("Sandbox aus: Shell-Befehle laufen ohne Abschirmung"),
            }
        }
    }
}

/// The modes of the CLI's permission prompt, with how the composer names them.
pub(crate) fn modes() -> [(&'static str, &'static str, &'static str); 5] {
    [
        ("default", t("Manuell"), "hand.raised"),
        ("acceptEdits", t("Änderungen annehmen"), "pencil.and.outline"),
        ("plan", t("Plan"), "list.bullet.clipboard"),
        ("auto", t("Auto"), "wand.and.stars"),
        ("bypassPermissions", t("Umgehen"), "exclamationmark.shield"),
    ]
}

/// The name of an effort level as the composer shows it.
pub(crate) fn effort_name(level: &str) -> String {
    match level {
        "minimal" => t("Minimal").to_string(),
        "low" => t("Niedrig").to_string(),
        "medium" => t("Mittel").to_string(),
        "high" => t("Hoch").to_string(),
        "xhigh" => t("Extra hoch").to_string(),
        "max" => t("Maximal").to_string(),
        other => other.to_string(),
    }
}

/// A model's id as people say it: `claude-opus-5-5` as `Opus 5.5`.
pub(crate) fn model_name(id: &str) -> String {
    let id = id.split('[').next().unwrap_or(id);
    let rest = id.strip_prefix("claude-").unwrap_or(id);
    let mut parts = rest.split('-').filter(|p| p.len() < 8);
    let Some(family) = parts.next() else {
        return id.to_string();
    };
    let mut name: String = family
        .chars()
        .enumerate()
        .map(|(n, c)| if n == 0 { c.to_ascii_uppercase() } else { c })
        .collect();
    let version: Vec<&str> = parts.collect();
    if !version.is_empty() {
        name.push(' ');
        name.push_str(&version.join("."));
    }
    name
}

/// The entry numbered `id` in any lane of `thread`.
pub(crate) fn find_entry(thread: &Thread, id: u64) -> Option<Arc<Entry>> {
    let lanes = std::iter::once(Lane::Main).chain(thread.subagents().iter().map(|s| Lane::Subagent(s.key.clone())));
    for lane in lanes {
        if let Some(entry) = thread.entries(&lane).iter().find(|e| e.id.get() == id) {
            return Some(entry.clone());
        }
    }
    None
}

#[cfg(feature = "selftest")]
mod selftest_probe;

#[cfg(test)]
mod tests {
    use super::*;
    use ide_thread::ThreadId;

    fn state() -> State {
        let thread = Thread::new(ThreadId::new(), "claude", std::path::Path::new("/p"));
        State::new(
            &Agent::Claude,
            Source::Idle,
            Rc::new(thread),
            ide_core::settings::AgentChoice::default(),
        )
    }

    fn message(text: &str) -> UserMessage {
        UserMessage {
            text: text.to_string(),
            ..UserMessage::default()
        }
    }

    #[test]
    fn model_ids_read_as_people_say_them() {
        assert_eq!(model_name("claude-opus-5-5"), "Opus 5.5");
        assert_eq!(model_name("claude-haiku-5-5[1m]"), "Haiku 5.5");
        assert_eq!(model_name("claude-sonnet-4-5-20250929"), "Sonnet 4.5");
        assert_eq!(model_name("opus"), "Opus");
    }

    #[test]
    fn the_queue_waits_while_the_session_is_not_live() {
        let mut state = state();
        state.queue = vec![message("a"), message("b")];
        assert!(state.next_queued().is_none());
        state.remove_queued(5);
        state.remove_queued(0);
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.queue[0].text, "b");
    }

    #[test]
    fn a_checkpoint_keys_the_message_it_was_taken_before() {
        let mut state = state();
        state.begin_send();
        assert!(state.running());
        state.delivering(&message("hi"));
        assert!(!state.running());
        assert_eq!(state.rewind.last_sent.as_ref().map(|m| m.text.as_str()), Some("hi"));
        state.sent(Some("m1".into()), None);
        assert!(state.checkpoints.is_empty() && state.turn.checkpoint.is_none());
    }

    #[test]
    fn something_to_send_is_text_or_an_attachment() {
        let mut state = state();
        assert!(!state.can_send("  \n"));
        assert!(state.can_send("x"));
        state.attachments.push(Attachment::File {
            path: "/p/a.rs".into(),
            lines: None,
        });
        assert!(state.can_send(""));
        state.remove_attachment(0);
        assert!(!state.can_send(""));
    }

    #[test]
    fn the_pills_name_model_effort_and_mode() {
        let mut state = state();
        assert_eq!(state.model_pill(), t("Standardmodell"));
        state.pick_model("claude-opus-5-5".into());
        state.pick_effort("high".into());
        assert_eq!(state.model_pill(), tf("{} · {}", &["Opus 5.5", t("Hoch")]));
        state.pick_mode("manual".into());
        assert_eq!(state.mode_pill(), (t("Manuell").to_string(), "hand.raised"));
        state.pick_mode("plan".into());
        assert_eq!(state.mode_pill().1, "list.bullet.clipboard");
        assert!(!state.sandbox_look().on);
    }

    #[test]
    fn the_title_falls_back_to_the_first_message_then_to_untitled() {
        let mut state = state();
        assert_eq!(state.title("Neu"), "Neu");
        state.custom_title = Some(String::new());
        assert_eq!(state.title("Neu"), "Neu");
        state.custom_title = Some("Mine".into());
        assert_eq!(state.title("Neu"), "Mine");
    }
}
