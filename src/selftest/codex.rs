//! The `codex` scenario (E147): the Codex choice starts the app-server of `ide_codex::test_server` (this program,
//! started with `IDE_CODEX_TEST_SERVER`) instead of the ACP adapter. Its models and modes fill the composer's
//! menus, a turn asks for a command, which the user allows once, and runs to its end; the overlays Codex offers are
//! in the `/` menu, `/status` says the route and the others reach Codex (`backend_dialogs`). The phone in the head
//! turns Remote Control on, which moves the session to the shared server (E148), and its menu turns it off. No Codex
//! on the Mac ever starts.

use std::path::PathBuf;
use std::time::Duration;

use ide_thread::Status;

use super::agent::{controller_of, picture, tab_of};
use super::agent_actions::{index_of, titles};
use super::{check, finish, give_up, give_up_saying, helpers};
use ide_ui_agent::{AgentKey, TabMenu};

mod tasks;

const NAME: &str = "codex";
const PROJECT: &str = "codex-project";
const REGISTRY: &str = include_str!("../../crates/ide-acp/tests/fixtures/registry.json");
/// The place of Codex in the "+" menu.
const CODEX: usize = 0;
/// A moment for the timeline to lay out the card it scrolled to.
const SCROLL_SETTLE: Duration = Duration::from_millis(300);

pub(super) fn log() -> PathBuf {
    ide_core::paths::home().join("codex-test-log")
}

/// Codex's own folder of the stand-in, where it keeps Remote Control's saved choice as the daemon does.
fn codex_home() -> PathBuf {
    ide_core::paths::home().join("codex-home")
}

fn soon(what: &'static str, ready: impl Fn() -> bool + 'static, then: impl FnOnce() + 'static) {
    helpers::wait_within(Duration::from_secs(15), ready, move |ok| {
        if ok {
            then();
        } else {
            give_up_saying(NAME, what, "waited in vain");
        }
    });
}

pub(crate) fn run() {
    if let Ok(register) = ide_acp::Register::parse(REGISTRY) {
        ide_ui_agent::set_test_register(register);
    }
    let made = helpers::make_project(PROJECT, &[("README.md", "# Codex\n")]);
    ide_ui_agent::set_test_launch(None);
    let _ = std::fs::remove_dir_all(codex_home());
    let server = std::env::current_exe().ok().zip(made.ok()).map(|(program, root)| {
        ide_codex::Launch::new(
            program,
            vec![
                ("IDE_CODEX_TEST_SERVER".into(), "1".into()),
                ("CODEX_TEST_SCENARIO".into(), "basic".into()),
                ("CODEX_TEST_LOG".into(), log().to_string_lossy().into_owned()),
                ("CODEX_HOME".into(), codex_home().to_string_lossy().into_owned()),
                ("PATH".into(), "/usr/bin:/bin".into()),
            ],
            root,
        )
    });
    ide_ui_agent::set_test_codex(server);
    helpers::open_project(NAME, PROJECT, &[], |controller| {
        let root = helpers::root(PROJECT);
        let Some(tab) = ide_ui_agent::open_preset(&controller, CODEX) else {
            return give_up(NAME, "the AI tab opens");
        };
        let key = tab.key;
        soon(
            "the app-server opens its thread",
            move || {
                tab_of(key).is_some_and(|t| t.is_live() && t.acp_capabilities().is_some_and(|c| !c.modes.is_empty()))
            },
            move || history(root, key),
        );
    });
}

/// The Agents panel lists the stored Codex threads of the folder that the app has not seen, as `thread/list` names
/// them, to continue with Codex.
fn history(root: PathBuf, key: AgentKey) {
    let Some(controller) = controller_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    controller.activate_panel(crate::window::Panel::Agents);
    let folder = root.clone();
    soon(
        "the Agents panel lists the earlier Codex thread",
        move || {
            controller_of(&folder)
                .and_then(|c| ide_ui_agent::agent_panel::inspect(&c))
                .is_some_and(|seen| seen.rows.iter().any(|(_, _, title)| title == "Begun in the terminal"))
        },
        move || {
            let Some(controller) = controller_of(&root) else {
                return give_up(NAME, "the folder's window is open");
            };
            let row = ide_ui_agent::agent_panel::inspect(&controller)
                .and_then(|seen| seen.rows.iter().position(|(_, _, t)| t == "Begun in the terminal"));
            if let Some(row) = row {
                ide_ui_agent::agent_panel::open_session_row(&controller, row);
            }
            soon(
                "the earlier thread continues with Codex",
                || {
                    std::fs::read_to_string(log()).is_ok_and(|l| {
                        l.lines().any(|line| {
                            line.contains("thread/resume") && line.contains(ide_codex::test_server::OLDER_THREAD)
                        })
                    })
                },
                move || {
                    check(
                        "the Agents panel lists Codex threads begun outside the app and continues them with Codex",
                        true,
                        "",
                    );
                    if let Some(tab) = tab_of(key) {
                        tab.select();
                    }
                    menus(root, key);
                },
            );
        },
    );
}

/// The composer's menus offer Codex's models and approval presets, and the `/` menu the overlays Codex can fill.
fn menus(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    let modes = titles(&tab.menu(TabMenu::Mode));
    let models = titles(&tab.menu(TabMenu::Model));
    let slash = titles(&tab.menu(TabMenu::Slash));
    let sent = std::fs::read_to_string(log()).unwrap_or_default();
    check(
        "the Codex choice speaks the app-server protocol, without the jsonrpc member",
        sent.contains(r#""method":"initialize""#) && !sent.contains("jsonrpc"),
        sent.lines().next().unwrap_or_default().to_string(),
    );
    check(
        "the menus offer Codex's approval presets and models, and the / menu the overlays it can fill",
        modes.iter().any(|m| m == "Nur lesen")
            && models.len() > 1
            && ["/status", "/mcp", "/skills", "/logout", "/bug"]
                .iter()
                .all(|c| slash.iter().any(|s| s == c))
            && !slash.iter().any(|s| s == "/btw"),
        format!("{modes:?} {models:?} {slash:?}"),
    );
    tab.type_and_send("list the files");
    soon(
        "Codex asks to run the command",
        move || tab_of(key).is_some_and(|t| t.snapshot().pending_requests().len() == 1),
        move || allow(root, key),
    );
}

/// Presses Allow once on the newest request card.
fn press_allow(key: AgentKey, then: impl FnOnce(bool) + 'static) {
    if let Some(tab) = tab_of(key) {
        tab.timeline().scroll_to_end();
    }
    helpers::after(SCROLL_SETTLE, move || {
        then(tab_of(key).is_some_and(|t| t.timeline().press_last("Einmal erlauben", 0)));
    });
}

/// Allowing the command and then the file change once answers Codex's requests, and the turn runs to its end.
fn allow(root: PathBuf, key: AgentKey) {
    press_allow(key, move |command| {
        let log = log();
        soon(
            "Codex asks to change a file",
            move || {
                std::fs::read_to_string(&log).is_ok_and(|l| l.contains(r#""id":900"#))
                    && tab_of(key).is_some_and(|t| t.snapshot().pending_requests().len() == 1)
            },
            move || {
                press_allow(key, move |file| {
                    soon(
                        "the turn ends",
                        move || {
                            tab_of(key).is_some_and(|t| {
                                let thread = t.snapshot();
                                thread.pending_requests().is_empty() && thread.status() == Status::Idle
                            })
                        },
                        move || ended(root, key, command && file),
                    );
                });
            },
        );
    });
}

fn ended(root: PathBuf, key: AgentKey, pressed: bool) {
    let thread = tab_of(key).map(|t| format!("{:?}", t.snapshot())).unwrap_or_default();
    let sent = std::fs::read_to_string(log()).unwrap_or_default();
    check(
        "allowing once answers each of Codex's requests with accept, and the turn shows the command's output",
        pressed && thread.contains("a.txt") && sent.matches(r#""decision":"accept""#).count() == 2,
        sent.lines()
            .filter(|l| l.contains("decision"))
            .collect::<Vec<_>>()
            .join(" "),
    );
    if let Some(controller) = controller_of(&root) {
        picture(&controller, "codex-turn");
    }
    status(key);
}

fn status(key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send("/status");
    soon(
        "/status fills",
        move || {
            tab_of(key)
                .and_then(|t| t.dialog_texts())
                .is_some_and(|t| t.iter().any(|x| x == "app-server"))
        },
        move || {
            let texts = tab_of(key).and_then(|t| t.dialog_texts()).unwrap_or_default();
            check(
                "/status says Codex runs over its app-server, with its version",
                texts.iter().any(|t| t == "0.167.0"),
                format!("{texts:?}"),
            );
            super::dialog_shapes::check_shape(key, super::dialog_shapes::Function::Status);
            if let Some(tab) = tab_of(key) {
                tab.dismiss_dialog();
            }
            let walk = super::backend_dialogs::Walk { name: NAME, key, log };
            super::backend_dialogs::codex(walk, move || question(key));
        },
    );
}

/// A question of Codex is answered from its card, and the turn goes on.
fn question(key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send("ask me");
    soon(
        "Codex asks a question",
        move || {
            tab_of(key).is_some_and(|t| {
                t.snapshot()
                    .pending_requests()
                    .iter()
                    .any(|e| matches!(e.kind, ide_thread::EntryKind::Question(_)))
            })
        },
        move || {
            press_answer(key, move |pressed| {
                soon(
                    "the answer reaches Codex",
                    || std::fs::read_to_string(log()).is_ok_and(|l| l.contains("Blue")),
                    move || {
                        let waiting = tab_of(key).is_some_and(|t| !t.snapshot().pending_requests().is_empty());
                        check("a question of Codex is answered from its card", pressed && !waiting, "");
                        rewind(key);
                    },
                );
            });
        },
    );
}

/// Presses the option Blue on the newest question card.
fn press_answer(key: AgentKey, then: impl FnOnce(bool) + 'static) {
    if let Some(tab) = tab_of(key) {
        tab.timeline().scroll_to_end();
    }
    helpers::after(SCROLL_SETTLE, move || {
        then(tab_of(key).is_some_and(|t| t.timeline().press_last("Blue", 0)));
    });
}

/// Rewinding the conversation to before the second prompt has Codex drop that turn from its stored thread, then
/// resumes it; the prompt's text is back in the composer.
fn rewind(key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    let prompts = tab.entry_ids(|kind| matches!(kind, ide_thread::EntryKind::UserMessage(_)));
    let Some(&second) = prompts.get(1) else {
        return give_up_saying(NAME, "both prompts are in the conversation", format!("{prompts:?}"));
    };
    let offered = tab.rewind_titles(second);
    if let Some(menu) = tab.rewind_menu_of(second) {
        menu.performActionForItemAtIndex(0);
    }
    soon(
        "Codex drops the turn and the session resumes",
        || {
            let sent = std::fs::read_to_string(log()).unwrap_or_default();
            let reverted = sent
                .lines()
                .position(|l| l.contains("thread/revert") && l.contains("turn-2"));
            reverted.is_some_and(|at| sent.lines().skip(at).any(|l| l.contains("thread/resume")))
        },
        move || {
            let text = tab_of(key).map(|t| t.composer_text()).unwrap_or_default();
            check(
                "rewinding a later prompt has Codex drop its turn, resumes the thread and gives the prompt back",
                offered.iter().any(|t| t.contains("Unterhaltung")) && text == "ask me",
                format!("{offered:?} {text:?}"),
            );
            remote(key);
        },
    );
}

/// What the phone in the head says while it is on.
fn remote_shown_on() -> &'static str {
    splat_i18n::t("Remote Control ist an: die Codex-Sitzungen dieses Macs laufen auch in Codex auf dem Handy")
}

/// What the phone in the head says while it is off.
fn remote_off() -> &'static str {
    splat_i18n::t("Remote Control: auf dem Handy in Codex weiterarbeiten")
}

/// The phone in the head turns Remote Control on, which moves the session to Codex's shared server; its menu offers
/// pairing and turns Remote Control off again, for every Codex session of the Mac.
fn remote(key: AgentKey) {
    soon(
        "the phone shows in the head of the resumed session",
        move || tab_of(key).is_some_and(|t| t.is_live() && t.inspect().remote.as_deref() == Some(remote_off())),
        move || remote_on(key),
    );
}

/// The phone turns Remote Control on, and its menu turns it off.
fn remote_on(key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.remote_control(&tab.root());
    soon(
        "the session moves to Codex's shared server",
        move || tab_of(key).is_some_and(|t| t.inspect().remote.as_deref() == Some(remote_shown_on())),
        move || {
            let Some(tab) = tab_of(key) else {
                return give_up(NAME, "the AI tab is open");
            };
            let shared = std::fs::read_to_string(log()).is_ok_and(|l| l.contains(r#""experimentalApi":true"#));
            let menu = tab.menu(TabMenu::Remote);
            let shown = titles(&menu);
            if let Some(end) = index_of(&menu, "Remote Control beenden") {
                menu.performActionForItemAtIndex(end);
            }
            soon(
                "Remote Control turns off",
                move || {
                    std::fs::read_to_string(log()).is_ok_and(|l| l.contains("remoteControl/disable"))
                        && tab_of(key).is_some_and(|t| t.inspect().remote.as_deref() == Some(remote_off()))
                },
                move || {
                    check(
                        "the phone moves the Codex session to the shared server, and its menu pairs or turns it off",
                        shared && shown.len() == 3,
                        format!("{shared} {shown:?}"),
                    );
                    tasks::run(key);
                },
            );
        },
    );
}
