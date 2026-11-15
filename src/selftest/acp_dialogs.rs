//! The `acp-dialogs` scenario (E146): the overlays of an agent over ACP against the test agent of `ide_acp`. The
//! project's `.mcp.json` goes over when the session starts; `/status` shows what the agent reported, `/config` sets
//! its mode, `/mcp` switches a project server and adds one of the user's own whose secret stays out of the state
//! file, `/memory` offers `AGENTS.md`, `/btw` answers in a fork that stays out of the conversation and runs no tools,
//! and `/logout` signs out of the agent. No agent installed on the Mac starts.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::mcp_servers::USER_SERVERS;
use ide_core::settings::State;

use super::agent::{controller_of, picture, tab_of};
use super::dialog_shapes::{Function, check_shape};
use super::{check, finish, give_up, give_up_saying, helpers};
use ide_ui_agent::AgentKey;

const NAME: &str = "acp-dialogs";
const PROJECT: &str = "acp-dialogs-project";
const REGISTRY: &str = include_str!("../../crates/ide-acp/tests/fixtures/registry.json");
const GEMINI: usize = 1;
const MCP_JSON: &str = r#"{"mcpServers": {"docs": {"command": "sh", "args": ["-c", "true"]}}}"#;

fn record() -> PathBuf {
    ide_core::paths::home().join("acp-dialogs-record")
}

fn recorded() -> String {
    std::fs::read_to_string(record()).unwrap_or_default()
}

fn test_launch(root: &Path) -> Option<ide_acp::Launch> {
    let program = std::env::current_exe().ok()?;
    Some(ide_acp::Launch::new(
        "test",
        "Test",
        program,
        Vec::new(),
        vec![
            ("IDE_ACP_TEST_AGENT".into(), "basic".into()),
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("ACP_TEST_RECORD".into(), record().to_string_lossy().into_owned()),
        ],
        root,
    ))
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

fn texts(key: AgentKey) -> Vec<String> {
    tab_of(key).and_then(|tab| tab.dialog_texts()).unwrap_or_default()
}

fn shows(key: AgentKey, text: &str) -> bool {
    texts(key).iter().any(|t| t.contains(text))
}

fn photo(root: &PathBuf, name: &str) {
    if let Some(controller) = controller_of(root) {
        picture(&controller, name);
    }
}

/// Opens `command` in the tab and runs `then` once its overlay shows `marker`.
fn open(key: AgentKey, command: &'static str, marker: &'static str, then: impl FnOnce() + 'static) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send(command);
    soon("the overlay fills", move || shows(key, marker), then);
}

fn dismiss(key: AgentKey) {
    if let Some(tab) = tab_of(key) {
        tab.dismiss_dialog();
    }
}

pub(crate) fn run() {
    if let Ok(register) = ide_acp::Register::parse(REGISTRY) {
        ide_ui_agent::set_test_register(register);
    }
    let made = helpers::make_project(PROJECT, &[("README.md", "# ACP dialogs\n"), (".mcp.json", MCP_JSON)]);
    ide_ui_agent::set_test_launch(made.ok().and_then(|root| test_launch(&root)));
    helpers::open_project(NAME, PROJECT, &[], |controller| {
        let root = helpers::root(PROJECT);
        let Some(tab) = ide_ui_agent::open_preset(&controller, GEMINI) else {
            return give_up(NAME, "the AI tab opens");
        };
        let key = tab.key;
        soon(
            "the ACP session opens",
            move || tab_of(key).is_some_and(|t| t.is_live() && t.acp_capabilities().is_some_and(|c| c.fork)),
            move || {
                let sent = recorded();
                check(
                    "the project's MCP servers go over when the session starts, the program as an absolute path",
                    sent.contains(r#""name":"docs""#) && sent.contains(r#""command":"/bin/sh""#),
                    sent.clone(),
                );
                status(root, key);
            },
        );
    });
}

fn status(root: PathBuf, key: AgentKey) {
    open(key, "/status", "Test Agent", move || {
        check(
            "/status shows the agent, its version, the protocol and what it can do",
            shows(key, "1.0.0") && shows(key, "ACP 1") && shows(key, "Fragen nebenbei") && shows(key, "Abmelden"),
            format!("{:?}", texts(key)),
        );
        photo(&root, "agent-dialog-status-acp");
        check_shape(key, Function::Status);
        dismiss(key);
        config(root, key);
    });
}

fn config(root: PathBuf, key: AgentKey) {
    open(key, "/config", "Modus", move || {
        photo(&root, "agent-dialog-config-acp");
        check_shape(key, Function::Config);
        let Some(tab) = tab_of(key) else {
            return give_up(NAME, "the AI tab is open");
        };
        let chosen = tab.choose_in_dialog("Modus", "Code") && tab.press_dialog_button("Übernehmen");
        soon(
            "the agent takes the mode",
            move || tab_of(key).is_some_and(|t| t.pills().1 == "Code"),
            move || {
                check(
                    "/config lists the agent's modes and options, and Apply sets one",
                    chosen,
                    "",
                );
                dismiss(key);
                mcp(root, key);
            },
        );
    });
}

fn mcp(root: PathBuf, key: AgentKey) {
    open(key, "/mcp", "docs", move || {
        check(
            "/mcp lists the project's servers, which apply from the next start, with what ACP cannot do greyed",
            shows(key, "Gilt ab dem nächsten Sitzungsstart") && shows(key, "Unbekannt"),
            format!("{:?}", texts(key)),
        );
        photo(&root, "agent-dialog-mcp-acp");
        check_shape(key, Function::Mcp);
        let flipped = tab_of(key).is_some_and(|t| t.press_dialog_switch("docs eingeschaltet"));
        soon(
            "the server is switched off for the project",
            move || shows(key, "Ausgeschaltet"),
            move || {
                check("the switch turns a project server off for this project", flipped, "");
                mcp_add(root, key);
            },
        );
    });
}

fn mcp_add(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    let filled = tab.press_dialog_button("Hinzufügen …")
        && tab.fill_dialog_field("docs", "web")
        && tab.fill_dialog_field("npx -y docs-mcp", "npx -y web-mcp")
        && tab.fill_dialog_field("NAME=Wert, NAME2=Wert", "TOKEN=geheim");
    let added = tab.press_dialog_button("Hinzufügen");
    soon(
        "the user's server is listed",
        move || shows(key, "web"),
        move || {
            let stored = State::load_user().get(&USER_SERVERS);
            let state = serde_json::to_string(&stored).unwrap_or_default();
            let secret = ide_ui_agent::own_agent::secrets()
                .get("mcp:web")
                .ok()
                .flatten()
                .unwrap_or_default();
            check(
                "adding a server keeps it in the user's list, its secret in the store of secrets and not in the state",
                filled
                    && added
                    && stored.iter().any(|s| s.name == "web" && s.env == ["TOKEN"])
                    && !state.contains("geheim")
                    && secret.contains("geheim"),
                format!("{state} / {secret}"),
            );
            dismiss(key);
            memory(root, key);
        },
    );
}

fn memory(root: PathBuf, key: AgentKey) {
    open(key, "/memory", "AGENTS.md", move || {
        check(
            "/memory offers the project's AGENTS.md",
            shows(key, "Anweisungen des Projekts") && shows(key, "Anlegen"),
            format!("{:?}", texts(key)),
        );
        check_shape(key, Function::Memory);
        dismiss(key);
        aside(root, key);
    });
}

fn aside(root: PathBuf, key: AgentKey) {
    open(key, "/btw Warum?", "Beside: Warum?", move || {
        let thread = tab_of(key).map(|t| format!("{:?}", t.snapshot())).unwrap_or_default();
        soon(
            "the fork's tool call is refused and the fork closed",
            || recorded().contains("aside permission: \"cancelled\"") && recorded().contains("closed:"),
            move || {
                check(
                    "/btw answers in a fork that stays out of the conversation and runs no tools",
                    !thread.contains("Beside:"),
                    recorded(),
                );
                photo(&root, "agent-dialog-btw-acp");
                dismiss(key);
                logout(key);
            },
        );
    });
}

fn logout(key: AgentKey) {
    open(key, "/logout", "Abmelden", move || {
        check_shape(key, Function::Account);
        helpers::answer_when_shown(objc2_app_kit::NSAlertFirstButtonReturn, None);
        let pressed = tab_of(key).is_some_and(|t| t.press_dialog_button("Abmelden"));
        soon(
            "the agent signs out and the session ends",
            move || recorded().contains("logout") && tab_of(key).is_some_and(|t| !t.is_live()),
            move || {
                check(
                    "/logout asks first, signs out of the agent and ends its sessions",
                    pressed && helpers::answered().iter().any(|m| m.contains("Von Gemini")),
                    format!("{:?}", helpers::answered()),
                );
                finish(NAME);
            },
        );
    });
}
