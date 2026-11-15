//! The `codex` scenario (E147): the Codex choice starts the app-server of `ide_codex::test_server` (this program,
//! started with `IDE_CODEX_TEST_SERVER`) instead of the ACP adapter. Its models and modes fill the composer's
//! menus, a turn asks for a command, which the user allows once, and runs to its end; the overlays Codex offers are
//! in the `/` menu, and `/status` says the route. No Codex on the Mac ever starts.

use std::path::PathBuf;
use std::time::Duration;

use ide_thread::Status;

use super::agent::{controller_of, picture, tab_of};
use super::agent_actions::titles;
use super::{check, finish, give_up, give_up_saying, helpers};
use ide_ui_agent::{AgentKey, TabMenu};

const NAME: &str = "codex";
const PROJECT: &str = "codex-project";
const REGISTRY: &str = include_str!("../../crates/ide-acp/tests/fixtures/registry.json");
/// The place of Codex in the "+" menu.
const CODEX: usize = 0;
/// A moment for the timeline to lay out the card it scrolled to.
const SCROLL_SETTLE: Duration = Duration::from_millis(300);

fn log() -> PathBuf {
    ide_core::paths::home().join("codex-test-log")
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
    let server = std::env::current_exe().ok().zip(made.ok()).map(|(program, root)| {
        ide_codex::Launch::new(
            program,
            vec![
                ("IDE_CODEX_TEST_SERVER".into(), "1".into()),
                ("CODEX_TEST_SCENARIO".into(), "basic".into()),
                ("CODEX_TEST_LOG".into(), log().to_string_lossy().into_owned()),
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
            move || menus(root, key),
        );
    });
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
            finish(NAME);
        },
    );
}
