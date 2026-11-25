//! The overlays of `agent-dialogs` that change the agent's settings: `/permissions`, `/hooks`, `/memory`, `/config`
//! and `/skills` fill from the CLI's listings, and each change goes through the CLI's own writer or request with the
//! JSON it takes.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::{NAME, shows, soon, stand_in, texts};
use crate::selftest::agent::{controller_of, picture, tab_of};
use crate::selftest::dialog_shapes::{Function, check_shape as shape};
use crate::selftest::{check, give_up, helpers};
use ide_ui_agent::AgentKey;

const FIXTURES: [(&str, &str); 5] = [
    (
        "list_permission_rules",
        include_str!("../../../crates/ide-claude/tests/dialog_fixtures/list_permission_rules.json"),
    ),
    (
        "get_hooks_listing",
        include_str!("../../../crates/ide-claude/tests/dialog_fixtures/get_hooks_listing.json"),
    ),
    (
        "get_memory_dialog",
        include_str!("../../../crates/ide-claude/tests/dialog_fixtures/get_memory_dialog.json"),
    ),
    (
        "get_settings",
        include_str!("../../../crates/ide-claude/tests/dialog_fixtures/get_settings.json"),
    ),
    (
        "get_skills_dialog",
        include_str!("../../../crates/ide-claude/tests/dialog_fixtures/get_skills_dialog.json"),
    ),
];

const WRITERS: [&str; 4] = [
    "edit-permission-rules",
    "edit-hook",
    "edit-memory-settings",
    "edit-skill-overrides",
];

/// What the stand-in got on stdin from each run of the writer `command`, oldest first.
fn edits(command: &str) -> Vec<Value> {
    let dir = stand_in().dir;
    (1..)
        .map_while(|n| {
            let args = std::fs::read_to_string(dir.join(format!("sub-{n}.args"))).ok()?;
            let stdin = std::fs::read_to_string(dir.join(format!("sub-{n}.stdin"))).unwrap_or_default();
            Some((args, stdin))
        })
        .filter(|(args, _)| args.lines().next() == Some(command))
        .map(|(_, stdin)| serde_json::from_str(&stdin).unwrap_or(Value::Null))
        .collect()
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
    soon(
        "the overlay fills from the CLI's listing",
        move || shows(key, marker),
        then,
    );
}

fn dismiss(key: AgentKey) {
    if let Some(tab) = tab_of(key) {
        tab.dismiss_dialog();
    }
}

/// `/permissions` groups the rules by effect; the form adds a rule and Remove takes one away, both through
/// `claude edit-permission-rules`.
pub(super) fn permissions_overlay(root: PathBuf, key: AgentKey) {
    let mut written = FIXTURES.iter().map(|(subtype, answer)| {
        let line = serde_json::from_str::<Value>(answer)
            .map(|v| v.to_string())
            .unwrap_or_default();
        stand_in().write(&format!("answer-{subtype}.json"), &line)
    });
    let answers = written.all(|w| w.is_ok());
    let outs = WRITERS.iter().all(|writer| {
        stand_in()
            .write(&format!("sub-{writer}.out"), "{\"ok\":true}\n")
            .is_ok()
    });
    if !answers || !outs {
        return give_up(NAME, "the listings are written");
    }
    open(key, "/permissions", "Bash(git status)", move || {
        check(
            "/permissions groups the rules as Allow, Ask and Deny",
            shows(key, "ERLAUBEN")
                && shows(key, "FRAGEN")
                && shows(key, "VERBIETEN")
                && shows(key, "Bash(git push *)")
                && shows(key, "Read(./.env)"),
            format!("{:?}", texts(key)),
        );
        photo(&root, "agent-dialog-permissions");
        let Some(tab) = tab_of(key) else {
            return give_up(NAME, "the AI tab is open");
        };
        let filled = tab.fill_dialog_field("Bash(git push *)", "Bash(cargo test *)")
            && tab.choose_in_dialog("Wirkung", "Verbieten");
        let added = tab.press_dialog_button("Hinzufügen");
        soon(
            "Add runs claude edit-permission-rules",
            || !edits("edit-permission-rules").is_empty(),
            move || {
                let sent = edits("edit-permission-rules");
                let want = json!({"op": "add", "rules": ["Bash(cargo test *)"], "behavior": "deny",
                    "destination": "localSettings"});
                check(
                    "the form adds the rule with its effect and scope through the CLI's writer",
                    filled && added && sent[0] == want,
                    format!("{sent:?}"),
                );
                permission_remove(root, key);
            },
        );
    });
}

fn permission_remove(root: PathBuf, key: AgentKey) {
    soon(
        "the rules load again after Add",
        move || shows(key, "Bash(git status)") && tab_of(key).is_some_and(|t| t.press_dialog_button("Entfernen")),
        move || {
            soon(
                "Remove runs claude edit-permission-rules",
                || edits("edit-permission-rules").len() > 1,
                move || {
                    let sent = edits("edit-permission-rules");
                    let want = json!({"op": "remove", "rule": "Bash(git status)", "behavior": "allow",
                        "source": "userSettings"});
                    check(
                        "Remove takes the rule out of the file it lives in",
                        sent[1] == want,
                        format!("{sent:?}"),
                    );
                    dismiss(key);
                    hooks_overlay(root, key);
                },
            );
        },
    );
}

/// `/hooks` lists the hooks by event; Edit changes one through `claude edit-hook`, naming the hook it replaces.
fn hooks_overlay(root: PathBuf, key: AgentKey) {
    open(key, "/hooks", "echo check", move || {
        check(
            "/hooks lists each hook under its event with its matcher and where it lives",
            shows(key, "PreToolUse") && shows(key, "Bash · command") && shows(key, "User settings"),
            format!("{:?}", texts(key)),
        );
        photo(&root, "agent-dialog-hooks");
        let Some(tab) = tab_of(key) else {
            return give_up(NAME, "the AI tab is open");
        };
        let editing = tab.press_dialog_button("Bearbeiten …");
        let filled = tab.fill_dialog_field("echo check", "echo changed");
        let saved = tab.press_dialog_button("Sichern");
        soon(
            "Save runs claude edit-hook",
            || !edits("edit-hook").is_empty(),
            move || {
                let sent = edits("edit-hook");
                let old = json!({"type": "command", "command": "echo check", "timeout": 30});
                let want = json!({"op": "replace", "source": "userSettings", "event": "PreToolUse",
                    "matcher": "Bash", "hook": {"type": "command", "command": "echo changed", "timeout": 30},
                    "target": {"event": "PreToolUse", "matcher": "Bash", "hook": old}});
                check(
                    "Edit replaces the hook in its file through the CLI's writer",
                    editing && filled && saved && sent[0] == want,
                    format!("{sent:?}"),
                );
                dismiss(key);
                memory_overlay(root, key);
            },
        );
    });
}

/// `/memory` shows the instruction files and switches automatic memory through `claude edit-memory-settings`.
fn memory_overlay(root: PathBuf, key: AgentKey) {
    open(key, "/memory", "User instructions", move || {
        check(
            "/memory offers to open the files there are and to create the ones missing",
            shows(key, "Project instructions") && shows(key, "Öffnen") && shows(key, "Anlegen"),
            format!("{:?}", texts(key)),
        );
        photo(&root, "agent-dialog-memory");
        shape(key, Function::Memory);
        let flipped = tab_of(key).is_some_and(|t| t.press_dialog_switch("Automatisches Gedächtnis"));
        soon(
            "the switch runs claude edit-memory-settings",
            || !edits("edit-memory-settings").is_empty(),
            move || {
                let sent = edits("edit-memory-settings");
                check(
                    "the switch turns automatic memory off through the CLI's writer",
                    flipped && sent[0] == json!({"autoMemoryEnabled": false}),
                    format!("{sent:?}"),
                );
                dismiss(key);
                config_overlay(root, key);
            },
        );
    });
}

/// `/config` shows the settings in effect with their source; a new output style goes to the local settings and the
/// CLI reads the styles again.
fn config_overlay(root: PathBuf, key: AgentKey) {
    open(key, "/config", "Explanatory", move || {
        let style = texts(key);
        check(
            "/config shows the settings in effect with where each comes from, and the style in effect as chosen",
            style.get(2).is_some_and(|s| s == "Explanatory")
                && shows(key, "claude-opus-5-5")
                && shows(key, "medium")
                && shows(key, "EINSTELLUNGSDATEIEN"),
            format!("{:?}", texts(key)),
        );
        photo(&root, "agent-dialog-config");
        shape(key, Function::Config);
        let Some(tab) = tab_of(key) else {
            return give_up(NAME, "the AI tab is open");
        };
        let chosen = tab.choose_in_dialog("Ausgabestil", "Learning");
        let applied = tab.press_dialog_button("Übernehmen");
        soon(
            "Apply sends update_settings and reload_output_styles",
            || !stand_in().requests("reload_output_styles").is_empty(),
            move || {
                let sent = stand_in().requests("update_settings");
                check(
                    "Apply sets the output style in the local settings and reloads the styles",
                    chosen
                        && applied
                        && sent.first().is_some_and(|s| {
                            s["source"] == "localSettings" && s["settings"]["outputStyle"] == "Learning"
                        }),
                    format!("{sent:?}"),
                );
                dismiss(key);
                skills_overlay(root, key);
            },
        );
    });
}

/// `/skills` lists the skills with their cost; the switch turns one off through `claude edit-skill-overrides` and the
/// CLI reads the skills again.
fn skills_overlay(root: PathBuf, key: AgentKey) {
    open(key, "/skills", "A demo skill for tests.", move || {
        check(
            "/skills shows each skill with its source and size",
            shows(key, "etwa 9 Tokens"),
            format!("{:?}", texts(key)),
        );
        photo(&root, "agent-dialog-skills");
        let flipped = tab_of(key).is_some_and(|t| t.press_dialog_switch("demo eingeschaltet"));
        soon(
            "the switch runs claude edit-skill-overrides and reload_skills",
            || !stand_in().requests("reload_skills").is_empty(),
            move || {
                let sent = edits("edit-skill-overrides");
                check(
                    "the switch turns the skill off and the CLI reads the skills again",
                    flipped && sent.first() == Some(&json!({"name": "demo", "state": "off"})),
                    format!("{sent:?}"),
                );
                dismiss(key);
                plugins_overlay(root, key);
            },
        );
    });
}

/// The arguments of each `claude plugin <verb>` run, oldest first.
fn plugin_runs(verb: &str) -> Vec<String> {
    let dir = stand_in().dir;
    let head = format!("plugin\n{verb}\n");
    (1..)
        .map_while(|n| std::fs::read_to_string(dir.join(format!("sub-{n}.args"))).ok())
        .filter(|args| args.starts_with(&head))
        .collect()
}

/// `/plugins` lists the installed plugins, finds offered ones and the marketplaces; the switch turns one off, and
/// installing one whose marketplace runs a command shows that command and goes on only once it is accepted.
fn plugins_overlay(root: PathBuf, key: AgentKey) {
    let list = json!({
        "installed": [{"id": "demo@market", "version": "1.0.0", "scope": "project", "enabled": true}],
        "available": [
            {"pluginId": "lint@market", "name": "lint", "marketplaceName": "market", "description": "Lints.", "installCount": 12},
            {"pluginId": "other@market", "name": "other", "marketplaceName": "market", "description": "Else.", "installCount": 3}
        ]
    });
    let markets = json!([{"name": "market", "source": "github", "repo": "owner/market"}]);
    let asking = json!({"ok": false, "shownCommand": {"command": "make install", "sha256": "ab"}});
    let written = [
        ("sub-plugin-list.out", list.to_string()),
        ("sub-plugin-marketplace.out", markets.to_string()),
        ("sub-plugin-disable.out", "{\"ok\":true}".to_string()),
        ("sub-plugin-install.out", "{\"ok\":true}".to_string()),
        ("sub-plugin-install.out.once", asking.to_string()),
    ]
    .iter()
    .all(|(name, text)| stand_in().write(name, text).is_ok());
    if !written {
        return give_up(NAME, "the plugin answers are written");
    }
    open(key, "/plugins", "owner/market", move || {
        check(
            "/plugins lists the installed plugins, the offered ones most installed first, and the marketplaces",
            shows(key, "demo") && shows(key, "market · 1.0.0 · Projekt") && shows(key, "market · 12 Installationen"),
            format!("{:?}", texts(key)),
        );
        photo(&root, "agent-dialog-plugins");
        let Some(tab) = tab_of(key) else {
            return give_up(NAME, "the AI tab is open");
        };
        let searched = tab.fill_dialog_field("Plugin suchen", "lin") && tab.press_dialog_button("Suchen");
        check(
            "a search shows only the offered plugins that match",
            searched && shows(key, "Lints.") && !shows(key, "Else."),
            format!("{:?}", texts(key)),
        );
        let flipped = tab.press_dialog_switch("demo eingeschaltet");
        soon(
            "the switch runs claude plugin disable",
            || !plugin_runs("disable").is_empty(),
            move || {
                check(
                    "the switch turns the plugin off in the scope it is installed in",
                    flipped && plugin_runs("disable")[0] == "plugin\ndisable\ndemo@market\n-s\nproject\n--json\n",
                    format!("{:?}", plugin_runs("disable")),
                );
                plugin_install(root, key);
            },
        );
    });
}

fn plugin_install(root: PathBuf, key: AgentKey) {
    soon(
        "the list loads again after the switch",
        move || shows(key, "Lints."),
        move || {
            helpers::answer_when_shown(objc2_app_kit::NSAlertFirstButtonReturn, None);
            let pressed = tab_of(key).is_some_and(|t| t.press_dialog_button("Installieren"));
            soon(
                "the accepted command goes with the second install",
                || plugin_runs("install").len() > 1,
                move || {
                    let runs = plugin_runs("install");
                    check(
                        "installing shows the marketplace's command and installs only with exactly that one accepted",
                        pressed
                            && helpers::answered().iter().any(|m| m.contains("lint@market"))
                            && runs[0] == "plugin\ninstall\nlint@market\n-s\nlocal\n--json\n"
                            && runs[1].contains("--accept-command\nab\n"),
                        format!("{runs:?} {:?}", helpers::answered()),
                    );
                    dismiss(key);
                    feedback_overlay(root, key);
                },
            );
        },
    );
}

/// `/btw` with a question asks it beside the conversation and shows the answer; the next question carries the
/// earlier one with its answer, and nothing joins the conversation.
fn aside_overlay(root: PathBuf, key: AgentKey) {
    let answer = json!({"response": "Weil der Test es so will.", "synthetic": false});
    if stand_in()
        .write("answer-side_question.json", &answer.to_string())
        .is_err()
    {
        return give_up(NAME, "the side answer is written");
    }
    open(key, "/btw Warum rot?", "Weil der Test es so will.", move || {
        let first = stand_in().requests("side_question");
        check(
            "/btw asks its question beside the conversation and shows the answer",
            first.len() == 1 && first[0]["question"] == "Warum rot?" && first[0].get("history").is_none(),
            format!("{first:?}"),
        );
        photo(&root, "agent-dialog-btw");
        let Some(tab) = tab_of(key) else {
            return give_up(NAME, "the AI tab is open");
        };
        let asked = tab.fill_dialog_field("Was möchtest du wissen?", "Und blau?") && tab.press_dialog_button("Senden");
        soon(
            "the second question goes out",
            || stand_in().requests("side_question").len() > 1,
            move || {
                let sent = stand_in().requests("side_question");
                let prompts = stand_in().written().iter().filter(|f| f["type"] == "user").count();
                check(
                    "the next question carries the earlier one with its answer, and nothing joins the conversation",
                    asked
                        && sent[1]["question"] == "Und blau?"
                        && sent[1]["history"]
                            == json!([{"question": "Warum rot?", "response": "Weil der Test es so will."}])
                        && prompts == 0,
                    format!("{sent:?}, {prompts} prompts"),
                );
                dismiss(key);
                super::refused(root, key);
            },
        );
    });
}

/// `/bug` says what goes to Anthropic, asks before sending, and shows what became of the report.
fn feedback_overlay(root: PathBuf, key: AgentKey) {
    if stand_in()
        .write("answer-submit_feedback.json", r#"{"feedback_id":"r-1"}"#)
        .is_err()
    {
        return give_up(NAME, "the feedback answer is written");
    }
    open(key, "/bug", "Claude Code sendet an Anthropic", move || {
        photo(&root, "agent-dialog-bug");
        let Some(tab) = tab_of(key) else {
            return give_up(NAME, "the AI tab is open");
        };
        let filled = tab.fill_dialog_field("Kurz, worum es geht", "Absturz")
            && tab.fill_dialog_field("Was ist passiert, und was hättest du erwartet?", "Es fiel um.");
        helpers::answer_when_shown(objc2_app_kit::NSAlertFirstButtonReturn, None);
        let pressed = tab.press_dialog_button("Senden …");
        soon(
            "the report says where it went",
            move || shows(key, "r-1"),
            move || {
                let sent = stand_in().requests("submit_feedback");
                check(
                    "/bug asks before sending and sends the report with its kind, title and description",
                    filled
                        && pressed
                        && helpers::answered().iter().any(|m| m.contains("Bericht senden?"))
                        && sent.len() == 1
                        && sent[0]["type"] == "bug"
                        && sent[0]["title"] == "Absturz"
                        && sent[0]["description"] == "Es fiel um.",
                    format!("{sent:?}"),
                );
                dismiss(key);
                aside_overlay(root, key);
            },
        );
    });
}

/// A request of the agent that arrives while an overlay is open is said in the overlay, whose button closes it to
/// answer the request.
pub(super) fn asking_overlay(first: AgentKey, second: AgentKey) {
    open(first, "/status", "2.1.325", move || {
        let quiet = !shows(first, "Der Agent wartet auf deine Antwort.");
        if let Some(tab) = tab_of(first) {
            tab.type_and_send("write hello");
        }
        soon(
            "the overlay says the agent waits",
            move || shows(first, "Der Agent wartet auf deine Antwort."),
            move || {
                let closed =
                    tab_of(first).is_some_and(|t| t.press_dialog_button("Zur Anfrage") && t.dialog_texts().is_none());
                check(
                    "a request that arrives while an overlay is open is said there, and its button leads to it",
                    quiet && closed,
                    format!("{:?}", texts(first)),
                );
                logout_overlay(first, second);
            },
        );
    });
}

/// `/logout` shows who is signed in; signing out asks first, runs `claude auth logout`, and every Claude tab then
/// asks for signing in again.
fn logout_overlay(first: AgentKey, second: AgentKey) {
    let status = json!({"loggedIn": true, "authMethod": "claude.ai", "email": "selftest@example.org"});
    let written = stand_in().write("sub-auth-status.out", &status.to_string()).is_ok()
        && stand_in()
            .write("sub-auth-logout.out", "Successfully logged out.\n")
            .is_ok();
    if !written {
        return give_up(NAME, "the account answers are written");
    }
    open(first, "/logout", "selftest@example.org", move || {
        check(
            "/logout shows who is signed in and how",
            shows(first, "Abo bei claude.ai") && shows(first, "Angemeldet"),
            format!("{:?}", texts(first)),
        );
        shape(first, Function::Account);
        helpers::answer_when_shown(objc2_app_kit::NSAlertFirstButtonReturn, None);
        let pressed = tab_of(first).is_some_and(|t| t.press_dialog_button("Abmelden"));
        let asks_again =
            |key: AgentKey| tab_of(key).is_some_and(|t| t.problem_texts().iter().any(|text| text == "Anmeldung nötig"));
        soon(
            "every Claude tab asks for signing in again",
            move || asks_again(first) && asks_again(second),
            move || {
                let dir = stand_in().dir;
                let logout = (1..)
                    .map_while(|n| std::fs::read_to_string(dir.join(format!("sub-{n}.args"))).ok())
                    .any(|args| args == "auth\nlogout\n");
                check(
                    "signing out asks first, runs claude auth logout and ends the sessions of every Claude tab",
                    pressed && logout && helpers::answered().iter().any(|m| m.contains("abmelden")),
                    format!("{:?}", helpers::answered()),
                );
                resume_overlay(first, second);
            },
        );
    });
}

/// `/resume` in a tab without a conversation lists the folder's other sessions of the agent, not its own; continuing
/// the one open in the first tab brings that tab to the front and the empty tab gives way to it.
fn resume_overlay(first: AgentKey, second: AgentKey) {
    ide_ui_agent::agent_panel::wait_for_writes(std::time::Duration::from_secs(2));
    let Some(title) = tab_of(first).map(|t| t.title()) else {
        return give_up(NAME, "the first AI tab is open");
    };
    let listed = title.clone();
    open(second, "/resume", "Fortsetzen", move || {
        let resumes = texts(second).iter().filter(|t| *t == "Fortsetzen").count();
        check(
            "/resume lists the folder's other session of the agent and not the tab's own",
            shows(second, &listed) && resumes == 1,
            format!("{:?}", texts(second)),
        );
        let pressed = tab_of(second).is_some_and(|t| t.press_dialog_button("Fortsetzen"));
        soon(
            "the empty tab gives way to the session continued",
            move || tab_of(second).is_none(),
            move || {
                let fronted = tab_of(first).is_some_and(|t| t.is_selected());
                check(
                    "continuing a session open in another tab brings that tab to the front and closes the empty one",
                    pressed && fronted,
                    format!("{title:?}, selected {fronted}"),
                );
                crate::selftest::finish(NAME);
            },
        );
    });
}
