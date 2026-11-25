//! The `agent-dialogs` scenario (E146): the overlays of a Claude tab against the stand-in of `agent_stand_in`. A
//! command typed alone opens its overlay instead of going to the CLI and the `/` menu offers it; the overlay fills
//! from the CLI's answer, Esc closes it and gives the focus back to the composer; Copy takes the exported
//! conversation and Save writes it; a request the CLI refuses offers the terminal; two tabs each keep their own
//! overlay; and `/resume` in the empty tab continues the other one.

use std::path::PathBuf;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use serde_json::json;

use super::agent::{controller_of, picture, tab_of};
use super::agent_stand_in::StandIn;
use super::{check, give_up, give_up_saying, helpers};
use ide_ui_agent::{AgentKey, TabMenu};

mod settings;

const NAME: &str = "agent-dialogs";
/// The commands whose overlays Dango offers in a Claude tab (E146), in the menu's order.
pub(super) const LOCAL: [&str; 14] = [
    "/status",
    "/export",
    "/mcp",
    "/permissions",
    "/hooks",
    "/memory",
    "/config",
    "/output-style",
    "/skills",
    "/plugins",
    "/logout",
    "/bug",
    "/btw",
    "/resume",
];
const EXPORTED: &str = "> Hallo\n\nHallo! Wie kann ich helfen?\n";

fn stand_in() -> StandIn {
    StandIn {
        dir: ide_core::paths::home().join("claude-dialogs-stand-in"),
    }
}

fn soon(what: &'static str, ready: impl Fn() -> bool + 'static, then: impl FnOnce() + 'static) {
    helpers::wait_until(ready, move |ok| {
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

pub(crate) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Ok(root) = helpers::make_project("dialogs-project", &[("README.md", "# Dialogs\n")]) else {
        return give_up(NAME, "the project is made");
    };
    crate::project::trust(&root);
    let Ok(prepared) = StandIn::new(&stand_in().dir, &root) else {
        return give_up(NAME, "the stand-in is written");
    };
    let status = json!({"sections": [
        {"title": "Session", "rows": [{"label": "Version", "value": "2.1.325"}, {"label": "cwd", "value": "/tmp/p"}]},
        {"title": "Environment", "rows": [{"label": "Model", "value": "claude-opus-5-5"}]}
    ]});
    let export = json!({"text": EXPORTED, "default_filename": "conversation.txt"});
    let written = prepared
        .write("answer-get_status.json", &status.to_string())
        .and_then(|()| prepared.write("answer-export_conversation.json", &export.to_string()));
    if written.is_err() {
        return give_up(NAME, "the stand-in's answers are written");
    }
    ide_ui_agent::set_test_claude(prepared.program());
    crate::app::open_project(&crate::context::app(), mtm, root.clone());
    helpers::wait_for_window(&root.clone(), move |controller| {
        let Some(controller) = controller else {
            return give_up(NAME, "the folder's window is open");
        };
        let Some(tab) = ide_ui_agent::open_tab(&*controller, None) else {
            return give_up(NAME, "the AI tab opens");
        };
        tab.keep_remote_control_off();
        let key = tab.key;
        soon(
            "the session starts with the stand-in",
            move || tab_of(key).is_some_and(|t| t.is_live()) && !stand_in().requests("initialize").is_empty(),
            move || status_overlay(root, key),
        );
    });
}

/// `/status` typed alone opens its overlay, which shows the CLI's rows; nothing goes to the CLI as a prompt.
fn status_overlay(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    let slash = super::agent_actions::titles(&tab.menu(TabMenu::Slash));
    check(
        "the / menu offers the overlays' commands next to the CLI's",
        slash.iter().any(|t| t == "/status") && slash.iter().any(|t| t == "/export"),
        format!("{slash:?}"),
    );
    tab.type_and_send("/status");
    let opened = tab.dialog_texts().is_some() && tab.composer_text().is_empty();
    soon(
        "the status overlay fills from the CLI's answer",
        move || shows(key, "2.1.325"),
        move || {
            let prompts = stand_in().written().iter().filter(|f| f["type"] == "user").count();
            check(
                "/status opens its overlay with the CLI's rows instead of going to the CLI",
                opened && shows(key, "Version") && shows(key, "claude-opus-5-5") && prompts == 0,
                format!("{:?}, {prompts} prompts", texts(key)),
            );
            if let Some(controller) = controller_of(&root) {
                picture(&controller, "agent-dialog-status");
            }
            super::dialog_shapes::check_shape(key, super::dialog_shapes::Function::Status);
            if let Some(tab) = tab_of(key) {
                tab.press_escape();
                check(
                    "Esc closes the overlay and gives the focus back to the composer",
                    tab.dialog_texts().is_none() && tab.composer_has_focus(),
                    format!("{:?}", tab.dialog_texts()),
                );
            }
            export_overlay(root, key);
        },
    );
}

/// `/export` shows the conversation; Copy puts all of it on the pasteboard.
fn export_overlay(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send("/export");
    soon(
        "the export overlay shows the conversation",
        move || shows(key, "Wie kann ich helfen?"),
        move || {
            let Some(tab) = tab_of(key) else {
                return give_up(NAME, "the AI tab is open");
            };
            let pressed = tab.press_dialog_button("Kopieren");
            let copied = NSPasteboard::generalPasteboard()
                .stringForType(ide_ui_common::constant!(NSPasteboardTypeString))
                .map(|s| s.to_string());
            check(
                "/export shows the conversation and Copy takes it",
                pressed && copied.as_deref() == Some(EXPORTED),
                format!("{copied:?}"),
            );
            export_save(root, key);
        },
    );
}

/// Save writes all of the conversation under the name the agent suggests; in a self-test it goes to `exported` of the
/// self-test's home instead of asking in a save panel.
fn export_save(root: PathBuf, key: AgentKey) {
    let file = ide_core::paths::home().join("exported").join("conversation.txt");
    let _ = std::fs::remove_file(&file);
    let pressed = tab_of(key).is_some_and(|tab| tab.press_dialog_button("Sichern …"));
    let probe = file.clone();
    helpers::wait_until(
        move || probe.is_file(),
        move |written| {
            let text = std::fs::read_to_string(&file).ok();
            check(
                "/export's Save writes all of the conversation under the name the agent suggests",
                pressed && written && text.as_deref() == Some(EXPORTED),
                format!("pressed {pressed}, written {written}: {text:?}"),
            );
            if let Some(tab) = tab_of(key) {
                tab.dismiss_dialog();
            }
            mcp_overlay(root, key);
        },
    );
}

/// `/mcp` lists the servers with their state; the switch, Reconnect, the form's Add and Remove each send what the
/// CLI takes.
fn mcp_overlay(root: PathBuf, key: AgentKey) {
    let servers = json!({"mcpServers": [
        {"name": "docs", "status": "connected", "transport": "stdio", "scope": "local", "tools": [{"name": "a"}, {"name": "b"}]},
        {"name": "broken", "status": "failed", "error": "ENOENT", "transport": "stdio", "scope": "user"},
        {"name": "remote", "status": "needs-auth", "transport": "http", "scope": "user"}
    ]});
    if stand_in()
        .write("answer-mcp_status.json", &servers.to_string())
        .is_err()
    {
        return give_up(NAME, "the servers are written");
    }
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send("/mcp");
    soon(
        "the MCP overlay lists the servers",
        move || shows(key, "broken"),
        move || {
            check(
                "/mcp lists every server with its state and tools, and offers the sign-in where one is missing",
                shows(key, "Verbunden")
                    && shows(key, "2 Werkzeuge")
                    && shows(key, "Fehlgeschlagen")
                    && shows(key, "Anmeldung nötig")
                    && shows(key, "Anmelden"),
                format!("{:?}", texts(key)),
            );
            if let Some(controller) = controller_of(&root) {
                picture(&controller, "agent-dialog-mcp");
            }
            super::dialog_shapes::check_shape(key, super::dialog_shapes::Function::Mcp);
            let searched = tab_of(key).is_some_and(|t| t.search_dialog("broken"));
            check(
                "⌘F searches the overlay and leaves only the rows that match",
                searched && shows(key, "broken") && !shows(key, "remote"),
                format!("{:?}", texts(key)),
            );
            if let Some(tab) = tab_of(key) {
                tab.search_dialog("");
            }
            let Some(tab) = tab_of(key) else {
                return give_up(NAME, "the AI tab is open");
            };
            let flipped = tab.press_dialog_switch("docs eingeschaltet");
            soon(
                "the switch sends mcp_toggle",
                move || !stand_in().requests("mcp_toggle").is_empty(),
                move || {
                    let sent = stand_in().requests("mcp_toggle");
                    check(
                        "the switch turns the server off through the CLI",
                        flipped && sent[0]["serverName"] == "docs" && sent[0]["enabled"] == false,
                        format!("{sent:?}"),
                    );
                    mcp_reconnect(root, key);
                },
            );
        },
    );
}

fn mcp_reconnect(root: PathBuf, key: AgentKey) {
    soon(
        "the list loads again after the switch",
        move || shows(key, "Neu verbinden"),
        move || {
            if let Some(tab) = tab_of(key) {
                tab.press_dialog_button("Neu verbinden");
            }
            soon(
                "Reconnect sends mcp_reconnect",
                move || !stand_in().requests("mcp_reconnect").is_empty(),
                move || {
                    check(
                        "Reconnect asks the CLI to connect the server again",
                        stand_in().requests("mcp_reconnect")[0]["serverName"] == "docs",
                        format!("{:?}", stand_in().requests("mcp_reconnect")),
                    );
                    mcp_add(root, key);
                },
            );
        },
    );
}

fn mcp_add(root: PathBuf, key: AgentKey) {
    soon(
        "the list loads again after Reconnect",
        move || shows(key, "Hinzufügen …"),
        move || {
            let Some(tab) = tab_of(key) else {
                return give_up(NAME, "the AI tab is open");
            };
            let opened = tab.press_dialog_button("Hinzufügen …");
            let filled =
                tab.fill_dialog_field("docs", "web") && tab.fill_dialog_field("npx -y docs-mcp", "npx -y web-mcp");
            let added = tab.press_dialog_button("Hinzufügen");
            let args = stand_in().dir.join("sub-1.args");
            soon(
                "Add runs claude mcp add-json",
                move || args.exists(),
                move || {
                    let args = std::fs::read_to_string(stand_in().dir.join("sub-1.args")).unwrap_or_default();
                    check(
                        "the form adds the server through claude mcp add-json in the chosen scope",
                        opened
                            && filled
                            && added
                            && args.starts_with("mcp\nadd-json\n-s\nlocal\nweb\n")
                            && args.contains(r#""command":"npx""#)
                            && args.contains(r#""args":["-y","web-mcp"]"#),
                        args.clone(),
                    );
                    mcp_remove(root, key);
                },
            );
        },
    );
}

fn mcp_remove(root: PathBuf, key: AgentKey) {
    soon(
        "the list loads again after Add",
        move || shows(key, "Entfernen"),
        move || {
            helpers::answer_when_shown(objc2_app_kit::NSAlertFirstButtonReturn, None);
            if let Some(tab) = tab_of(key) {
                tab.press_dialog_button("Entfernen");
            }
            let args = stand_in().dir.join("sub-2.args");
            soon(
                "Remove runs claude mcp remove",
                move || args.exists(),
                move || {
                    let args = std::fs::read_to_string(stand_in().dir.join("sub-2.args")).unwrap_or_default();
                    check(
                        "Remove asks first, then removes the server from its scope",
                        helpers::answered().iter().any(|m| m.contains("docs"))
                            && args == "mcp\nremove\n-s\nlocal\ndocs\n",
                        args.clone(),
                    );
                    if let Some(tab) = tab_of(key) {
                        tab.dismiss_dialog();
                    }
                    settings::permissions_overlay(root, key);
                },
            );
        },
    );
}

/// A request the CLI refuses says why and offers the terminal.
fn refused(root: PathBuf, key: AgentKey) {
    if stand_in().write("refuse-get_status.txt", "unknown subtype").is_err() {
        return give_up(NAME, "the refusal is written");
    }
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send("/status");
    soon(
        "the refused request offers the terminal",
        move || shows(key, "Im Terminal öffnen"),
        move || {
            check(
                "a request the CLI refuses says so and offers the terminal",
                shows(key, "unknown subtype"),
                format!("{:?}", texts(key)),
            );
            let _ = std::fs::remove_file(stand_in().dir.join("refuse-get_status.txt"));
            two_tabs(root, key);
        },
    );
}

/// Two tabs each keep their own overlay.
fn two_tabs(root: PathBuf, first: AgentKey) {
    let Some(controller) = controller_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let Some(second) = ide_ui_agent::open_tab(&*controller, None) else {
        return give_up(NAME, "a second AI tab opens");
    };
    second.keep_remote_control_off();
    let second = second.key;
    soon(
        "the second tab's session starts",
        move || tab_of(second).is_some_and(|t| t.is_live()),
        move || {
            if let Some(tab) = tab_of(second) {
                tab.type_and_send("/status");
            }
            soon(
                "the second tab's overlay fills",
                move || shows(second, "2.1.325"),
                move || {
                    if let Some(tab) = tab_of(second) {
                        tab.dismiss_dialog();
                    }
                    check(
                        "two tabs each keep their own overlay",
                        texts(second).is_empty() && shows(first, "Im Terminal öffnen"),
                        format!("{:?} / {:?}", texts(first), texts(second)),
                    );
                    settings::asking_overlay(first, second);
                },
            );
        },
    );
}
