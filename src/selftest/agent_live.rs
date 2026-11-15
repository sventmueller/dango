//! The `agent-live` scenario: one real turn with the installed `claude` (haiku) in an empty folder, its permission
//! request answered as a user would, then a rewind. Needs a signed-in CLI and the network, so it runs only when named.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_thread::{Filter, ThreadIndex};
use splat_i18n::t;

use super::agent::{DRAWN, controller_of, picture, tab_of};
use super::{check, finish, helpers};
use ide_ui_agent::{AgentKey, AgentTab};

const NAME: &str = "agent-live";
/// How long a real turn of haiku may take.
const TURN: Duration = Duration::from_secs(100);

/// `agent-live`: one real turn with the installed `claude` (haiku) in an empty folder, answering its permission request
/// as a user would. Needs a signed-in CLI and the network, so it runs only when named.
pub fn live() {
    helpers::open_project(NAME, "live-project", &[], |controller| {
        let root = helpers::root("live-project");
        let Some(tab) = ide_ui_agent::open_tab(&controller, None) else {
            return finish(NAME);
        };
        tab.use_model("haiku");
        tab.keep_remote_control_off();
        let key = tab.key;
        let live = move || tab_of(key).is_some_and(|tab| tab.is_live());
        helpers::wait_within(TURN, live, move |_| {
            let Some(tab) = tab_of(key) else { return finish(NAME) };
            tab.type_and_send(
                "Run `echo hello-from-vibeedit` with the Bash tool, then reply with exactly one word: done.",
            );
            helpers::wait_within(
                TURN,
                move || allowed_and_done(key),
                move |done| live_turn(root, key, done),
            );
        });
    });
}

/// Answers a permission request of the tab `key` as the user does; whether its turn ended with the answer "done".
fn allowed_and_done(key: AgentKey) -> bool {
    let Some(tab) = tab_of(key) else { return true };
    let thread = tab.snapshot();
    if let Some(request) = thread.pending_requests().first() {
        tab.permission(request.id.get(), ide_ui_agent::ALLOW_ONCE, None);
        return false;
    }
    answered_with(&tab, "done")
}

fn live_turn(root: PathBuf, key: AgentKey, done: bool) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    if !done {
        let entries = tab.inspect().visible.join(" | ");
        check("a live turn finishes in time", false, entries);
        return finish(NAME);
    }
    let thread = tab.snapshot();
    let output = thread.entries(&ide_thread::Lane::Main).iter().any(|e| match &e.kind {
        ide_thread::EntryKind::ToolCall(call) => call
            .output
            .as_ref()
            .is_some_and(|o| format!("{o:?}").contains("hello-from-vibeedit")),
        _ => false,
    });
    check("a live prompt streams back an answer", done, tab.inspect().title);
    check("the allowed command ran and its output shows", output, "");
    check(
        "a checkpoint was taken before the prompt",
        tab.checkpoint_count() == 1,
        format!("{}", tab.checkpoint_count()),
    );
    let seen = tab.inspect();
    check(
        "the status line shows context and cost",
        !seen.status.is_empty(),
        seen.status.join(" | "),
    );
    check(
        "the head offers Remote Control, off as the thread chose",
        seen.remote.as_deref() == Some(t("Remote Control: auf dem Handy oder auf claude.ai weiterarbeiten")),
        format!("{:?}", seen.remote),
    );
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-live");
        }
        live_rewind(root, key);
    });
}

const SECOND: &str = "Reply with exactly one word: second.";

/// Whether the session is idle with an answer that says `word`.
fn answered_with(tab: &AgentTab, word: &str) -> bool {
    let thread = tab.snapshot();
    thread.status() == ide_thread::Status::Idle
        && thread.entries(&ide_thread::Lane::Main).iter().any(|e| {
            matches!(&e.kind, ide_thread::EntryKind::AgentMessage(m) if !m.streaming && m.markdown.to_lowercase().contains(word))
        })
}

/// A second prompt goes out, the conversation rewinds to before it, and it goes out again.
fn live_rewind(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    tab.type_and_send(SECOND);
    let second = move || tab_of(key).is_some_and(|tab| answered_with(&tab, "second"));
    helpers::wait_within(TURN, second, move |ok| {
        let Some(tab) = tab_of(key).filter(|_| ok) else {
            return rewind_late();
        };
        let original = tab.session_id().unwrap_or_default();
        let thread = tab.snapshot();
        let prompt = thread
            .entries(&ide_thread::Lane::Main)
            .iter()
            .rev()
            .find_map(|e| match &e.kind {
                ide_thread::EntryKind::UserMessage(_) => Some(e.id.get()),
                _ => None,
            });
        if let Some(id) = prompt {
            tab.rewind(id, 1);
        }
        check(
            "the second prompt went back into the composer",
            tab.composer_text() == SECOND,
            tab.composer_text(),
        );
        helpers::wait_within(
            TURN,
            move || tab_of(key).is_some_and(|tab| tab.is_live()),
            move |ok| {
                let Some(tab) = tab_of(key).filter(|_| ok) else {
                    return rewind_late();
                };
                tab.submit();
                helpers::wait_within(TURN, second, move |ok| match tab_of(key).filter(|_| ok) {
                    Some(tab) => live_rewound(&root, &tab, &original),
                    None => rewind_late(),
                });
            },
        );
    });
}

fn rewind_late() {
    let seen = ide_ui_agent::all().first().map(|tab| tab.inspect().visible.join(" | "));
    check("a live rewind finishes in time", false, seen.unwrap_or_default());
    finish(NAME);
}

/// The checks once the prompt went out again on the rewound session.
fn live_rewound(root: &Path, tab: &AgentTab, original: &str) {
    let thread = tab.snapshot();
    let main = thread.entries(&ide_thread::Lane::Main);
    let refused = main.iter().any(|e| {
        matches!(
            &e.kind,
            ide_thread::EntryKind::Notice(ide_thread::Notice::RewindRejected { .. })
        )
    });
    let prompts = main
        .iter()
        .filter(|e| matches!(e.kind, ide_thread::EntryKind::UserMessage(_)))
        .count();
    let now = tab.session_id().unwrap_or_default();
    check(
        "the agent takes a live rewind and goes on as a new branch",
        !refused && prompts == 2 && !now.is_empty() && now != original,
        format!("refused {refused}, {prompts} prompts, session {original} → {now}"),
    );
    let index = ThreadIndex::open(&ThreadIndex::default_dir());
    let kept: Vec<String> = index
        .list(&Filter::default())
        .into_iter()
        .filter(|e| e.session.as_deref() == Some(original))
        .map(|e| e.title.clone())
        .collect();
    check(
        "the session before the rewind stays in the history",
        kept.iter().any(|t| t.contains("vor dem Zurückspulen")),
        kept.join(" | "),
    );
    let root = root.to_path_buf();
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-live-rewind");
            ide_ui_agent::window_closed(&controller);
        }
        live_end();
    });
}

/// Finishes once `claude` has ended: a quit from inside a main-queue block cannot wait for running processes, since
/// the app's answer to the quit comes through that same queue.
fn live_end() {
    let ended = || {
        !ide_process::Supervisor::global()
            .running()
            .into_iter()
            .any(|p| p.kind == ide_process::Kind::Agent)
    };
    helpers::wait_within(Duration::from_secs(8), ended, |ok| {
        check("closing the tab ends claude", ok, "");
        finish(NAME);
    });
}
