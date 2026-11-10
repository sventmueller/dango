//! The context and cost step of the `agent` scenario (I3): a recording with two turns, each followed by the CLI's
//! context answer, shows the session's and the last turn's cost in the status line; the gauge opens the breakdown
//! only once the CLI answered (E21); the AI panel's session row shows the cost. Pictures in light and dark.

use std::path::PathBuf;
use std::time::Duration;

use objc2_app_kit::{NSAppearanceNameAqua, NSAppearanceNameDarkAqua};
use splat_i18n::t;

use super::agent::{appearance, controller_of, picture, view_picture};
use super::{check, finish};
use crate::util::{constant, main_after};
use crate::window::Panel;

const NAME: &str = "agent";
const CONTEXT: &str = include_str!("../../crates/ide-claude/tests/fixtures/context_cost.ndjson");
const SETTLE: Duration = Duration::from_millis(500);

/// Plays the recording up to its first turn's end, before any context answer, then goes on with `then`.
pub fn run(root: PathBuf, then: fn(PathBuf)) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let Some(tab) = crate::agent::open_replay(&controller) else {
        return finish(NAME);
    };
    let lines: Vec<&str> = CONTEXT.lines().collect();
    let first_end = lines
        .iter()
        .position(|l| l.contains(r#""type":"result""#))
        .map_or(lines.len(), |n| n + 1);
    tab.feed(&lines[..first_end].join("\n"));
    let key = tab.key;
    let thread = tab.snapshot();
    let (cost, _) = crate::agent::cost_text(thread.usage());
    let status = tab.inspect().status;
    check(
        "after a turn the status line shows the session's and the turn's cost",
        !cost.is_empty() && thread.usage().last_turn().is_some_and(|t| t.cost_usd.is_some()) && status.contains(&cost),
        status.join(" | "),
    );
    check(
        "without the CLI's context answer the gauge opens no breakdown",
        !crate::agent::context_seen(key).offered,
        "",
    );
    tab.feed(&lines[first_end..].join("\n"));
    let seen = crate::agent::context_seen(key);
    check("the CLI's answer lets the gauge open the breakdown", seen.offered, "");
    let clicked = crate::agent::click_context(key);
    main_after(SETTLE, move || opened(root, key, clicked, then));
}

fn opened(root: PathBuf, key: u64, clicked: bool, then: fn(PathBuf)) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let seen = crate::agent::context_seen(key);
    let names = seen.rows.join(" | ");
    check(
        "a click on the gauge shows every row of the breakdown",
        clicked && seen.rows.len() == 8 && names.contains(t("Systemprompt")) && names.contains(t("Frei")),
        names,
    );
    picture(&controller, "agent-context");
    if let Some(view) = &seen.view {
        view_picture(view, "agent-context-popover");
    }
    crate::agent::close_context();
    controller.activate_panel(Panel::Agents);
    let details = crate::agent_panel::session_details(&controller);
    let cost = crate::agent::all()
        .into_iter()
        .find(|t| t.key == key)
        .and_then(|t| t.snapshot().usage().cost_usd);
    check(
        "the AI panel's session row shows what the session cost",
        cost.is_some() && details.iter().any(|d| d.contains(" · ")),
        details.join(" | "),
    );
    appearance(&controller, constant!(NSAppearanceNameDarkAqua));
    main_after(SETTLE, move || dark(root, key, then));
}

fn dark(root: PathBuf, key: u64, then: fn(PathBuf)) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    controller.activate_panel(Panel::Files);
    let clicked = crate::agent::click_context(key);
    main_after(SETTLE, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-context-dark");
            let seen = crate::agent::context_seen(key);
            check(
                "the breakdown opens in dark appearance too",
                clicked && seen.view.is_some(),
                "",
            );
            crate::agent::close_context();
            appearance(&controller, constant!(NSAppearanceNameAqua));
        }
        then(root);
    });
}
