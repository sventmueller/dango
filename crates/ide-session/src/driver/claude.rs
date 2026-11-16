//! What a session the user drives in an AI tab needs beyond an order's (B37b): messages into the turn that runs,
//! permission answers with an edited input and rules to keep, Claude Code's own calls, and the played recordings of
//! the self-test.

use ide_thread::{EntryKind, PermissionOption, PermissionRequest, UserMessage};
use serde_json::Value;

use super::{AnswerError, Driver, Prompt, SendError};
use crate::backend::SendFailure;
use crate::launch::Contract;

/// How long a checkpoint's label of the user's message gets, in characters.
const LABEL: usize = 80;

/// The label of the checkpoint before the user's message `text`, as the AI tab names it: its first line that is not
/// blank, cut to `LABEL` characters, with an ellipsis when it is cut or more lines follow.
pub(super) fn label(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim();
    let mut out: String = line.chars().take(LABEL).collect();
    if line.chars().count() > LABEL || text.trim().lines().count() > 1 {
        out.push('…');
    }
    out
}

/// Why `always`, the suggestions of `asked` to keep as rules, may not be kept, with `edited` the input the user
/// changed: a destructive git command is never allowed for good (E27, E33), and only offered suggestions are kept.
fn refused_rules(asked: &PermissionRequest, edited: Option<&Value>, always: &[usize]) -> Option<String> {
    if always.is_empty() {
        return None;
    }
    let destructive = asked.tool_name == "Bash"
        && [Some(&asked.input), edited]
            .into_iter()
            .flatten()
            .any(|input| ide_thread::shell::destructive_input(input).is_some());
    if destructive {
        return Some("a destructive git command is allowed once at most".into());
    }
    let offered = |wanted: usize| {
        (asked.options.iter())
            .any(|o| matches!(o, PermissionOption::AllowAlways { suggestion, .. } if *suggestion == wanted))
    };
    (always.iter())
        .find(|n| !offered(**n))
        .map(|n| format!("the request offers no rule {n} to keep"))
}

impl Driver {
    /// The Claude session, for what only Claude Code offers, such as its overlays, Remote Control and its usage;
    /// `None` on the other paths.
    pub fn claude(&self) -> Option<&ide_claude::Session> {
        self.backend.claude()
    }

    /// Whether a recording plays instead of an agent.
    pub fn is_replay(&self) -> bool {
        self.backend.is_replay()
    }

    /// Whether a view may hand the session messages: it has not ended, its agent has opened it, and an agent runs it,
    /// not a recording.
    pub fn takes_prompts(&self) -> bool {
        !self.is_ended() && self.backend.is_ready() && !self.backend.is_ended() && !self.backend.is_replay()
    }

    /// Hands `message` to the turn that runs, at once and without a checkpoint, as the user's messages reach a running
    /// Claude turn from an AI tab; the agent's id of the message where the path gives one. An order's session, and a
    /// path that is busy, take it as a prompt that waits for the turn instead (E184), with no id.
    pub fn steer(&mut self, message: UserMessage) -> Result<Option<String>, SendError> {
        if self.is_ended() {
            return Err(SendError::Ended);
        }
        let queued = |driver: &mut Driver, message| {
            let prompt = Prompt {
                message,
                ..Prompt::default()
            };
            driver.send(prompt).map(|_| None)
        };
        if let Contract::Order(_) = self.contract {
            return queued(self, message);
        }
        match self.backend.send(message.clone(), None) {
            Ok(id) => Ok(id),
            Err(SendFailure::Busy) => queued(self, message),
            Err(SendFailure::Failed(why)) => Err(SendError::Refused(why)),
        }
    }

    /// Answers the permission request `request` as the user does in an AI tab, with the input they edited and the
    /// suggestions to keep as rules, which only Claude takes; keeping a rule for a destructive git command is refused
    /// (E27, E33).
    pub fn answer_permission_as(
        &mut self,
        request: &str,
        answer: ide_claude::PermissionAnswer,
    ) -> Result<(), AnswerError> {
        let Some(EntryKind::PermissionRequest(asked)) = self.pending(request) else {
            return Err(AnswerError::NoRequest(request.into()));
        };
        if let ide_claude::PermissionAnswer::Allow { updated_input, always } = &answer
            && let Some(why) = refused_rules(&asked, updated_input.as_ref(), always)
        {
            return Err(AnswerError::Path(why));
        }
        self.backend.permission_as(&asked, answer).map_err(AnswerError::Path)
    }
}

#[cfg(any(test, feature = "selftest"))]
impl Driver {
    /// Plays recorded `lines` into the recording the session plays and hands what changed to the subscribers at
    /// once, without a hop to the main thread; what changed, nothing on the other paths.
    pub fn feed(&mut self, lines: &str) -> Vec<ide_thread::Change> {
        let Some(changes) = self.backend.feed(lines) else {
            return Vec::new();
        };
        self.publish_now(&changes);
        changes
    }

    /// Answers the plan `request` of a played recording with the frame a session would send, handed to the
    /// subscribers at once; what changed, nothing on the other paths.
    pub fn replay_plan(&mut self, request: &str, reply: crate::PlanReply) -> Vec<ide_thread::Change> {
        let Some(changes) = self.backend.replay_plan(request, reply) else {
            return Vec::new();
        };
        self.publish_now(&changes);
        changes
    }

    fn publish_now(&mut self, changes: &[ide_thread::Change]) {
        let mut signals = std::mem::take(&mut self.outbox);
        self.thread = self.backend.snapshot();
        self.decide_requests(&mut signals);
        self.feeds.publish(changes, &signals);
    }
}

#[cfg(test)]
mod tests {
    use ide_thread::{RequestState, RuleScope};
    use serde_json::json;

    use super::*;

    fn asked(command: &str) -> PermissionRequest {
        PermissionRequest {
            request_id: "r1".into(),
            tool_call: None,
            tool_name: "Bash".into(),
            title: None,
            description: None,
            reason: None,
            blocked_path: None,
            input: json!({ "command": command }),
            options: vec![
                PermissionOption::AllowOnce,
                PermissionOption::AllowAlways {
                    suggestion: 1,
                    rule: "Bash(cargo test:*)".into(),
                    scope: RuleScope::Project,
                },
                PermissionOption::Deny,
            ],
            default_to_no: false,
            state: RequestState::Pending,
        }
    }

    #[test]
    fn only_offered_rules_are_kept_and_never_one_for_destructive_git() {
        assert_eq!(refused_rules(&asked("cargo test"), None, &[1]), None);
        assert!(refused_rules(&asked("cargo test"), None, &[0]).is_some());
        assert!(refused_rules(&asked("git reset --hard"), None, &[1]).is_some());
        assert_eq!(
            refused_rules(&asked("git reset --hard"), None, &[]),
            None,
            "once is allowed"
        );
        let edited = json!({ "command": "git push --force" });
        assert!(refused_rules(&asked("cargo test"), Some(&edited), &[1]).is_some());
    }

    #[test]
    fn a_label_is_the_first_line_that_says_something() {
        assert_eq!(label("  \n Fix the build \n"), "Fix the build");
        assert_eq!(label("Fix it\nand test"), "Fix it…");
        assert_eq!(label(&"x".repeat(90)).chars().count(), LABEL + 1);
    }
}
