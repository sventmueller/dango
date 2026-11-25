//! Plan+: Claude plans in its own plan mode while the tab allows every permission request itself, except destructive
//! git commands (E27); the plan still goes to the user, and once they approve it the session goes on in
//! `bypassPermissions`.

use std::collections::HashSet;

use ide_claude::PermissionAnswer;
use ide_thread::{EntryKind, PermissionRequest, Status, Thread};

use super::tab::AgentTab;

/// The tab's name for Plan+, which the CLI does not know.
pub(crate) const PLAN_PLUS: &str = "planPlus";
/// The CLI's plan mode.
const PLAN: &str = "plan";
/// The mode the session goes on in after the user approved a plan made in Plan+.
const AFTER_PLAN: &str = "bypassPermissions";

/// The mode the CLI runs in for the mode `picked` in the composer.
pub(crate) fn wire(picked: &str) -> &str {
    if picked == PLAN_PLUS { PLAN } else { picked }
}

/// Whether the tab runs Plan+ now: picked, and the CLI still plans.
pub(crate) fn active(picked: &str, thread: &Thread) -> bool {
    picked == PLAN_PLUS && thread.mode().is_none_or(|mode| mode == PLAN)
}

/// Whether Plan+ allows `request` without asking.
fn allows(request: &PermissionRequest) -> bool {
    !request.default_to_no && super::cards::request::destructive(request).is_none()
}

/// What Plan+ keeps between updates of the thread.
#[derive(Default)]
pub(crate) struct PlanPlus {
    /// The requests allowed already, which the thread may still show as waiting until the session's answer is in.
    allowed: HashSet<String>,
    /// The user approved a plan, and the session is not in `bypassPermissions` yet.
    leaving: bool,
}

impl AgentTab {
    /// Allows the requests Plan+ allows, and moves the session on after an approved plan.
    pub(super) fn plan_plus_moved(&self) {
        if self.is_driven() {
            return;
        }
        let (picked, thread) = {
            let state = self.state.borrow();
            (state.picks().mode().to_string(), state.thread().clone())
        };
        if self.plan_plus.borrow().leaving {
            self.leave_plan(&thread);
        }
        if thread.status() != Status::RequiresAction || !active(&picked, &thread) {
            return;
        }
        for entry in thread.pending_requests() {
            let EntryKind::PermissionRequest(request) = &entry.kind else {
                continue;
            };
            if allows(request) && self.plan_plus.borrow_mut().allowed.insert(request.request_id.clone()) {
                let answer = PermissionAnswer::Allow {
                    updated_input: None,
                    always: Vec::new(),
                };
                self.settle(entry.id.get(), &request.request_id, answer);
            }
        }
    }

    /// The user approved a plan: Plan+ ends and the session goes on in `bypassPermissions`.
    pub(super) fn plan_approved(&self) {
        let picked_plus = self.state.borrow().picks().mode() == PLAN_PLUS;
        if !picked_plus || self.is_driven() {
            return;
        }
        self.plan_plus.replace(PlanPlus {
            leaving: true,
            ..PlanPlus::default()
        });
        self.state.borrow_mut().pick_mode(AFTER_PLAN.to_string());
        self.switch_mode(AFTER_PLAN);
        self.refresh_composer();
    }

    /// Asks for `bypassPermissions` again when the CLI left plan mode for another mode after the approval; done once
    /// the session runs in it.
    fn leave_plan(&self, thread: &Thread) {
        match thread.mode() {
            Some(AFTER_PLAN) => self.plan_plus.borrow_mut().leaving = false,
            Some(PLAN) | None => {}
            Some(_) => {
                self.plan_plus.borrow_mut().leaving = false;
                self.switch_mode(AFTER_PLAN);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ide_thread::{RequestState, ThreadId, Update};

    fn request(command: &str) -> PermissionRequest {
        PermissionRequest {
            request_id: "r".into(),
            tool_call: None,
            tool_name: "Bash".into(),
            title: None,
            description: None,
            reason: None,
            blocked_path: None,
            input: serde_json::json!({ "command": command }),
            options: Vec::new(),
            default_to_no: false,
            state: RequestState::Pending,
        }
    }

    #[test]
    fn plan_plus_runs_the_cli_in_plan_mode_and_allows_all_but_destructive_git() {
        assert_eq!(wire(PLAN_PLUS), "plan");
        assert_eq!(wire("acceptEdits"), "acceptEdits");
        let mut thread = Thread::new(ThreadId::new(), "claude", "/tmp");
        assert!(active(PLAN_PLUS, &thread));
        assert!(!active("plan", &thread));
        thread.apply(Update::Mode("default".into()));
        assert!(!active(PLAN_PLUS, &thread), "the CLI left plan mode");
        assert!(allows(&request("cargo test")));
        assert!(!allows(&request("git push --force")));
        assert!(!allows(&PermissionRequest {
            default_to_no: true,
            ..request("ls")
        }));
    }
}
