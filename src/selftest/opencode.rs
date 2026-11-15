//! The `opencode` scenario (E147): the OpenCode choice starts the server of `ide_opencode::test_server` (this
//! program, started with `IDE_OPENCODE_TEST_SERVER`) instead of `opencode acp`, every request with the password of
//! that start. Its agents and models fill the composer's menus, a turn asks for a command, which the user allows
//! once, and runs to its end; the overlays OpenCode offers are in the `/` menu, and `/status` says the route. No
//! OpenCode on the Mac ever starts.

use std::path::PathBuf;
use std::time::Duration;

use ide_thread::Status;

use super::agent::{controller_of, picture, tab_of};
use super::agent_actions::titles;
use super::{check, finish, give_up, give_up_saying, helpers};
use ide_ui_agent::{AgentKey, TabMenu};

const NAME: &str = "opencode";
const PROJECT: &str = "opencode-project";
const REGISTRY: &str = include_str!("../../crates/ide-acp/tests/fixtures/registry.json");
/// The place of OpenCode in the "+" menu.
const OPENCODE: usize = 3;
/// A moment for the timeline to lay out the card it scrolled to.
const SCROLL_SETTLE: Duration = Duration::from_millis(300);

fn log() -> PathBuf {
    ide_core::paths::home().join("opencode-test-log")
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
    let made = helpers::make_project(PROJECT, &[("README.md", "# OpenCode\n")]);
    ide_ui_agent::set_test_launch(None);
    let server = std::env::current_exe().ok().zip(made.ok()).map(|(program, root)| {
        ide_opencode::Launch::new(
            program,
            vec![
                ("IDE_OPENCODE_TEST_SERVER".into(), "1".into()),
                (ide_opencode::test_server::SCENARIO_VARIABLE.into(), "basic".into()),
                (
                    ide_opencode::test_server::LOG_VARIABLE.into(),
                    log().to_string_lossy().into_owned(),
                ),
                ("PATH".into(), "/usr/bin:/bin".into()),
            ],
            root,
        )
    });
    ide_ui_agent::set_test_opencode(server);
    helpers::open_project(NAME, PROJECT, &[], |controller| {
        let root = helpers::root(PROJECT);
        let Some(tab) = ide_ui_agent::open_preset(&controller, OPENCODE) else {
            return give_up(NAME, "the AI tab opens");
        };
        let key = tab.key;
        soon(
            "the server opens its session",
            move || {
                tab_of(key).is_some_and(|t| t.is_live() && t.acp_capabilities().is_some_and(|c| !c.modes.is_empty()))
            },
            move || menus(root, key),
        );
    });
}

/// The composer's menus offer OpenCode's agents and models, and the `/` menu the overlays OpenCode can fill.
fn menus(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    let modes = titles(&tab.menu(TabMenu::Mode));
    let models = titles(&tab.menu(TabMenu::Model));
    let slash = titles(&tab.menu(TabMenu::Slash));
    let sent = std::fs::read_to_string(log()).unwrap_or_default();
    check(
        "the OpenCode choice speaks its HTTP API, every request with the password of this start",
        sent.contains(r#""path":"/api/session""#) && !sent.contains(r#""authorized":false"#),
        sent.lines().next().unwrap_or_default().to_string(),
    );
    check(
        "the menus offer OpenCode's agents and models, and the / menu the overlays it can fill",
        !modes.is_empty()
            && models.len() > 1
            && ["/status", "/mcp", "/config", "/memory"]
                .iter()
                .all(|c| slash.iter().any(|s| s == c))
            && !slash.iter().any(|s| s == "/btw" || s == "/logout"),
        format!("{modes:?} {models:?} {slash:?}"),
    );
    tab.type_and_send("list the files");
    soon(
        "OpenCode asks to run the command",
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

/// Allowing the command once answers OpenCode's request, and the turn runs to its end.
fn allow(root: PathBuf, key: AgentKey) {
    press_allow(key, move |pressed| {
        soon(
            "the turn ends",
            move || {
                tab_of(key).is_some_and(|t| {
                    let thread = t.snapshot();
                    thread.pending_requests().is_empty() && thread.status() == Status::Idle
                })
            },
            move || ended(root, key, pressed),
        );
    });
}

fn ended(root: PathBuf, key: AgentKey, pressed: bool) {
    let thread = tab_of(key).map(|t| format!("{:?}", t.snapshot())).unwrap_or_default();
    let sent = std::fs::read_to_string(log()).unwrap_or_default();
    let reply = sent
        .lines()
        .find(|l| l.contains("/reply"))
        .unwrap_or_default()
        .to_string();
    check(
        "allowing once answers OpenCode's permission request, and the turn shows the command's output",
        pressed && thread.contains("a.txt") && reply.contains(ide_opencode::test_server::PERMISSION),
        reply,
    );
    if let Some(controller) = controller_of(&root) {
        picture(&controller, "opencode-turn");
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
                .is_some_and(|t| t.iter().any(|x| x == "HTTP v2"))
        },
        move || {
            let texts = tab_of(key).and_then(|t| t.dialog_texts()).unwrap_or_default();
            check(
                "/status says OpenCode runs over its HTTP API, with its version",
                texts.iter().any(|t| t.contains("2.2.6")),
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
