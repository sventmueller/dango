//! What the user answers in an AI tab's cards: permissions and their rules, questions, plans, and the background tasks a turn started.

use ide_ui_common::window::{Files, Notices, Window};
use std::path::{Path, PathBuf};

use ide_claude::{PermissionAnswer, PlanAnswer};
use ide_thread::{Answer, BackgroundItem, EntryKind, PermissionOption, RequestState, RuleScope, Thread, ToolView};
use ide_ui_common::alert::Alert;
use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSMenu, NSView};
use splat_i18n::{t, tf};

use super::actions::pop_up;
use super::cards::pack;
use super::cards::request::{ALLOW_ALWAYS, ALLOW_ONCE, DENY, PLAN_APPROVE, PLAN_REJECT, edited_input, scope_name};
use super::tab::AgentTab;
use crate::AgentKey;
use ide_ui_common::util::main_soon;

/// What the model is told when the user refuses a tool; for the model, so not translated.
const DENIED: &str = "The user denied this tool call.";

/// The rules a request offers to store, by suggestion: the rule and where it would be stored.
pub(super) fn always_rules(options: &[PermissionOption]) -> Vec<(usize, String, RuleScope)> {
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
    pub(super) fn rule_items(&self, id: ide_thread::EntryId, rules: &[(usize, String, RuleScope)]) -> Retained<NSMenu> {
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
    pub(crate) fn allow_rule(&self, id: u64, suggestion: usize) {
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
        crate::host::host().remember_claude_rules(self.cwd());
        self.settle(id, &request.request_id, answer);
    }

    fn settle(&self, id: u64, request: &str, answer: PermissionAnswer) {
        let result = self.state.borrow().source().answer(request, answer);
        if let Err(error) = result {
            ide_ui_common::diag::report("answering a permission request", &error);
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
            Alert::new(self.mtm, t("Die geänderte Eingabe ist kein gültiges JSON."))
                .informative(t("Korrigiere sie oder lehne die Anfrage ab."))
                .run_modal();
        }
        input
    }

    /// Option `option` of question `question` of request `id`; sent once every question has an answer.
    pub(crate) fn answer(&self, id: u64, question: u8, option: u8) {
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
    pub(crate) fn answer_other(&self, id: u64, question: u8) {
        let text = self.typed(id, question);
        if text.is_empty() {
            objc2_app_kit::NSBeep();
            return;
        }
        self.timeline()
            .inputs()
            .borrow_mut()
            .chosen
            .insert((id, question), text);
        if !self.send_answers(id) {
            objc2_app_kit::NSBeep();
        }
    }

    /// The checked options and the typed words of question `question`, joined as the official extension does.
    fn typed(&self, id: u64, question: u8) -> String {
        let inputs = self.timeline().inputs().borrow();
        let mut parts = inputs
            .checks
            .get(&(id, question))
            .map(|b| super::cards::request::checked(b))
            .unwrap_or_default();
        parts.extend(inputs.field_text(id, question));
        parts.join(", ")
    }

    /// Sends the answers of request `id` once every question has one; false while one is still missing.
    fn send_answers(&self, id: u64) -> bool {
        let Some(entry) = self.entry(id) else { return false };
        let EntryKind::Question(asked) = &entry.kind else {
            return false;
        };
        let Some(request) = asked.request_id.clone() else {
            return false;
        };
        for question in (0..asked.items.len()).filter_map(|n| u8::try_from(n).ok()) {
            if self.timeline().inputs().borrow().chosen.contains_key(&(id, question)) {
                continue;
            }
            let text = self.typed(id, question);
            if !text.is_empty() {
                self.timeline()
                    .inputs()
                    .borrow_mut()
                    .chosen
                    .insert((id, question), text);
            }
        }
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
            return false;
        };
        let result = self.state.borrow().source().answer_questions(&request, answers);
        if let Err(error) = result {
            ide_ui_common::diag::report("answering a question", &error);
        }
        self.timeline().inputs().borrow_mut().forget(id);
        true
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
        let result = self.state.borrow_mut().answer_plan(&request, answer);
        match result {
            Ok(changes) if !changes.is_empty() => {
                let snapshot = self.state.borrow().source().snapshot();
                if let Some(snapshot) = snapshot {
                    self.absorb(snapshot, &changes);
                }
            }
            Ok(_) => {}
            Err(error) => ide_ui_common::diag::report("answering a plan", &error),
        }
        self.timeline().inputs().borrow_mut().forget(id);
    }

    /// Stops the subagent the card `id` shows.
    pub(crate) fn stop_task_of(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        let EntryKind::ToolCall(call) = &entry.kind else { return };
        let task = self
            .state
            .borrow()
            .thread()
            .subagent(&call.call_id)
            .and_then(|s| s.task_id.clone());
        if let Some(task) = task {
            self.stop_task(&task);
        }
    }

    /// Stops a subagent or background task by its task id.
    pub fn stop_task(&self, task: &str) {
        self.state.borrow().source().stop_task(task);
    }

    pub(crate) fn move_to_background(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        if let EntryKind::ToolCall(call) = &entry.kind {
            self.state.borrow().source().background(&call.call_id);
        }
    }

    pub(crate) fn show_output_of(&self, id: u64) {
        let Some(entry) = self.entry(id) else { return };
        if let EntryKind::ToolCall(call) = &entry.kind
            && let Some(task) = &call.background_task
        {
            self.show_background_output(task);
        }
    }

    /// Shows the whole output of background task `task`: the file the CLI writes it to, else what it hands over, else
    /// what the session streamed of it. Where that is empty, the file the command sends its output to itself, as in
    /// `make > build.log`; a notice where there is none yet.
    pub fn show_background_output(&self, task: &str) {
        let Some(controller) = self.controller() else { return };
        let (path, streamed, about) = {
            let state = self.state.borrow();
            let thread = state.thread();
            let item = thread.background().iter().find(|b| b.id == task);
            (
                item.and_then(|b| b.output.path.clone()).map(PathBuf::from),
                item.and_then(|b| b.output.text.clone()).unwrap_or_default(),
                Silent {
                    label: item.map_or_else(|| task.to_string(), |b| b.label.clone()),
                    ended: item.is_some_and(|b| b.status.is_final()),
                    logs: item
                        .and_then(|b| command_line(thread, b))
                        .map(|command| ide_thread::shell::output_files(&command, self.cwd()))
                        .unwrap_or_default(),
                },
            )
        };
        let key = self.key;
        let task = task.to_string();
        if let Some(path) = path.filter(|p| p.is_file()) {
            if std::fs::metadata(&path).is_ok_and(|m| m.len() > 0) {
                show_file(&controller, &path);
            } else {
                snapshot(key, task, String::new(), about);
            }
            return;
        }
        let Some(session) = self.state.borrow().source().session() else {
            return snapshot(key, task, streamed, about);
        };
        // The answer comes on the session's reader thread, which must not wait for the disk.
        session.task_output(&task).then(move |output| {
            let output = output.map(|o| o.output).unwrap_or(streamed);
            snapshot(key, task, output, about);
        });
    }
}

/// The command line of background task `item`, from the tool call that started it.
fn command_line(thread: &Thread, item: &BackgroundItem) -> Option<String> {
    match &thread.entry_by_key(item.tool_call.as_deref()?)?.kind {
        EntryKind::ToolCall(call) => match &call.view {
            ToolView::Command { command, .. } => Some(command.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// A background task whose output came empty: for the notice that it has written nothing, or the file its command
/// writes to instead.
struct Silent {
    label: String,
    ended: bool,
    /// The files the task's command sends its output to, in the order it does.
    logs: Vec<PathBuf>,
}

impl Silent {
    /// The last of the files the command sends its output to that is there; it reads the disk.
    fn log(&self) -> Option<PathBuf> {
        self.logs.iter().rev().find(|path| path.is_file()).cloned()
    }

    fn notice(&self) -> String {
        if self.ended {
            tf("„{}“ hat nichts ausgegeben.", &[&self.label])
        } else {
            tf("„{}“ hat noch nichts ausgegeben.", &[&self.label])
        }
    }
}

/// Opens `path`, whose editor, when it is open already, takes the file again: a file outside the project's folders
/// changes unwatched.
fn show_file(controller: &Window, path: &Path) {
    controller.open_file(path);
    crate::host::host().reread_file(path);
}

/// Writes `output` of background task `task` to a file of the app's and shows it in the window of AI tab `key`; when
/// it is empty, shows the file the command writes to instead or tells that there is no output yet. Any thread may call
/// it.
fn snapshot(key: AgentKey, task: String, output: String, about: Silent) {
    let _ = ide_core::lane::DISK.try_run(move || {
        // The agent names the task, and a name with a folder in it must not lead out of the app's folder.
        let Some(name) = Path::new(&task).file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return;
        };
        let file = ide_core::paths::data_dir()
            .join("task-output")
            .join(format!("{name}.txt"));
        let written = if output.is_empty() {
            about.log()
        } else {
            let made = file.parent().map(std::fs::create_dir_all).transpose();
            match made.and_then(|_| std::fs::write(&file, output)) {
                Ok(()) => Some(file),
                Err(_) => return,
            }
        };
        main_soon(move || {
            let Some(controller) = super::get(key).and_then(|t| t.controller()) else {
                return;
            };
            match written {
                Some(file) => show_file(&controller, &file),
                None => controller.show_notice(&about.notice()),
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_options_that_store_a_rule_are_offered_as_rules() {
        let options = [
            PermissionOption::AllowOnce,
            PermissionOption::AllowAlways {
                suggestion: 2,
                rule: "Bash(cargo test:*)".into(),
                scope: RuleScope::Project,
            },
            PermissionOption::Deny,
            PermissionOption::AllowAlways {
                suggestion: 0,
                rule: "Read".into(),
                scope: RuleScope::Session,
            },
        ];
        assert_eq!(
            always_rules(&options),
            [
                (2, "Bash(cargo test:*)".to_string(), RuleScope::Project),
                (0, "Read".to_string(), RuleScope::Session),
            ]
        );
        assert!(always_rules(&[PermissionOption::AllowOnce, PermissionOption::Deny]).is_empty());
    }

    #[test]
    fn a_background_task_reads_its_command_from_the_call_that_started_it_also_in_a_subagent() {
        let mut thread = Thread::new(ide_thread::ThreadId::from("t"), "test", "/tmp");
        let mut call = ide_thread::ToolCall::new("toolu_bg", "Bash", ide_thread::ToolKind::Execute);
        call.view = ToolView::Command {
            command: "./batch.sh > batch.log 2>&1".into(),
            description: None,
            background: true,
            timeout: None,
        };
        thread.apply(ide_thread::Update::Put {
            lane: ide_thread::Lane::Subagent("toolu_agent".into()),
            key: Some("toolu_bg".into()),
            at: std::time::UNIX_EPOCH,
            entry: EntryKind::ToolCall(call),
        });
        let item = |tool_call: Option<&str>| BackgroundItem {
            id: "b1".into(),
            kind: ide_thread::BackgroundKind::Shell,
            label: "Batch".into(),
            status: ide_thread::TaskStatus::Running,
            started: None,
            ended: None,
            tool_call: tool_call.map(str::to_string),
            parent: None,
            output: ide_thread::OutputTail::default(),
            is_backgrounded: true,
            ambient: false,
        };
        assert_eq!(
            command_line(&thread, &item(Some("toolu_bg"))).as_deref(),
            Some("./batch.sh > batch.log 2>&1")
        );
        assert_eq!(command_line(&thread, &item(Some("toolu_other"))), None);
        assert_eq!(command_line(&thread, &item(None)), None);
    }

    #[test]
    fn an_empty_output_falls_back_to_the_last_file_the_command_wrote_that_is_there() {
        let dir = std::env::temp_dir().join(format!("ide-silent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("batch.log");
        std::fs::write(&log, "tick\n").unwrap();
        let about = Silent {
            label: "Batch".into(),
            ended: false,
            logs: vec![log.clone(), dir.join("missing.log")],
        };
        assert_eq!(about.log(), Some(log));
        let none = Silent {
            logs: vec![dir.join("missing.log")],
            ..about
        };
        assert_eq!(none.log(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
