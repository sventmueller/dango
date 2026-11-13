//! What the user answers in an AI tab's cards: permissions and their rules, questions, plans, and the background tasks a turn started.

use std::path::PathBuf;

use ide_claude::{PermissionAnswer, PlanAnswer};
use ide_thread::{Answer, EntryKind, PermissionOption, RequestState};
use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSAlert, NSMenu, NSView};
use splat_i18n::{t, tf};

use super::actions::pop_up;
use super::cards::pack;
use super::cards::request::{ALLOW_ALWAYS, ALLOW_ONCE, DENY, PLAN_APPROVE, PLAN_REJECT, edited_input, scope_name};
use super::tab::AgentTab;
use crate::util::{main_soon, ns};

/// What the model is told when the user refuses a tool; for the model, so not translated.
const DENIED: &str = "The user denied this tool call.";

/// The rules a request offers to store, by suggestion: the rule and where it would be stored.
pub(super) fn always_rules(options: &[PermissionOption]) -> Vec<(usize, String, String)> {
    options
        .iter()
        .filter_map(|o| match o {
            PermissionOption::AllowAlways {
                suggestion,
                rule,
                scope,
            } => Some((*suggestion, rule.clone(), scope.clone())),
            _ => None,
        })
        .collect()
}

impl AgentTab {
    pub fn permission(&self, id: u64, what: u8, view: Option<&NSView>) {
        let several_rules = || {
            self.entry(id).is_some_and(|e| match &e.kind {
                EntryKind::PermissionRequest(r) => {
                    r.options
                        .iter()
                        .filter(|o| matches!(o, PermissionOption::AllowAlways { .. }))
                        .count()
                        > 1
                }
                _ => false,
            })
        };
        if self.agent.is_acp() && !(what == ALLOW_ALWAYS && several_rules()) {
            return self.acp_permission(id, what, None);
        }
        if self.agent.is_own() {
            return self.own_permission(id, what);
        }
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::PermissionRequest(request) = &entry.kind else {
            return;
        };
        if request.state != RequestState::Pending {
            return;
        }
        let answer = match what {
            ALLOW_ONCE => {
                let Some(input) = self.edited(id, &request.tool_name, &request.input) else {
                    return;
                };
                PermissionAnswer::Allow {
                    updated_input: Some(input).filter(|i| *i != request.input),
                    always: Vec::new(),
                }
            }
            ALLOW_ALWAYS => {
                let rules = always_rules(&request.options);
                if rules.len() > 1 {
                    if let Some(view) = view {
                        pop_up(&self.rule_items(entry.id, &rules), view);
                    }
                    return;
                }
                let Some((suggestion, _, _)) = rules.first() else {
                    return;
                };
                return self.allow_rule(id, *suggestion);
            }
            DENY => PermissionAnswer::Deny {
                message: DENIED.to_string(),
                interrupt: false,
            },
            _ => return,
        };
        self.settle(id, &request.request_id, answer);
    }

    /// The menu of the rules request `id` could store, each with where it would be stored.
    pub(super) fn rule_items(&self, id: ide_thread::EntryId, rules: &[(usize, String, String)]) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        for (suggestion, rule, scope) in rules {
            let tag = pack(id, u8::try_from(*suggestion).unwrap_or(0));
            self.menu_item(
                &menu,
                &tf("{} · {}", &[rule, &scope_name(scope)]),
                sel!(ideAgentAllowRule:),
                tag,
            );
        }
        menu
    }

    /// Allows request `id` and stores the rule of suggestion `suggestion`.
    pub fn allow_rule(&self, id: u64, suggestion: usize) {
        if self.agent.is_acp() {
            return self.acp_permission(id, ALLOW_ALWAYS, Some(suggestion));
        }
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::PermissionRequest(request) = &entry.kind else {
            return;
        };
        let Some(input) = self.edited(id, &request.tool_name, &request.input) else {
            return;
        };
        let answer = PermissionAnswer::Allow {
            updated_input: Some(input).filter(|i| *i != request.input),
            always: vec![suggestion],
        };
        crate::settings::remember_claude_rules(self.cwd());
        self.settle(id, &request.request_id, answer);
    }

    fn settle(&self, id: u64, request: &str, answer: PermissionAnswer) {
        let result = self.state.borrow().source.answer(request, answer);
        if let Err(error) = result {
            crate::diag::report("answering a permission request", &error);
        }
        self.timeline().inputs().borrow_mut().forget(id);
    }

    /// The input as the user edited it; `None`, after telling them, when the edit is not valid.
    fn edited(&self, id: u64, tool: &str, original: &serde_json::Value) -> Option<serde_json::Value> {
        let text = self.timeline().inputs().borrow().editor_text(id);
        let Some(text) = text else {
            return Some(original.clone());
        };
        let input = edited_input(tool, original, &text);
        if input.is_none() {
            let alert = NSAlert::new(self.mtm);
            alert.setMessageText(&ns(t("Die geänderte Eingabe ist kein gültiges JSON.")));
            alert.setInformativeText(&ns(t("Korrigiere sie oder lehne die Anfrage ab.")));
            alert.runModal();
        }
        input
    }

    /// Option `option` of question `question` of request `id`; sent once every question has an answer.
    pub fn answer(&self, id: u64, question: u8, option: u8) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::Question(asked) = &entry.kind else {
            return;
        };
        let Some(label) = asked
            .items
            .get(usize::from(question))
            .and_then(|i| i.options.get(usize::from(option)))
            .map(|o| o.label.clone())
        else {
            return;
        };
        self.timeline()
            .inputs()
            .borrow_mut()
            .chosen
            .insert((id, question), label);
        self.send_answers(id);
    }

    /// The user's own words, or the checked options, for question `question` of request `id`.
    pub fn answer_other(&self, id: u64, question: u8) {
        let text = {
            let inputs = self.timeline().inputs().borrow();
            let mut parts = inputs
                .checks
                .get(&(id, question))
                .map(|b| super::cards::request::checked(b))
                .unwrap_or_default();
            parts.extend(inputs.field_text(id, question));
            parts.join(", ")
        };
        if text.is_empty() {
            return;
        }
        self.timeline()
            .inputs()
            .borrow_mut()
            .chosen
            .insert((id, question), text);
        self.send_answers(id);
    }

    fn send_answers(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::Question(asked) = &entry.kind else {
            return;
        };
        let Some(request) = asked.request_id.clone() else {
            return;
        };
        let answers: Option<Vec<Answer>> = {
            let inputs = self.timeline().inputs().borrow();
            asked
                .items
                .iter()
                .enumerate()
                .map(|(n, item)| {
                    let chosen = inputs.chosen.get(&(id, u8::try_from(n).ok()?))?;
                    Some(Answer {
                        question: item.question.clone(),
                        answer: chosen.clone(),
                    })
                })
                .collect()
        };
        let Some(answers) = answers else {
            self.timeline().reload_entry(id);
            return;
        };
        let result = self.state.borrow().source.answer_questions(&request, answers);
        if let Err(error) = result {
            crate::diag::report("answering a question", &error);
        }
        self.timeline().inputs().borrow_mut().forget(id);
    }

    /// Answers plan `id` as its button `what` says: approve, revise with the comments, or reject.
    pub fn plan(&self, id: u64, what: u8) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::PlanApproval(plan) = &entry.kind else {
            return;
        };
        let Some(request) = plan.request_id.clone() else { return };
        let answer = match what {
            PLAN_APPROVE => PlanAnswer::Approve,
            PLAN_REJECT => PlanAnswer::Reject,
            _ => PlanAnswer::Revise(self.plan_feedback(id)),
        };
        super::plan::close_comment();
        let result = self.state.borrow_mut().source.answer_plan(&request, answer);
        match result {
            Ok(changes) if !changes.is_empty() => {
                let snapshot = self.state.borrow().source.snapshot();
                if let Some(snapshot) = snapshot {
                    self.absorb(snapshot, &changes);
                }
            }
            Ok(_) => {}
            Err(error) => crate::diag::report("answering a plan", &error),
        }
        self.timeline().inputs().borrow_mut().forget(id);
    }

    /// Stops the subagent the card `id` shows.
    pub fn stop_task_of(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::ToolCall(call) = &entry.kind else { return };
        let task = self
            .state
            .borrow()
            .thread
            .subagent(&call.call_id)
            .and_then(|s| s.task_id.clone());
        if let Some(task) = task {
            self.stop_task(&task);
        }
    }

    /// Stops a subagent or background task by its task id.
    pub fn stop_task(&self, task: &str) {
        self.state.borrow().source.stop_task(task);
    }

    pub fn move_to_background(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        if let EntryKind::ToolCall(call) = &entry.kind {
            self.state.borrow().source.background(&call.call_id);
        }
    }

    pub fn show_output_of(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        if let EntryKind::ToolCall(call) = &entry.kind
            && let Some(task) = &call.background_task
        {
            self.show_background_output(task);
        }
    }

    /// Shows the whole output of background task `task`: the file the CLI writes it to, else what it hands over.
    pub fn show_background_output(&self, task: &str) {
        let Some(controller) = self.controller() else { return };
        let path = {
            let state = self.state.borrow();
            state
                .thread
                .background()
                .iter()
                .find(|b| b.id == task)
                .and_then(|b| b.output.path.clone())
        };
        if let Some(path) = path.map(PathBuf::from).filter(|p| p.is_file()) {
            controller.open_file(&path);
            return;
        }
        let Some(session) = self.state.borrow().source.session().cloned() else {
            return;
        };
        let key = self.key;
        let task = task.to_string();
        session.task_output(&task).then(move |output| {
            let Ok(output) = output else { return };
            let file = ide_core::paths::data_dir()
                .join("task-output")
                .join(format!("{task}.txt"));
            let written = file
                .parent()
                .map(std::fs::create_dir_all)
                .transpose()
                .and_then(|_| std::fs::write(&file, output.output));
            if written.is_ok() {
                main_soon(move || {
                    if let Some(controller) = super::get(key).and_then(|t| t.controller()) {
                        controller.open_file(&file);
                    }
                });
            }
        });
    }
}
