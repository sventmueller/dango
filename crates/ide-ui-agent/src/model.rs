//! What an AI tab holds and decides without AppKit (E98): its session, the queue and the attachments, the picks of
//! model, effort and mode, the checkpoints of its turns, and how sending moves them.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use ide_claude::Capabilities;
use ide_git::Checkpoint;
use ide_thread::{Attachment, Entry, Lane, Status, Thread, UserMessage};
use splat_i18n::{t, tf};

use super::rewind::Fork;
use super::source::{Problem, Resume, Source};

/// What the composer's menus offered, so the picked item's tag can be read.
#[derive(Default)]
pub struct Offered {
    pub commands: Vec<String>,
    pub models: Vec<String>,
    pub efforts: Vec<String>,
    pub modes: Vec<String>,
    pub past: Vec<Resume>,
}

pub struct State {
    pub source: Source,
    pub thread: Rc<Thread>,
    pub caps: Capabilities,
    /// What the ACP agent and its session offer.
    pub acp: Option<ide_acp::Capabilities>,
    /// The endpoint and model of the own agent's session.
    pub own: Option<crate::own_agent::ModelChoice>,
    pub problem: Option<Problem>,
    pub custom_title: Option<String>,
    pub queue: Vec<UserMessage>,
    pub attachments: Vec<Attachment>,
    pub sandbox: bool,
    /// Whether the user connected the thread to Remote Control or ended it; `None` follows their Claude settings.
    pub remote: Option<bool>,
    pub mode: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub bookmarked: bool,
    pub resume: Option<Resume>,
    /// The checkpoint taken before each user message, by the CLI's id for the message.
    pub checkpoints: HashMap<String, Checkpoint>,
    /// The checkpoint of the turn that runs.
    pub turn: Option<Checkpoint>,
    pub status: Status,
    /// A checkpoint is being taken before a prompt goes out.
    pub sending: bool,
    /// When the last message went out, which the stop button waits a moment after.
    pub sent_at: Option<std::time::Instant>,
    pub offered: Offered,
    /// The rewind the session started with, until the agent accepted it.
    pub fork: Option<Fork>,
    /// The prompt sent last, which goes out again when the agent refuses a rewind.
    pub last_sent: Option<UserMessage>,
    /// The editor's selection, offered as context while the composer has the focus.
    pub offer: Option<Attachment>,
    /// The session runs with the model and mode the user picked last for its agent, where it offers them.
    pub choice_applied: bool,
}

/// How the sandbox button looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxLook {
    pub symbol: &'static str,
    /// Green while the sandbox is on, grey while it is off.
    pub on: bool,
    pub tip: &'static str,
}

impl State {
    /// A tab's state before its session starts, with the user's last picks for its agent.
    pub fn new(source: Source, thread: Rc<Thread>, choice: ide_core::settings::AgentChoice) -> State {
        State {
            source,
            thread,
            caps: Capabilities::default(),
            acp: None,
            own: None,
            problem: None,
            custom_title: None,
            queue: Vec::new(),
            attachments: Vec::new(),
            sandbox: false,
            remote: None,
            mode: choice.mode.unwrap_or_else(|| "default".into()),
            model: choice.model,
            effort: choice.effort,
            bookmarked: false,
            resume: None,
            checkpoints: HashMap::new(),
            turn: None,
            status: Status::Idle,
            sending: false,
            sent_at: None,
            offered: Offered::default(),
            fork: None,
            last_sent: None,
            offer: None,
            choice_applied: false,
        }
    }

    /// The session's title: the user's, else the agent's, else the first line of the first message, else `untitled`.
    pub fn title(&self, untitled: &str) -> String {
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
    pub fn next_queued(&mut self) -> Option<UserMessage> {
        if self.status != Status::Idle || self.sending || !self.source.is_live() || self.queue.is_empty() {
            return None;
        }
        Some(self.queue.remove(0))
    }

    /// Notes that `message` goes out now that its checkpoint is taken.
    pub fn delivering(&mut self, message: &UserMessage) {
        self.sending = false;
        // A refusal comes with the result of the first prompt on the rewound session, so by the second the rewind
        // holds.
        if self.fork.as_ref().is_some_and(|f| f.prompted) {
            self.fork = None;
        } else if let Some(fork) = self.fork.as_mut() {
            fork.prompted = true;
        }
        self.last_sent = Some(message.clone());
    }

    /// Notes that the message went out with `checkpoint` taken before it; `id` is the message's id, the agent's or
    /// the app's own.
    pub fn sent(&mut self, id: Option<String>, checkpoint: Option<Checkpoint>) {
        if let Some(checkpoint) = checkpoint {
            if let Some(id) = id {
                self.checkpoints.insert(id, checkpoint.clone());
            }
            self.turn = Some(checkpoint);
        }
    }

    pub fn remove_queued(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);
        }
    }

    pub fn remove_attachment(&mut self, index: usize) {
        if index < self.attachments.len() {
            self.attachments.remove(index);
        }
    }

    /// Whether a turn runs or a prompt is on its way, so the send button stops instead.
    pub fn running(&self) -> bool {
        self.status != Status::Idle || self.sending
    }

    /// Whether there is something to send with `text` in the composer.
    pub fn can_send(&self, text: &str) -> bool {
        !text.trim().is_empty() || !self.attachments.is_empty()
    }

    /// What the empty composer says.
    pub fn placeholder(&self) -> &'static str {
        if self.sending {
            t("Sicherungspunkt wird angelegt …")
        } else {
            t("⌘Esc zum Fokussieren oder Verlassen")
        }
    }

    /// The model pill of a Claude session: the model as people say it, with the effort if one is set.
    pub fn model_pill(&self) -> String {
        let model_id = self.thread.model().map(str::to_string).or_else(|| self.model.clone());
        let model = model_id
            .as_deref()
            .map(|id| {
                self.caps
                    .models
                    .iter()
                    .find(|m| m.value == id)
                    .map_or_else(|| model_name(id), |m| m.display_name.clone())
            })
            .unwrap_or_else(|| t("Standardmodell").to_string());
        let effort = self.thread.effort().map(str::to_string).or_else(|| self.effort.clone());
        match effort {
            Some(effort) => tf("{} · {}", &[&model, &effort_name(&effort)]),
            None => model,
        }
    }

    /// The mode pill of a Claude session: its title and symbol.
    pub fn mode_pill(&self) -> (String, &'static str) {
        let mode_id = self.thread.mode().map_or_else(|| self.mode.clone(), str::to_string);
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

    pub fn sandbox_look(&self) -> SandboxLook {
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
pub fn modes() -> [(&'static str, &'static str, &'static str); 5] {
    [
        ("default", t("Manuell"), "hand.raised"),
        ("acceptEdits", t("Änderungen annehmen"), "pencil.and.outline"),
        ("plan", t("Plan"), "list.bullet.clipboard"),
        ("auto", t("Auto"), "wand.and.stars"),
        ("bypassPermissions", t("Umgehen"), "exclamationmark.shield"),
    ]
}

/// The name of an effort level as the composer shows it.
pub fn effort_name(level: &str) -> String {
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
pub fn model_name(id: &str) -> String {
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
pub fn find_entry(thread: &Thread, id: u64) -> Option<Arc<Entry>> {
    let lanes = std::iter::once(Lane::Main).chain(thread.subagents().iter().map(|s| Lane::Subagent(s.key.clone())));
    for lane in lanes {
        if let Some(entry) = thread.entries(&lane).iter().find(|e| e.id.get() == id) {
            return Some(entry.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ide_thread::ThreadId;

    fn state() -> State {
        let thread = Thread::new(ThreadId::new(), "claude", std::path::Path::new("/p"));
        State::new(
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
        state.sending = true;
        assert!(state.running());
        state.delivering(&message("hi"));
        assert!(!state.running());
        assert_eq!(state.last_sent.as_ref().map(|m| m.text.as_str()), Some("hi"));
        state.sent(Some("m1".into()), None);
        assert!(state.checkpoints.is_empty() && state.turn.is_none());
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
        state.model = Some("claude-opus-5-5".into());
        state.effort = Some("high".into());
        assert_eq!(state.model_pill(), tf("{} · {}", &["Opus 5.5", t("Hoch")]));
        state.mode = "manual".into();
        assert_eq!(state.mode_pill(), (t("Manuell").to_string(), "hand.raised"));
        state.mode = "plan".into();
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
