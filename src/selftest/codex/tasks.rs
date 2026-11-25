//! The real head buttons and work popovers, backed by the Codex stand-in's child thread and paginated terminals.

use ide_thread::{Lane, TaskStatus};
use ide_ui_agent::AgentKey;

use super::{NAME, check, finish, give_up, helpers, log, soon, tab_of};
use crate::activity::{self, Part};

pub(super) fn run(key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send("tasks");
    soon(
        "Codex's child and background terminals reach the UI",
        move || {
            tab_of(key).is_some_and(|tab| {
                let thread = tab.snapshot();
                thread.subagents().len() == 1 && thread.background().len() == 3
            })
        },
        move || lists(key),
    );
}

fn lists(key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    let Some(controller) = super::controller_of(&helpers::root(super::PROJECT)) else {
        return give_up(NAME, "the window is open");
    };
    for part in [Part::Subagents, Part::Background] {
        let (tip, enabled, accent) = tab.work_button(part);
        check(
            "Codex's work button counts live tasks and can be opened",
            enabled && accent,
            tip,
        );
        tab.press_work(part);
        let lines = activity::overview_lines(&controller);
        let shown = match part {
            Part::Subagents => lines.iter().any(|l| l.contains("Review the files")),
            _ => lines.iter().filter(|l| l.contains("sleep 30")).count() == 3,
        };
        check(
            "Codex's work popover shows the tasks of its button",
            shown,
            lines.join(" | "),
        );
        if let Some(view) = activity::popover_view(&controller) {
            super::super::agent::view_picture(
                &view,
                if part == Part::Subagents {
                    "codex-subagents"
                } else {
                    "codex-background"
                },
            );
        }
        activity::close(&controller);
    }
    let thread = tab.snapshot();
    let sub = &thread.subagents()[0];
    check(
        "Codex's subagent has its own streamed conversation",
        thread
            .entries(&Lane::Subagent(sub.key.clone()))
            .iter()
            .any(|e| matches!(&e.kind, ide_thread::EntryKind::AgentMessage(m) if m.markdown == "Reviewing")),
        "",
    );
    if let Some(task) = &sub.task_id {
        tab.stop_task(task);
    }
    for task in thread.background() {
        tab.stop_task(&task.id);
    }
    soon(
        "the UI stops each Codex task independently",
        move || {
            tab_of(key).is_some_and(|tab| {
                let thread = tab.snapshot();
                thread.subagents().iter().all(|s| s.status == TaskStatus::Stopped)
                    && thread.background().iter().all(|b| b.status == TaskStatus::Stopped)
            })
        },
        move || {
            check(
                "the session can stop Codex tasks without interrupting the parent",
                tab_of(key).is_some_and(|t| t.snapshot().status() == ide_thread::Status::Running),
                "",
            );
            let sent = std::fs::read_to_string(log()).unwrap_or_default();
            check(
                "Codex task stops name the child thread and the individual terminals",
                sent.contains("thread/backgroundTerminals/terminate") && sent.contains("thr_child"),
                "",
            );
            if let Some(tab) = tab_of(key) {
                tab.send_or_stop();
            }
            helpers::after(super::SCROLL_SETTLE, move || finish(NAME));
        },
    );
}
