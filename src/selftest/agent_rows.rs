//! The timeline steps of the `agent` scenario: a card built while its row is squeezed, and a thousand entries built
//! and scrolled without a card for each (E25).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ide_ui_agent::AgentKey;
use serde_json::json;

use super::agent::{NAME, controller_of, long_session, picture, tab_of};
use super::{check, finish, give_up, helpers};

/// How long the timeline gets to lay out a card and the entry after it; a card that took the wrong height stays wrong,
/// so there is no state to wait for.
const LAYOUT_SETTLE: Duration = Duration::from_millis(300);

/// A frame of the session the `squeezed` step plays.
fn squeezed_frame(kind: &str, id: &str, content: serde_json::Value) -> String {
    json!({"type": kind, "message": {"model": "claude-opus-5-5", "id": id, "type": "message",
           "role": if kind == "user" { "user" } else { "assistant" }, "content": content, "stop_reason": null,
           "usage": {"input_tokens": 10, "output_tokens": 20}},
           "parent_tool_use_id": null, "session_id": "51e1f7e5-0000-4000-8000-0000000000ff", "uuid": format!("{id}-uuid")})
    .to_string()
}

/// A shell starts in a timeline that has taken its width.
pub(super) fn squeezed(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let Some(tab) = ide_ui_agent::open_replay(&controller) else {
        return give_up(NAME, "the AI tab opens");
    };
    tab.feed(&squeezed_frame(
        "assistant",
        "msg_sq_1",
        json!([{"type": "tool_use", "id": "toolu_sq", "name": "Bash",
                "input": {"command": "rg -n pattern src | head -40", "description": "Search the sources"}}]),
    ));
    let key = tab.key;
    helpers::wait_until(
        move || tab_of(key).is_some_and(|t| t.inspect().rows > 0),
        move |_| squeezed_output(root, key),
    );
}

/// The shell's output arrives, its card is built, and the next entry follows before the row took the card's height.
fn squeezed_output(root: PathBuf, key: AgentKey) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return give_up(NAME, "the folder's window and the AI tab are open");
    };
    let output: String = (1..=40).map(|n| format!("line {n} of the output\n")).collect();
    tab.feed(&squeezed_frame(
        "user",
        "msg_sq_2",
        json!([{"tool_use_id": "toolu_sq", "type": "tool_result", "content": output}]),
    ));
    if let Some(window) = controller.window() {
        window.layoutIfNeeded();
        window.displayIfNeeded();
    }
    tab.feed(&squeezed_frame(
        "assistant",
        "msg_sq_3",
        json!([{"type": "thinking", "thinking": "Next step.", "signature": ""}]),
    ));
    helpers::after(LAYOUT_SETTLE, move || {
        if let Some(tab) = tab_of(key) {
            let misfits = tab.timeline().misfits();
            check(
                "a shell card keeps its height when the next entry follows its output at once",
                misfits.is_empty(),
                misfits.join(" | "),
            );
        }
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-squeezed");
        }
        super::agent_context::run(root, long);
    });
}

/// A thousand entries: built and scrolled without building a card for each (E25).
fn long(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    let Some(tab) = ide_ui_agent::open_replay(&controller) else {
        return give_up(NAME, "the AI tab opens");
    };
    let notices = ide_ui_agent::notifications_posted();
    let recording = long_session();
    let started = Instant::now();
    tab.feed(&recording);
    tab.timeline().scroll_to_end();
    let fed = started.elapsed();
    let seen = tab.inspect();
    check(
        "a thousand entries load in under a second",
        seen.rows == 1000 && fed < Duration::from_secs(1),
        format!("{} rows in {} ms", seen.rows, fed.as_millis()),
    );
    check(
        "a thousand entries build only the cards in view",
        seen.built < 120,
        format!("{} cards built", seen.built),
    );
    let started = Instant::now();
    for row in (0..1000).step_by(50).rev() {
        tab.timeline().table().scrollRowToVisible(row);
        tab.timeline().table().displayIfNeeded();
    }
    let scrolled = started.elapsed();
    check(
        "scrolling through a thousand entries stays fast",
        scrolled < Duration::from_secs(2),
        format!(
            "20 jumps in {} ms, {} cards built",
            scrolled.as_millis(),
            tab.inspect().built
        ),
    );
    let settled = ide_ui_agent::finished_notice_delay() + Duration::from_secs(1);
    helpers::after(settled, move || {
        let posted = ide_ui_agent::notifications_posted() - notices;
        check(
            "a finished session notifies only while another app is in front",
            posted == 0 || ide_ui_common::notify::app_in_background(),
            format!("{posted} posted"),
        );
        finish(NAME);
    });
}
