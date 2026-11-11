//! The `acp` scenario: AI tabs with agents over ACP, played by the test agent of `ide_acp`, which is this program
//! started with `IDE_ACP_TEST_AGENT`. No agent installed on the Mac ever starts: every launch is the test agent or
//! nothing, and the registry is a fixture.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ide_thread::{EntryKind, Lane, Notice, Status, ToolOutput, ToolStatus};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSAppearanceNameAqua, NSAppearanceNameDarkAqua};

use splat_i18n::t;

use super::agent::{appearance, controller_of, picture, tab_of, view_picture};
use super::{check, finish};
use crate::util::{constant, main_after};
use crate::window::Panel;

const NAME: &str = "acp";
const REGISTRY: &str = include_str!("../../crates/ide-acp/tests/fixtures/registry.json");

/// The places of the agents in the "+" menu.
const CODEX: usize = 0;
const GEMINI: usize = 1;
const OPENCODE: usize = 3;

fn test_launch(scenario: &str, root: &PathBuf) -> Option<ide_acp::Launch> {
    let program = std::env::current_exe().ok()?;
    Some(ide_acp::Launch::new(
        "test",
        "Test",
        program,
        Vec::new(),
        vec![
            ("IDE_ACP_TEST_AGENT".into(), scenario.into()),
            ("PATH".into(), "/usr/bin:/bin".into()),
        ],
        root,
    ))
}

/// Calls `then` once `ready` holds, checking every 100 ms; after `limit` it fails `what` and ends the scenario.
fn wait(
    what: &'static str,
    started: Instant,
    limit: Duration,
    ready: impl Fn() -> bool + Send + 'static,
    then: impl FnOnce() + Send + 'static,
) {
    if ready() {
        return then();
    }
    if started.elapsed() > limit {
        check(what, false, "waited in vain");
        return finish(NAME);
    }
    main_after(Duration::from_millis(100), move || {
        wait(what, started, limit, ready, then)
    });
}

fn soon(what: &'static str, ready: impl Fn() -> bool + Send + 'static, then: impl FnOnce() + Send + 'static) {
    wait(what, Instant::now(), Duration::from_secs(15), ready, then);
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = ide_core::paths::home().join("acp-project");
    let _ = std::fs::create_dir_all(&root);
    let _ = std::fs::write(root.join("README.md"), "# ACP project\n");
    let root = root.canonicalize().unwrap_or(root);
    crate::project::trust(&root);
    if let Ok(register) = ide_acp::Register::parse(REGISTRY) {
        crate::agent::set_test_register(register);
    }
    crate::agent::set_test_launch(test_launch("basic", &root));
    crate::app::open_project(mtm, root.clone());
    main_after(Duration::from_millis(500), move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        let names: Vec<String> = crate::agent::preset_menu().into_iter().map(|(n, _)| n).collect();
        check(
            "the new tab menu offers the ACP agents",
            names == ["Codex", "Gemini", "Antigravity", "OpenCode"],
            names.join(", "),
        );
        let Some(tab) = crate::agent::open_preset(&controller, GEMINI) else {
            return finish(NAME);
        };
        let key = tab.key;
        soon(
            "the ACP session opens",
            move || {
                tab_of(key).is_some_and(|t| t.is_live() && t.acp_capabilities().is_some_and(|c| !c.modes.is_empty()))
            },
            move || ready(root, key),
        );
    });
}

/// The session is open: what the agent offers shows in the composer.
fn ready(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    check(
        "the tab runs the agent it was opened with",
        tab.agent_name() == "Gemini",
        tab.agent_name(),
    );
    let (texts, other) = tab.empty_state();
    check(
        "the empty tab names its own agent, without an image and without Claude's /model tip",
        texts == ["Gemini"] && !other,
        format!("{texts:?}, more {other}"),
    );
    let caps = tab.acp_capabilities().unwrap_or_default();
    check(
        "the agent's modes and model reach the tab",
        caps.modes.len() == 2 && caps.model().is_some_and(|m| m.current_value() == "fast"),
        format!("{:?} / {:?}", caps.modes, caps.model()),
    );
    let (model, mode, sandbox) = tab.pills();
    check(
        "the pills show the agent's model and mode",
        model == "Fast" && mode == "Ask",
        format!("{model} / {mode}"),
    );
    check(
        "the sandbox switch says what it would limit",
        sandbox.contains("Schreiben nur im Projekt"),
        sandbox,
    );
    picture(&controller, "acp-ready");
    tab.type_and_send("list the files");
    soon(
        "the agent asks for permission",
        move || tab_of(key).is_some_and(|t| t.snapshot().pending_requests().len() == 1),
        move || permission(root, key),
    );
}

fn permission(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let thread = tab.snapshot();
    let request = thread.pending_requests().first().map(|e| (e.id.get(), e.kind.clone()));
    let Some((id, EntryKind::PermissionRequest(asked))) = request else {
        check("the request is a permission", false, "");
        return finish(NAME);
    };
    check(
        "the permission card offers the agent's options",
        asked.options.len() == 3 && asked.tool_name == "ls -la",
        format!("{:?} for {}", asked.options, asked.tool_name),
    );
    main_after(Duration::from_millis(300), move || {
        let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
            return finish(NAME);
        };
        picture(&controller, "acp-permission");
        tab.permission(id, crate::agent::ALLOW_ONCE, None);
        soon(
            "the turn ends after the permission",
            move || {
                tab_of(key).is_some_and(|t| {
                    let thread = t.snapshot();
                    thread.status() == Status::Idle && thread.title() == "Listing files"
                })
            },
            move || done(root, key),
        );
    });
    drop(controller);
}

fn tool_status(tab: &crate::agent::AgentTab, id: &str) -> Option<(ToolStatus, Option<ToolOutput>)> {
    tab.snapshot().entries(&Lane::Main).iter().find_map(|e| match &e.kind {
        EntryKind::ToolCall(call) if call.call_id == id => Some((call.status, call.output.clone())),
        _ => None,
    })
}

/// The turn is over: its cards, the to-dos, the usage and the title.
fn done(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let ls = tool_status(&tab, "call-1");
    check(
        "the allowed command ran and shows its output",
        matches!(&ls, Some((ToolStatus::Completed, Some(ToolOutput::Text(t)))) if t.contains("a.txt")),
        format!("{ls:?}"),
    );
    let edit = tool_status(&tab, "call-2");
    check(
        "an edit shows as a diff",
        matches!(&edit, Some((_, Some(ToolOutput::Diff { .. })))),
        format!("{edit:?}"),
    );
    let seen = tab.inspect();
    check(
        "the agent's plan, title and cost show",
        seen.title == "Listing files" && seen.status.iter().any(|s| s.contains('$')),
        format!("{} | {}", seen.title, seen.status.join(" | ")),
    );
    let (activity, _) = crate::activity::inspect(&controller);
    crate::activity::close(&controller);
    check(
        "the activity view lists the ACP session",
        activity.iter().any(|l| l.starts_with("Listing files")),
        activity.join(" | "),
    );
    tab.timeline().scroll_to_end();
    main_after(Duration::from_millis(400), move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        picture(&controller, "acp-done");
        appearance(&controller, constant!(NSAppearanceNameDarkAqua));
        main_after(Duration::from_millis(400), move || {
            let Some(controller) = controller_of(&root) else {
                return finish(NAME);
            };
            picture(&controller, "acp-dark");
            appearance(&controller, constant!(NSAppearanceNameAqua));
            code_rewind(root, key);
        });
    });
    drop(controller);
}

/// Taking the code back to before an ACP prompt (I6): the app's own checkpoint, with the question about the user's
/// own changes since.
fn code_rewind(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let edited = root.join(ide_acp::test_agent::EDITED.0);
    check(
        "a checkpoint was taken before the ACP prompt",
        tab.checkpoint_count() == 1,
        format!("{}", tab.checkpoint_count()),
    );
    check(
        "the agent's edit is in the folder",
        edited.is_file(),
        edited.display().to_string(),
    );
    let prompt = tab.snapshot().entries(&Lane::Main).iter().find_map(|e| match &e.kind {
        EntryKind::UserMessage(m) if m.message_id.is_some() => Some(e.id.get()),
        _ => None,
    });
    let Some(prompt) = prompt else {
        check("the ACP prompt has the app's id", false, "");
        return finish(NAME);
    };
    let choices = tab.rewind_titles(prompt);
    check(
        "an ACP prompt offers only the code rewind",
        choices == ["Code auf den Stand vor dieser Nachricht"],
        choices.join(", "),
    );
    soon(
        "the turn's checkpoint is finished",
        move || tab_of(key).is_some_and(|t| t.finished_checkpoints() == 1),
        move || {
            let readme = root.join("README.md");
            let _ = std::fs::write(&readme, "# Changed by hand\n");
            crate::agent::answer_alerts(&[NSAlertFirstButtonReturn, NSAlertFirstButtonReturn]);
            if let Some(tab) = tab_of(key) {
                tab.rewind(prompt, 2);
            }
            soon(
                "the agent's edit is taken back",
                move || !edited.exists() && crate::agent::alerts_seen().len() == 2,
                move || {
                    let seen = crate::agent::alerts_seen();
                    check(
                        "rewinding asks about the user's own changes",
                        seen.first().is_some_and(|q| q.contains("selbst geändert"))
                            && seen.get(1).is_some_and(|q| q == "Code zurückgespult"),
                        seen.join(" | "),
                    );
                    let kept = std::fs::read_to_string(&readme).unwrap_or_default();
                    check("the user's own change stays", kept == "# Changed by hand\n", kept);
                    options(root, key);
                },
            );
        },
    );
}

/// Model, mode and the sandbox, as the composer's pills change them.
fn options(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    tab.choose_option("model", "deep");
    tab.choose_mode("code");
    soon(
        "the agent takes the new model and mode",
        move || tab_of(key).is_some_and(|t| t.pills().0 == "Deep" && t.pills().1 == "Code"),
        move || {
            check("the pills follow the agent's model and mode", true, "");
            let Some(tab) = tab_of(key) else { return finish(NAME) };
            tab.toggle_sandbox();
            soon(
                "the session starts again with the sandbox",
                move || tab_of(key).is_some_and(|t| t.is_live() && t.pills().2.starts_with("Sandbox an")),
                move || {
                    check("the sandbox restarts the agent with its switch", true, "");
                    remembered(root, key);
                },
            );
        },
    );
}

/// A new session of the agent starts with the model and mode picked in the first.
fn remembered(root: PathBuf, key: u64) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let Some(second) = crate::agent::open_preset(&controller, GEMINI) else {
        return finish(NAME);
    };
    let other = second.key;
    soon(
        "a new session takes the model and mode picked last",
        move || tab_of(other).is_some_and(|t| t.is_live() && t.pills().0 == "Deep" && t.pills().1 == "Code"),
        move || {
            check(
                "a new session of the agent starts with the model and mode picked last",
                true,
                "",
            );
            if let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(other))
                && let Some(id) = controller.tab_with_key(&tab.tab_key())
            {
                controller.close_tab(id);
            }
            cancel(root, key);
        },
    );
}

fn cancel(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    tab.type_and_send("go slow");
    soon(
        "the slow turn streams",
        move || {
            tab_of(key).is_some_and(|t| {
                t.snapshot()
                    .entries(&Lane::Main)
                    .iter()
                    .any(|e| matches!(&e.kind, EntryKind::AgentMessage(m) if m.markdown == "Working on it"))
            })
        },
        move || {
            if let Some(tab) = tab_of(key) {
                tab.stop_turn();
            }
            soon(
                "the turn stops",
                move || tab_of(key).is_some_and(|t| t.snapshot().status() == Status::Idle),
                move || {
                    let stopped = tab_of(key).is_some_and(|t| {
                        let thread = t.snapshot();
                        matches!(
                            thread.entries(&Lane::Main).last().map(|e| &e.kind),
                            Some(EntryKind::Notice(Notice::Interrupted))
                        )
                    });
                    check("stopping cancels the agent's turn", stopped, "");
                    panel(root);
                },
            );
        },
    );
}

/// The panel lists the session with its agent and offers the agents that are ready; the settings' Agents tab lists
/// every agent's state and the registry (E58).
fn panel(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    controller.activate_panel(Panel::Agents);
    let seen = crate::agent_panel::inspect(&controller);
    let sessions = seen.as_ref().map(|s| s.sessions.clone()).unwrap_or_default();
    check(
        "the session list names the agent",
        sessions.iter().any(|s| s.ends_with("· Gemini")),
        sessions.join(" | "),
    );
    let offered = seen.map(|s| s.new_session).unwrap_or_default();
    check(
        "a new session offers the agents that are ready, not Gemini any more, and the way to manage them",
        offered.iter().any(|t| t == "Codex")
            && !offered.iter().any(|t| t == "Gemini")
            && offered.last().map(String::as_str) == Some(t("Agenten verwalten …")),
        offered.join(" | "),
    );
    main_after(Duration::from_millis(400), move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        picture(&controller, "acp-panel");
        crate::settings::show_agents(MainThreadMarker::from(&*controller));
        let lines = crate::settings::agents_lines();
        check(
            "the settings list every offered agent's state, not Gemini any more, and the loaded registry",
            lines.iter().any(|l| l.starts_with("Codex Bereit"))
                && !lines.iter().any(|l| l.starts_with("Gemini"))
                && lines.iter().any(|l| l.starts_with("Claude über ACP"))
                && lines.iter().any(|l| l.starts_with("ACP-Register 9 Agenten")),
            lines.join(" | "),
        );
        main_after(Duration::from_millis(400), move || registry(root));
    });
}

/// The registry's window from the settings' Agents tab, then the settings close.
fn registry(root: PathBuf) {
    if let Some(window) = crate::settings::window() {
        super::snapshot(&window, "acp-agents");
    }
    let window = crate::settings::open_register();
    let listed = window.as_ref().map_or(0, |(_, ids)| ids.len());
    check(
        "the registry window lists every entry",
        listed == 9,
        format!("{listed}"),
    );
    main_after(Duration::from_millis(400), move || {
        if let Some((window, _)) = crate::settings::register_window()
            && let Some(view) = window.contentView()
        {
            view_picture(&view, "acp-register");
            window.orderOut(None);
        }
        if let Some(window) = crate::settings::window() {
            window.orderOut(None);
        }
        if let Some(controller) = controller_of(&root) {
            controller.activate_panel(Panel::Files);
        }
        sign_in(root);
    });
}

/// An agent that wants a sign-in first opens its session after it.
fn sign_in(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    crate::agent::set_test_launch(test_launch("auth", &root));
    let Some(tab) = crate::agent::open_preset(&controller, CODEX) else {
        return finish(NAME);
    };
    let key = tab.key;
    soon(
        "the agent asks for a sign-in",
        move || tab_of(key).is_some_and(|t| t.problem_name() == Some("sign in")),
        move || {
            let Some(controller) = controller_of(&root) else {
                return finish(NAME);
            };
            picture(&controller, "acp-sign-in");
            if let Some(tab) = tab_of(key) {
                tab.sign_in(0);
            }
            soon(
                "the session opens after the sign-in",
                move || tab_of(key).is_some_and(|t| t.is_live()),
                move || {
                    check("signing in opens the session", true, "");
                    missing(root);
                },
            );
        },
    );
}

/// An agent that is neither on the PATH nor installed offers its installation.
fn missing(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    crate::agent::set_test_launch(None);
    let Some(tab) = crate::agent::open_preset(&controller, OPENCODE) else {
        return finish(NAME);
    };
    let key = tab.key;
    soon(
        "a missing agent says so",
        move || tab_of(key).is_some_and(|t| t.problem_name() == Some("missing")),
        move || {
            let Some(tab) = tab_of(key) else { return finish(NAME) };
            let (_, _, sandbox) = tab.pills();
            check(
                "an agent without a sandbox says so",
                sandbox.contains("keine Sandbox"),
                sandbox,
            );
            main_after(Duration::from_millis(300), move || {
                let Some(controller) = controller_of(&root) else {
                    return finish(NAME);
                };
                picture(&controller, "acp-missing");
                crate::agent::window_closed(&controller);
                end(Instant::now());
            });
        },
    );
}

/// Finishes once no agent process is left.
fn end(since: Instant) {
    let agents = ide_process::Supervisor::global()
        .running()
        .into_iter()
        .filter(|p| p.kind == ide_process::Kind::Agent)
        .count();
    if agents == 0 || since.elapsed() > Duration::from_secs(8) {
        check(
            "closing the tabs ends the agents",
            agents == 0,
            format!("{agents} left"),
        );
        finish(NAME);
    } else {
        main_after(Duration::from_millis(200), move || end(since));
    }
}
