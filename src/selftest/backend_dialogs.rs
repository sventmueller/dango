//! The overlays of the `codex` and `opencode` scenarios beyond `/status` (E146, E147), against the stand-in servers:
//! `/mcp` lists the servers the agent reports and sends its changes to the agent, which writes its own configuration;
//! `/memory` offers `AGENTS.md`, `/config` applies a mode through the session, and `/export` shows the conversation.
//! Codex also switches a skill off, shows the account without signing out, and sends a report once the user
//! confirmed what goes.

use std::path::PathBuf;
use std::time::Duration;

use objc2_app_kit::NSAlertFirstButtonReturn;
use splat_i18n::t;

use super::agent::tab_of;
use super::dialog_shapes::{Function, check_shape};
use super::{check, give_up_saying, helpers};
use ide_ui_agent::AgentKey;

const WAIT: Duration = Duration::from_secs(15);

/// What the steps go on with.
type Then = Box<dyn FnOnce()>;

/// The overlays of the AI tab `key` in the scenario `name`, whose stand-in logs what it was sent to `log`.
#[derive(Clone, Copy)]
pub(super) struct Walk {
    pub(super) name: &'static str,
    pub(super) key: AgentKey,
    pub(super) log: fn() -> PathBuf,
}

impl Walk {
    fn texts(self) -> Vec<String> {
        tab_of(self.key).and_then(|tab| tab.dialog_texts()).unwrap_or_default()
    }

    fn said(self) -> String {
        format!("{:?}", self.texts())
    }

    fn shows(self, text: &str) -> bool {
        self.texts().iter().any(|t| t.contains(text))
    }

    /// The lines the stand-in was sent that hold each of `parts`.
    fn sent(self, parts: &[&str]) -> Vec<String> {
        std::fs::read_to_string((self.log)())
            .unwrap_or_default()
            .lines()
            .filter(|line| parts.iter().all(|part| line.contains(part)))
            .map(str::to_string)
            .collect()
    }

    fn press(self, title: &str) -> bool {
        tab_of(self.key).is_some_and(|tab| tab.press_dialog_button(title))
    }

    fn switch(self, label: &str) -> bool {
        tab_of(self.key).is_some_and(|tab| tab.press_dialog_switch(label))
    }

    fn fill(self, placeholder: &str, text: &str) -> bool {
        tab_of(self.key).is_some_and(|tab| tab.fill_dialog_field(placeholder, text))
    }

    fn dismiss(self) {
        if let Some(tab) = tab_of(self.key) {
            tab.dismiss_dialog();
        }
    }

    /// Opens `command` and runs `then` once its overlay shows `marker`.
    fn open(self, command: &'static str, marker: &'static str, then: impl FnOnce() + 'static) {
        let Some(tab) = tab_of(self.key) else {
            return give_up_saying(self.name, "the AI tab is open", command);
        };
        tab.type_and_send(command);
        helpers::wait_within(
            WAIT,
            move || self.shows(marker),
            move |shown| {
                if !shown {
                    return give_up_saying(self.name, "the overlay fills", format!("{command}: {}", self.said()));
                }
                then();
            },
        );
    }

    /// Does `act`, a change the overlay sends to the agent, and runs `then` with whether it acted, the stand-in was
    /// sent a new line holding `parts` and the overlay loaded again to show `marker`.
    fn change(
        self,
        (parts, marker): (&'static [&'static str], &'static str),
        act: impl FnOnce() -> bool,
        then: impl FnOnce(bool) + 'static,
    ) {
        let before = self.sent(parts).len();
        let acted = act();
        helpers::wait_within(
            WAIT,
            move || self.sent(parts).len() > before && self.shows(marker) && !self.shows(t("Wird ausgeführt …")),
            move |done| then(acted && done),
        );
    }
}

/// Codex's overlays beyond `/status`, then `then`.
pub(super) fn codex(walk: Walk, then: impl FnOnce() + 'static) {
    let then: Then = Box::new(then);
    walk.open("/mcp", "Anmeldung nötig", move || {
        check(
            "Codex's /mcp lists the servers it reports with their tools and the sign-in they wait for",
            walk.shows("docs") && walk.shows("Anmeldung nötig · Alle meine Projekte · 1 Werkzeuge"),
            walk.said(),
        );
        check_shape(walk.key, Function::Mcp);
        let reload: &'static [&'static str] = &["config/mcpServer/reload"];
        walk.change(
            (reload, "Anmeldung nötig"),
            || walk.press("Neu verbinden"),
            move |done| {
                check(
                    "Codex's /mcp reconnects by reloading Codex's servers",
                    done,
                    walk.said(),
                );
                codex_mcp_add(walk, then);
            },
        );
    });
}

fn codex_mcp_add(walk: Walk, then: Then) {
    let reloads = walk.sent(&["config/mcpServer/reload"]).len();
    let filled = walk.press("Hinzufügen …")
        && walk.fill("docs", "web")
        && walk.fill("npx -y docs-mcp", "sh -c web-mcp")
        && walk.fill("NAME=Wert, NAME2=Wert", "TOKEN=geheim");
    let written: &'static [&'static str] = &["config/value/write", "mcp_servers.web", "web-mcp", "geheim"];
    walk.change(
        (written, "Anmeldung nötig"),
        || walk.press("Hinzufügen"),
        move |done| {
            let reloaded = walk.sent(&["config/mcpServer/reload"]).len() > reloads;
            check(
                "adding a server to Codex writes it into Codex's own configuration and reloads its servers",
                filled && done && reloaded,
                format!("{:?}", walk.sent(&["config/value/write"])),
            );
            walk.dismiss();
            codex_skills(walk, then);
        },
    );
}

fn codex_skills(walk: Walk, then: Then) {
    walk.open("/skills", "review", move || {
        check(
            "Codex's /skills lists its skills with where each comes from",
            walk.shows("Reviews") && walk.shows("repo"),
            walk.said(),
        );
        let off: &'static [&'static str] = &["skills/config/write", r#""enabled":false"#, "review/SKILL.md"];
        walk.change(
            (off, "review"),
            || walk.switch("review eingeschaltet"),
            move |done| {
                check("switching a skill off has Codex turn it off", done, walk.said());
                walk.dismiss();
                memory(
                    walk,
                    Box::new(move || config(walk, Box::new(move || codex_account(walk, then)))),
                );
            },
        );
    });
}

fn codex_account(walk: Walk, then: Then) {
    walk.open("/logout", "dev@example.test", move || {
        check(
            "Codex's /logout shows who is signed in and how",
            walk.shows(t("Angemeldet")) && walk.shows("ChatGPT (plus)"),
            walk.said(),
        );
        check_shape(walk.key, Function::Account);
        walk.dismiss();
        export(walk, Box::new(move || codex_feedback(walk, then)));
    });
}

fn codex_feedback(walk: Walk, then: Then) {
    walk.open("/bug", "Codex sendet an OpenAI", move || {
        let filled = walk.fill("Kurz, worum es geht", "Kaputt")
            && walk.fill("Was ist passiert, und was hättest du erwartet?", "Die Liste fehlt.");
        helpers::answer_when_shown(NSAlertFirstButtonReturn, None);
        let uploaded: &'static [&'static str] = &["feedback/upload", r#""classification":"bug""#, "Die Liste fehlt."];
        let before = walk.sent(uploaded).len();
        let pressed = walk.press("Senden …");
        helpers::wait_within(
            WAIT,
            move || walk.sent(uploaded).len() > before && walk.shows("Gesendet."),
            move |sent| {
                let asked = helpers::answered().iter().any(|m| m.contains(t("Bericht senden?")));
                check(
                    "Codex's /bug sends the report only after the user confirmed what goes, and says it went",
                    filled && pressed && asked && sent && !walk.sent(&["feedback/upload", "Kaputt"]).is_empty(),
                    format!("{:?} {:?}", helpers::answered(), walk.said()),
                );
                walk.dismiss();
                then();
            },
        );
    });
}

/// OpenCode's overlays beyond `/status`, then `then`.
pub(super) fn opencode(walk: Walk, then: impl FnOnce() + 'static) {
    let then: Then = Box::new(then);
    walk.open("/mcp", "Sign in to docs", move || {
        check(
            "OpenCode's /mcp lists its servers with their state and why one waits",
            walk.shows("Anmeldung nötig") && walk.shows("Verbunden") && walk.shows("files"),
            walk.said(),
        );
        check_shape(walk.key, Function::Mcp);
        let off: &'static [&'static str] = &[r#""method":"POST""#, "/api/experimental/mcp/files/disconnect"];
        walk.change(
            (off, "Sign in to docs"),
            || walk.switch("files eingeschaltet"),
            move |disconnected| {
                let on: &'static [&'static str] = &[r#""method":"POST""#, "/api/experimental/mcp/docs/connect"];
                walk.change(
                    (on, "Sign in to docs"),
                    || walk.press("Neu verbinden"),
                    move |connected| {
                        check(
                            "OpenCode's switch disconnects a server and Reconnect connects one",
                            disconnected && connected,
                            walk.said(),
                        );
                        opencode_mcp_edit(walk, then);
                    },
                );
            },
        );
    });
}

/// Adding a server and removing one, after the user confirmed it, go to OpenCode's configuration.
fn opencode_mcp_edit(walk: Walk, then: Then) {
    let filled =
        walk.press("Hinzufügen …") && walk.fill("docs", "web") && walk.fill("npx -y docs-mcp", "sh -c web-mcp");
    let added: &'static [&'static str] = &[r#""method":"PUT""#, "/api/experimental/mcp/web", "web-mcp"];
    walk.change(
        (added, "Sign in to docs"),
        || walk.press("Hinzufügen"),
        move |added| {
            helpers::answer_when_shown(NSAlertFirstButtonReturn, None);
            let removed: &'static [&'static str] = &[r#""method":"DELETE""#, "/api/experimental/mcp/docs"];
            walk.change(
                (removed, "Sign in to docs"),
                || walk.press("Entfernen"),
                move |removed| {
                    let asked = helpers::answered().iter().any(|m| m.contains("„docs“"));
                    check(
                        "OpenCode's /mcp adds a server to OpenCode's configuration and removes one once the user confirmed it",
                        filled && added && removed && asked,
                        format!("{:?} {:?}", helpers::answered(), walk.said()),
                    );
                    walk.dismiss();
                    memory(walk, Box::new(move || config(walk, Box::new(move || export(walk, then)))));
                },
            );
        },
    );
}

/// `/memory` offers the project's `AGENTS.md` to create.
fn memory(walk: Walk, then: Then) {
    walk.open("/memory", "AGENTS.md", move || {
        check(
            "/memory offers the project's AGENTS.md to create",
            walk.shows(t("Anweisungen des Projekts")) && walk.shows(t("Anlegen")),
            walk.said(),
        );
        check_shape(walk.key, Function::Memory);
        walk.dismiss();
        then();
    });
}

/// `/config` applies the mode chosen through the session and loads again, as it was.
fn config(walk: Walk, then: Then) {
    walk.open("/config", "Modus", move || {
        check_shape(walk.key, Function::Config);
        let moded = walk.shows("Modus");
        let before = walk.texts();
        let pressed = walk.press(t("Übernehmen"));
        helpers::wait_within(
            WAIT,
            move || walk.texts() == before,
            move |again| {
                check(
                    "/config applies the mode through the agent's session and loads again",
                    pressed && again && moded,
                    walk.said(),
                );
                walk.dismiss();
                then();
            },
        );
    });
}

/// `/export` shows the conversation with the prompts quoted, to copy or save.
fn export(walk: Walk, then: Then) {
    walk.open("/export", "> list the files", move || {
        check(
            "/export shows the conversation with its prompts quoted, to copy or to save",
            walk.shows(t("Kopieren")) && walk.shows(t("Sichern …")),
            walk.said(),
        );
        walk.dismiss();
        then();
    });
}
