//! Parks a real supervised stand-in for Claude, then sends a prompt by resuming the same session.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Instant;

use ide_process::{Supervisor, system::physical_footprint};
use ide_ui_agent::{Agent, AgentKey};

use super::agent::tab_of;
use super::{check, finish, give_up, helpers};

const NAME: &str = "agent-sleep";

const CLI: &str = r#"#!/bin/sh
printf '%s\n' "$@" >> parking-args.txt
printf '%s\n' '{"type":"system","subtype":"init","session_id":"00000000-0000-4000-8000-000000000091","cwd":"/","model":"claude-haiku-5-5","permissionMode":"default","tools":[]}'
while IFS= read -r frame; do
    case "$frame" in
        *'"type":"user"'*)
            printf '%s\n' '{"type":"assistant","message":{"id":"resumed","content":[{"type":"text","text":"resumed reply"}]},"parent_tool_use_id":null}'
            printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"resumed reply","session_id":"00000000-0000-4000-8000-000000000091"}'
            ;;
    esac
done
"#;

pub(crate) fn run() {
    helpers::open_project(NAME, NAME, &[("editor.txt", "another tab\n")], |controller| {
        let root = helpers::root(NAME);
        let program = root.join("fake-claude");
        if std::fs::write(&program, CLI)
            .and_then(|()| std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)))
            .is_err()
        {
            return give_up(NAME, "the stand-in is executable");
        }
        ide_ui_agent::set_test_claude(program);
        let Some(tab) = ide_ui_agent::open_agent(&*controller, Agent::Claude, None) else {
            return give_up(NAME, "a Claude tab opens");
        };
        let key = tab.key;
        helpers::wait_until(
            move || tab_of(key).is_some_and(|tab| tab.is_live() && tab.session_id().is_some()),
            move |ready| {
                if !ready {
                    return give_up(NAME, "Claude initialized");
                }
                park(root, key);
            },
        );
    });
}

fn park(root: PathBuf, key: AgentKey) {
    let (Some(controller), Some(tab)) = (helpers::window_of(&root), tab_of(key)) else {
        return give_up(NAME, "the window and Claude tab are open");
    };
    let Some(process) = Supervisor::global()
        .running()
        .into_iter()
        .find(|p| p.kind == ide_process::Kind::Agent)
    else {
        return give_up(NAME, "the stand-in is supervised");
    };
    let before = physical_footprint(process.pid);
    controller.open_file(&root.join("editor.txt"));
    tab.check_parking_for_selftest();
    helpers::wait_until(
        move || {
            Supervisor::global()
                .wait(process.id, std::time::Duration::ZERO)
                .is_some()
                && tab_of(key).is_some_and(|tab| tab.is_parked_for_selftest())
        },
        move |parked| {
            check(
                "hidden idle Claude releases its process",
                parked,
                format!(
                    "agent physical footprint bytes before {before:?}, after {:?}",
                    physical_footprint(process.pid)
                ),
            );
            resume(root, key);
        },
    );
}

fn resume(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the parked tab is still open");
    };
    let started = Instant::now();
    tab.select();
    tab.set_draft("resume this session");
    tab.submit();
    helpers::wait_until(
        move || {
            tab_of(key).is_some_and(|tab| {
            tab.view_snapshot().entries(&ide_thread::Lane::Main).iter().any(|entry| {
                matches!(&entry.kind, ide_thread::EntryKind::AgentMessage(message) if message.markdown.contains("resumed reply"))
            })
        })
        },
        move |answered| {
            let resumed = std::fs::read_to_string(root.join("parking-args.txt")).is_ok_and(|args| {
                args.lines()
                    .any(|arg| arg == "--resume=00000000-0000-4000-8000-000000000091")
            });
            check(
                "the next prompt resumes the same Claude session",
                answered && resumed,
                format!("{} ms to a reply from the stand-in", started.elapsed().as_millis()),
            );
            finish(NAME);
        },
    );
}
