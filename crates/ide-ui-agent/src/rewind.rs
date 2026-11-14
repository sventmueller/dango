//! Rewinding the conversation to just before a prompt (I6): the agent resumes the session as a new branch that ends
//! there, guarded so that it refuses rather than drops more than that prompt's turn.

use std::rc::Rc;
use std::time::SystemTime;

use ide_thread::{
    Change, Entry, EntryId, EntryKind, IndexEntry, Lane, Notice, Subagent, SubagentPatch, TaskStatus, Thread, Update,
};
use splat_i18n::tf;

use super::source::Source;
use super::tab::AgentTab;

/// What the thread was before a rewind, to go back to when the agent refuses it.
pub struct Fork {
    /// The session the rewind branches off.
    pub session: String,
    /// Where it branches, when the prompt was not the first; a first prompt starts a new session instead.
    pub rewind: Option<ide_thread::Rewind>,
    /// The whole thread before the rewind.
    pub before: Thread,
    /// Whether a prompt went out on the rewound session; once a second one does, the agent has accepted the rewind.
    pub prompted: bool,
    /// The history entry that keeps the session as it was before the rewind.
    pub kept_entry: Option<ide_thread::ThreadId>,
}

/// `thread` up to, not including, the main-lane entry `prompt`, with the agent's ids of each kept entry, so that a
/// later rewind to an earlier prompt still finds them. Subagents keep their own timelines; those that still ran
/// stop with the process the rewind ends.
pub fn kept_before(thread: &Thread, prompt: EntryId) -> Thread {
    let main = thread.entries(&Lane::Main);
    kept_until(thread, main.iter().position(|e| e.id == prompt).unwrap_or(main.len()))
}

/// `thread` with the first `cut` entries of its main lane.
fn kept_until(thread: &Thread, cut: usize) -> Thread {
    let mut kept = Thread::new(thread.id().clone(), thread.agent(), thread.cwd());
    if !thread.title().is_empty() {
        kept.apply(Update::Title(thread.title().to_string()));
    }
    let main = thread.entries(&Lane::Main);
    let mut updates = Vec::new();
    for (n, entry) in main[..cut].iter().enumerate() {
        let key = format!("kept:{n}");
        updates.push(Update::Put {
            lane: Lane::Main,
            key: Some(key.clone()),
            at: entry.at,
            entry: entry.kind.clone(),
        });
        if let Some(uuid) = &entry.uuid {
            updates.push(Update::Chain {
                key,
                uuid: uuid.clone(),
                follows: entry.follows.clone(),
            });
        }
        if let EntryKind::ToolCall(call) = &entry.kind
            && let Some(sub) = thread.subagent(&call.call_id)
        {
            updates.push(Update::Subagent(patch_of(sub)));
            let lane = Lane::Subagent(sub.key.clone());
            for (m, inner) in thread.entries(&lane).iter().enumerate() {
                updates.push(Update::Put {
                    lane: lane.clone(),
                    key: Some(format!("kept:{n}:{m}")),
                    at: inner.at,
                    entry: inner.kind.clone(),
                });
            }
        }
    }
    kept.apply_all(updates);
    kept
}

impl AgentTab {
    /// Whether the conversation can go back to just before the prompt `entry`: a later prompt of a session the agent
    /// knows, or the first prompt, before which a new session starts.
    pub(super) fn conversation_rewindable(&self, entry: &Entry) -> bool {
        let state = self.state.borrow();
        let thread = &state.thread;
        let first = first_prompt(thread) == Some(entry.id);
        let known = thread.session().is_some() || state.resume.is_some();
        first || (known && thread.rewind_before(entry.id).is_some())
    }

    /// Ends the session and starts it again as a branch that stops just before the prompt `id`, whose text goes back
    /// into the composer to be sent again or changed.
    pub(super) fn rewind_conversation(&self, id: u64) {
        let thread = self.snapshot();
        let Some(entry) = thread.entries(&Lane::Main).iter().find(|e| e.id.get() == id).cloned() else {
            return;
        };
        let EntryKind::UserMessage(message) = &entry.kind else {
            return;
        };
        if !self.conversation_rewindable(&entry) {
            return;
        }
        let session = thread
            .session()
            .map(str::to_string)
            .or_else(|| self.state.borrow().resume.as_ref().map(|r| r.session.clone()))
            .unwrap_or_default();
        let rewind = thread.rewind_before(entry.id);
        let kept = kept_before(&thread, entry.id);
        let kept_entry = self.keep_in_history(&session);
        let replaying = {
            let mut state = self.state.borrow_mut();
            let old = std::mem::replace(&mut state.source, Source::Idle);
            let replaying = old.is_replay();
            old.end();
            state.rewound(
                kept.clone(),
                &message.attachments,
                Fork {
                    session,
                    rewind,
                    before: thread.clone(),
                    prompted: false,
                    kept_entry,
                },
            );
            replaying
        };
        self.timeline().apply(Rc::new(kept), &[Change::Cleared]);
        self.composer.set_text(&message.text);
        self.refresh_chips();
        self.refresh();
        if !replaying {
            self.start();
        }
        self.focus_composer();
    }

    /// The agent refused the rewind: the whole conversation comes back, the session resumes without the rewind, and
    /// the prompt that met the refusal goes out again. The same rewind would fail again, so it is not retried.
    pub(super) fn rewind_refused(&self, why: &str) {
        let (fork, message) = { self.state.borrow_mut().take_refused_rewind() };
        let Some(fork) = fork else { return };
        if let Some(id) = &fork.kept_entry {
            crate::agent_panel::remove(id);
        }
        let mut before = fork.before;
        before.apply(Update::Put {
            lane: Lane::Main,
            key: None,
            at: SystemTime::now(),
            entry: EntryKind::Notice(Notice::RewindRejected {
                detail: why.to_string(),
            }),
        });
        {
            let mut state = self.state.borrow_mut();
            let old = std::mem::replace(&mut state.source, Source::Idle);
            old.end();
            state.rewind_undone(before.clone(), message);
        }
        self.timeline().apply(Rc::new(before), &[Change::Cleared]);
        self.refresh_queue();
        self.refresh();
        self.start();
    }
}

impl AgentTab {
    /// Lists the session `session` as it stands before a rewind as a history entry of its own, which reopening
    /// resumes; the tab goes on with the branch.
    fn keep_in_history(&self, session: &str) -> Option<ide_thread::ThreadId> {
        if session.is_empty() {
            return None;
        }
        let id = ide_thread::ThreadId::new();
        let entry = IndexEntry {
            id: id.clone(),
            title: tf("{} · vor dem Zurückspulen", &[&self.title()]),
            agent: ide_claude::AGENT.to_string(),
            group: None,
            archived: false,
            last_activity: SystemTime::now(),
            status: ide_thread::Status::Idle,
            session: Some(session.to_string()),
            cwd: Some(self.cwd.clone()),
            cost_usd: self.snapshot().usage().billed_cost(),
        };
        crate::agent_panel::upsert(entry);
        Some(id)
    }
}

/// The conversation's first prompt.
fn first_prompt(thread: &Thread) -> Option<EntryId> {
    thread
        .entries(&Lane::Main)
        .iter()
        .find(|e| matches!(e.kind, EntryKind::UserMessage(_)))
        .map(|e| e.id)
}

fn patch_of(sub: &Subagent) -> SubagentPatch {
    let status = if sub.status.is_final() {
        sub.status
    } else {
        TaskStatus::Stopped
    };
    SubagentPatch {
        task_id: sub.task_id.clone(),
        agent_type: sub.agent_type.clone(),
        description: Some(sub.description.clone()),
        status: Some(status),
        ended: sub.ended,
        elapsed: Some(sub.elapsed),
        tokens: Some(sub.tokens),
        tool_uses: Some(sub.tool_uses),
        last_tool: sub.last_tool.clone(),
        summary: sub.summary.clone(),
        cancellable: Some(false),
        backgrounded: Some(sub.backgrounded),
        parent: sub.parent.clone(),
        ..SubagentPatch::new(sub.key.clone(), sub.started)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use ide_claude::replay::Replay;
    use ide_thread::ThreadId;

    use super::*;

    const QUESTION: &str = include_str!("../../ide-claude/tests/fixtures/recorded_question.ndjson");
    const SUBAGENT: &str = include_str!("../../ide-claude/tests/fixtures/recorded_subagent_background.ndjson");

    fn replayed(recording: &str) -> Thread {
        let thread = Thread::new(ThreadId::from("t"), "claude", "/tmp/x");
        let mut replay = Replay::new(thread, UNIX_EPOCH + Duration::from_secs(1), Duration::from_millis(10));
        replay.feed_all(recording);
        replay.into_thread()
    }

    fn prompts(thread: &Thread) -> Vec<EntryId> {
        thread
            .entries(&Lane::Main)
            .iter()
            .filter(|e| matches!(e.kind, EntryKind::UserMessage(_)))
            .map(|e| e.id)
            .collect()
    }

    #[test]
    fn a_rewind_keeps_the_turns_before_the_prompt_with_their_ids() {
        let thread = replayed(QUESTION);
        let second = prompts(&thread)[1];
        let kept = kept_before(&thread, second);
        let main = thread.entries(&Lane::Main);
        let cut = main.iter().position(|e| e.id == second).unwrap();
        assert_eq!(kept.entries(&Lane::Main).len(), cut);
        for (old, new) in main[..cut].iter().zip(kept.entries(&Lane::Main)) {
            assert_eq!((&old.uuid, &old.kind), (&new.uuid, &new.kind));
        }
        let rewind = thread
            .rewind_before(second)
            .expect("the second prompt can be rewound to");
        let last = kept.entries(&Lane::Main).iter().rev().find_map(|e| e.uuid.clone());
        assert_eq!(
            last.as_deref(),
            Some(rewind.fork_at.as_str()),
            "the kept turn ends where the fork starts"
        );
    }

    #[test]
    fn a_subagent_keeps_its_timeline_and_stops_with_the_process() {
        let lines: Vec<&str> = SUBAGENT.lines().take(174).collect();
        let thread = replayed(&lines.join("\n"));
        let all = kept_until(&thread, thread.entries(&Lane::Main).len());
        let sub = thread.subagents().first().expect("the recording starts a subagent");
        assert_eq!(sub.status, TaskStatus::Running);
        let kept_sub = all.subagent(&sub.key).expect("the subagent is kept");
        assert_eq!(kept_sub.status, TaskStatus::Stopped);
        assert_eq!(
            all.entries(&Lane::Subagent(sub.key.clone())).len(),
            thread.entries(&Lane::Subagent(sub.key.clone())).len()
        );
    }
}
