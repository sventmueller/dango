//! The rewind step of the `agent` scenario: a recorded session goes back to just before its second prompt (I6).

use std::path::PathBuf;
use std::time::Duration;

use super::agent::{NAME, QUESTION, controller_of, picture};
use super::agent_rows::squeezed;
use super::{check, finish, helpers};

/// A moment for the timeline to draw the rewound rows before their picture, which shows no state to wait for.
const PICTURE_SETTLE: Duration = Duration::from_millis(400);

/// Rewinding the conversation to just before the second prompt of a recorded session.
pub(super) fn rewind(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let Some(tab) = ide_ui_agent::open_replay(&controller) else {
        return finish(NAME);
    };
    tab.feed(QUESTION);
    let thread = tab.snapshot();
    let main = thread.entries(&ide_thread::Lane::Main);
    let prompts: Vec<(usize, u64, String)> = main
        .iter()
        .enumerate()
        .filter_map(|(n, e)| match &e.kind {
            ide_thread::EntryKind::UserMessage(m) => Some((n, e.id.get(), m.text.clone())),
            _ => None,
        })
        .collect();
    let Some((cut, id, text)) = prompts.get(1).cloned() else {
        check("the recording has two prompts", false, format!("{}", prompts.len()));
        return finish(NAME);
    };
    tab.rewind(id, 1);
    let rows = tab.inspect().rows;
    check(
        "rewinding keeps the turns before the prompt",
        rows == cut,
        format!("{rows} rows, {cut} before the prompt"),
    );
    check(
        "the rewound prompt goes back into the composer",
        tab.composer_text() == text,
        tab.composer_text(),
    );
    check("the rewind waits for the agent to accept it", tab.rewind_pending(), "");
    helpers::after(PICTURE_SETTLE, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-rewind");
        }
        squeezed(root);
    });
}
