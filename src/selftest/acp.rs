//! The `acp` scenario: AI tabs with agents over ACP, played by the test agent of `ide_acp`, which is this program
//! started with `IDE_ACP_TEST_AGENT`. No agent installed on the Mac ever starts: every launch is the test agent or
//! nothing, and the registry is a fixture.

use std::path::PathBuf;
use std::time::Duration;

use ide_thread::{EntryKind, Lane, Notice, Status, ToolOutput, ToolStatus};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSAppearanceNameAqua, NSAppearanceNameDarkAqua};

use splat_i18n::t;

use super::agent::{appearance, controller_of, picture, tab_of, view_picture};
use super::{check, finish, helpers};
use crate::util::constant;
use crate::window::Panel;

const NAME: &str = "acp";
const PROJECT: &str = "acp-project";
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

/// A moment for the timeline to lay out and draw what changed before a picture, which shows no state to wait for.
const SETTLE: Duration = Duration::from_millis(300);

/// Calls `then` once `ready` holds; after `limit` it fails `what` and ends the scenario.
fn wait(what: &'static str, limit: Duration, ready: impl Fn() -> bool + 'static, then: impl FnOnce() + 'static) {
    helpers::wait_within(limit, ready, move |ok| {
        if ok {
            then();
        } else {
            check(what, false, "waited in vain");
            finish(NAME);
        }
    });
}

fn soon(what: &'static str, ready: impl Fn() -> bool + 'static, then: impl FnOnce() + 'static) {
    wait(what, Duration::from_secs(15), ready, then);
}

pub fn run() {
    if let Ok(register) = ide_acp::Register::parse(REGISTRY) {
        ide_ui_agent::set_test_register(register);
    }
    let made = helpers::make_project(PROJECT, &[("README.md", "# ACP project\n")]);
    ide_ui_agent::set_test_launch(made.ok().and_then(|root| test_launch("basic", &root)));
    helpers::open_project(NAME, PROJECT, &[], |controller| {
        let root = helpers::root(PROJECT);
        let names: Vec<String> = ide_ui_agent::preset_menu().into_iter().map(|(n, _)| n).collect();
        check(
            "the new tab menu offers the ACP agents",
            names == ["Codex", "Gemini", "Antigravity", "OpenCode"],
            names.join(", "),
        );
        let Some(tab) = ide_ui_agent::open_preset(&controller, GEMINI) else {
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
        "the empty tab shows neither the agent's name nor an image nor Claude's /model tip",
        texts.is_empty() && !other,
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
    for part in [crate::activity::Part::Subagents, crate::activity::Part::Background] {
        let (tip, enabled, _) = tab.work_button(part);
        check(
            "the head's subagent and background buttons are unavailable for an agent that reports neither",
            !enabled && tip.starts_with("Gemini meldet keine"),
            tip,
        );
    }
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
    helpers::after(SETTLE, move || {
        let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
            return finish(NAME);
        };
        picture(&controller, "acp-permission");
        tab.permission(id, ide_ui_agent::ALLOW_ONCE, None);
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

fn tool_status(tab: &ide_ui_agent::AgentTab, id: &str) -> Option<(ToolStatus, Option<ToolOutput>)> {
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
    helpers::after(SETTLE, move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        picture(&controller, "acp-done");
        appearance(&controller, constant!(NSAppearanceNameDarkAqua));
        helpers::after(SETTLE, move || {
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
            ide_ui_agent::answer_alerts(&[NSAlertFirstButtonReturn, NSAlertFirstButtonReturn]);
            if let Some(tab) = tab_of(key) {
                tab.rewind(prompt, 2);
            }
            soon(
                "the agent's edit is taken back",
                move || !edited.exists() && ide_ui_agent::alerts_seen().len() == 2,
                move || {
                    let seen = ide_ui_agent::alerts_seen();
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
    let Some(second) = ide_ui_agent::open_preset(&controller, GEMINI) else {
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
                tab.stop_turn(ide_ui_agent::Stop::Button);
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
    let seen = ide_ui_agent::agent_panel::inspect(&controller);
    let rows = seen.as_ref().map(|s| s.rows.clone()).unwrap_or_default();
    check(
        "the session list shows the agent's session as an AI session, without the agent's name",
        rows.iter().any(|(kind, _, _)| *kind == "AI") && !rows.iter().any(|(_, _, title)| title.contains("Gemini")),
        format!("{rows:?}"),
    );
    let offered = seen.map(|s| s.new_session).unwrap_or_default();
    check(
        "a new session offers the agents that are ready, not Gemini any more, and the way to manage them",
        offered.iter().any(|t| t == "Codex")
            && !offered.iter().any(|t| t == "Gemini")
            && offered.last().map(String::as_str) == Some(t("Agenten verwalten …")),
        offered.join(" | "),
    );
    helpers::after(SETTLE, move || {
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
        helpers::wait_until(
            || crate::settings::window().is_some_and(|w| w.isVisible()),
            move |_| registry(root),
        );
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
    helpers::wait_until(
        || crate::settings::register_window().is_some_and(|(w, _)| w.isVisible()),
        move |_| {
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
        },
    );
}

/// An agent that wants a sign-in first opens its session after it.
fn sign_in(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    ide_ui_agent::set_test_launch(test_launch("auth", &root));
    let Some(tab) = ide_ui_agent::open_preset(&controller, CODEX) else {
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
                    usage(root, 0);
                },
            );
        },
    );
}

/// Usage-only responses reach the status line even when the agent reports no monetary cost or context window.
fn usage(root: PathBuf, index: usize) {
    let scenarios = ["usage-tokens", "usage-metadata", "usage-credits"];
    let Some(&scenario) = scenarios.get(index) else {
        return missing(root);
    };
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    ide_ui_agent::set_test_launch(test_launch(scenario, &root));
    let Some(tab) = ide_ui_agent::open_preset(&controller, CODEX) else {
        return finish(NAME);
    };
    let key = tab.key;
    soon(
        "the usage agent opens",
        move || tab_of(key).is_some_and(|t| t.is_live()),
        move || {
            if let Some(tab) = tab_of(key) {
                tab.type_and_send("report usage");
            }
            soon(
                "the usage response arrives",
                move || tab_of(key).is_some_and(|t| t.snapshot().usage().reported_tokens.is_some()),
                move || {
                    let Some(tab) = tab_of(key) else { return finish(NAME) };
                    let status = tab.inspect().status.join(" | ");
                    let credit = index == 2;
                    check(
                        "ACP token reports stand on the left like the context, original cost units on the right",
                        if credit {
                            status.contains("2,5 credits") && status.contains("6 % Kontext")
                        } else {
                            status.starts_with("1 k Tokens") && !status.contains("Kontext")
                        },
                        status,
                    );
                    let tip = ide_ui_agent::token_text(tab.snapshot().usage()).map(|(_, tip)| tip);
                    check(
                        "ACP token totals keep their reported breakdown in the tooltip",
                        tip.as_deref().is_some_and(|s| {
                            s.contains("Zuletzt gemeldet: 1 k Tokens")
                                && s.contains("Eingabe: 700")
                                && s.contains("Ausgabe: 300")
                        }),
                        tip.unwrap_or_default(),
                    );
                    if let Some(controller) = controller_of(&root) {
                        picture(&controller, scenario);
                    }
                    usage(root, index + 1);
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
    ide_ui_agent::set_test_launch(None);
    let Some(tab) = ide_ui_agent::open_preset(&controller, OPENCODE) else {
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
            helpers::after(SETTLE, move || {
                let Some(controller) = controller_of(&root) else {
                    return finish(NAME);
                };
                picture(&controller, "acp-missing");
                ide_ui_agent::window_closed(&controller);
                end();
            });
        },
    );
}

/// Finishes once no agent process is left.
fn end() {
    let running = || {
        ide_process::Supervisor::global()
            .running()
            .into_iter()
            .filter(|p| p.kind == ide_process::Kind::Agent)
            .count()
    };
    helpers::wait_within(
        Duration::from_secs(8),
        move || running() == 0,
        move |_| {
            let agents = running();
            check(
                "closing the tabs ends the agents",
                agents == 0,
                format!("{agents} left"),
            );
            finish(NAME);
        },
    );
}
