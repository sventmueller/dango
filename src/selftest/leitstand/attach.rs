//! The `leitstand-attach` scenario (B37b, E184): a work order's builder runs a scripted session in the shared
//! registry; "Sitzung öffnen" attaches an AI tab to it mid-run, which wears the badge "Auftrag CSV-Export · Builder",
//! keeps the mode and sandbox pills locked, sends the user's message into the driver's queue, and closes without
//! ending the session.

use std::rc::Rc;

use ide_session::{End, Script, ScriptTurn, SessionId, Step};
use ide_thread::{EntryKind, Lane};
use ide_ui_agent::AgentTab;
use objc2::rc::Retained;
use splat_i18n::{t, tf};

use super::super::helpers::{self, wait_until};
use super::super::{check, finish, give_up, snapshot};
use crate::window::WindowController;

const SCENARIO: &str = "leitstand-attach";
const PROJECT: &str = "leitstand-attach-project";
const TITLE: &str = "CSV-Export";
const ANSWER: &str = "Semikolons gehen jetzt auch.";

pub(crate) fn run() {
    helpers::open_project(SCENARIO, PROJECT, &[("README.md", "# CSV\n")], |controller| {
        let Some(root) = controller.project_root() else {
            return give_up(SCENARIO, "the window has a folder");
        };
        let turn = ScriptTurn::new(vec![Step::Say(ANSWER.into())], End::Succeed);
        let script = Script::new(vec![turn]);
        let cx = controller.context();
        let Some(session) = cx.leitstand().start_scripted(&root, "o-attach", TITLE, script) else {
            return give_up(SCENARIO, "the order's scripted session starts");
        };
        if !cx.leitstand().open_session_tab(cx, &controller, session) {
            return give_up(SCENARIO, "the AI tab attaches to the order's session");
        }
        let Some(tab) = attached(&controller, session) else {
            return give_up(SCENARIO, "the attached AI tab is in the window");
        };
        badge_and_pills(&tab);
        tab.type_and_send("Bitte auch Semikolons erlauben.");
        let probe = tab.clone();
        wait_until(
            move || answered(&probe),
            move |found| {
                check(
                    "the user's message goes through the order's driver and its answer shows in the attached tab",
                    found,
                    format!("{:?}", tab.inspect().visible),
                );
                if let Some(window) = controller.window() {
                    window.displayIfNeeded();
                    snapshot(&window, "leitstand-attach");
                }
                close(&controller, &tab, session);
            },
        );
    });
}

/// The window's AI tab on `session`.
fn attached(controller: &WindowController, session: SessionId) -> Option<Rc<AgentTab>> {
    ide_ui_agent::of_window(controller)
        .into_iter()
        .find(|tab| tab.followed_session() == Some(session))
}

/// The badge names the order and the role; mode and sandbox stay as the order set them.
fn badge_and_pills(tab: &AgentTab) {
    let badge = tab.badge();
    let wanted = tf("Auftrag {} · {}", &[TITLE, t("Builder")]);
    check(
        "the AI tab of an order's session wears the badge with the order and the role",
        badge.as_deref() == Some(wanted.as_str()),
        format!("{badge:?}"),
    );
    let (mode, sandbox) = tab.mode_and_sandbox_enabled();
    check(
        "the mode and sandbox pills of an order's session are locked",
        !mode && !sandbox,
        format!("mode enabled {mode}, sandbox enabled {sandbox}"),
    );
    check(
        "the attached tab is named after the order",
        tab.title() == TITLE,
        tab.title(),
    );
}

/// Whether the tab shows the scripted answer to the user's message.
fn answered(tab: &AgentTab) -> bool {
    let thread = tab.snapshot();
    let entries = thread.entries(&Lane::Main);
    let asked = entries
        .iter()
        .any(|e| matches!(&e.kind, EntryKind::UserMessage(m) if m.text.contains("Semikolons")));
    let said = entries
        .iter()
        .any(|e| matches!(&e.kind, EntryKind::AgentMessage(m) if m.markdown.contains(ANSWER)));
    asked && said
}

/// Closes the tab as the user does and checks that the session runs on without it.
fn close(controller: &Retained<WindowController>, tab: &AgentTab, session: SessionId) {
    if let Some(id) = controller.tab_with_key(&tab.tab_key()) {
        controller.close_tab(id);
    }
    let probe = controller.clone();
    let controller = controller.clone();
    wait_until(
        move || attached(&probe, session).is_none(),
        move |closed| {
            let followers = controller.context().leitstand().session_followers(session);
            check(
                "closing the AI tab of an order's session leaves the session running, followed by the runtime alone",
                closed && followers == Some((true, 0)),
                format!("tab closed {closed}, session running and views {followers:?}"),
            );
            controller.context().leitstand().end_session(session);
            finish(SCENARIO);
        },
    );
}
