//! The `agent-live` scenario: one real turn with the installed `claude` (haiku) in an empty folder, its permission
//! request answered as a user would, then a rewind. Needs a signed-in CLI and the network, so it runs only when named.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_thread::{Filter, ThreadIndex};
use objc2::MainThreadMarker;
use splat_i18n::t;

use super::agent::{controller_of, picture, tab_of};
use super::{check, finish};
use crate::agent::AgentTab;
use crate::util::main_after;

/// `agent-live`: one real turn with the installed `claude` (haiku) in an empty folder, answering its permission request
/// as a user would. Needs a signed-in CLI and the network, so it runs only when named.
pub fn live() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = ide_core::paths::home().join("live-project");
    let _ = std::fs::create_dir_all(&root);
    let root = root.canonicalize().unwrap_or(root);
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    let Some(controller) = controller_of(&root) else {
        return finish("agent-live");
    };
    let Some(tab) = crate::agent::open_tab(&controller, None) else {
        return finish("agent-live");
    };
    tab.use_model("haiku");
    tab.keep_remote_control_off();
    let key = tab.key;
    let started = Instant::now();
    main_after(Duration::from_millis(500), move || live_wait(root, key, started, false));
}

fn live_wait(root: PathBuf, key: u64, started: Instant, sent: bool) {
    let name = "agent-live";
    let Some(tab) = tab_of(key) else { return finish(name) };
    if started.elapsed() > Duration::from_secs(100) {
        let entries = tab.inspect().visible.join(" | ");
        check("a live turn finishes in time", false, entries);
        return finish(name);
    }
    let later = root.clone();
    let again = move |sent: bool| main_after(Duration::from_millis(500), move || live_wait(later, key, started, sent));
    if !sent {
        if !tab.is_live() {
            return again(false);
        }
        tab.type_and_send("Run `echo hello-from-vibeedit` with the Bash tool, then reply with exactly one word: done.");
        return again(true);
    }
    let thread = tab.snapshot();
    if let Some(request) = thread.pending_requests().first() {
        let id = request.id.get();
        tab.permission(id, crate::agent::ALLOW_ONCE, None);
        return again(true);
    }
    let answered = thread.entries(&ide_thread::Lane::Main).iter().any(|e| {
        matches!(&e.kind, ide_thread::EntryKind::AgentMessage(m) if !m.streaming && m.markdown.to_lowercase().contains("done"))
    });
    if thread.status() != ide_thread::Status::Idle || !answered {
        return again(true);
    }
    let output = thread.entries(&ide_thread::Lane::Main).iter().any(|e| match &e.kind {
        ide_thread::EntryKind::ToolCall(call) => call
            .output
            .as_ref()
            .is_some_and(|o| format!("{o:?}").contains("hello-from-vibeedit")),
        _ => false,
    });
    check("a live prompt streams back an answer", answered, tab.inspect().title);
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
    main_after(Duration::from_millis(400), move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-live");
        }
        live_rewind(root, key, Instant::now(), Stage::Ask, String::new());
    });
}

/// Where the live rewind is: a second prompt goes out, the conversation rewinds to before it, and it goes out again.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Ask,
    Answered,
    Rewound,
    Resent,
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

fn live_rewind(root: PathBuf, key: u64, started: Instant, stage: Stage, original: String) {
    let name = "agent-live";
    let Some(tab) = tab_of(key) else { return finish(name) };
    if started.elapsed() > Duration::from_secs(150) {
        check(
            "a live rewind finishes in time",
            false,
            tab.inspect().visible.join(" | "),
        );
        return finish(name);
    }
    let later = root.clone();
    let session = original.clone();
    let next = move |stage: Stage| {
        main_after(Duration::from_millis(500), move || {
            live_rewind(later, key, started, stage, session)
        });
    };
    match stage {
        Stage::Ask => {
            tab.type_and_send(SECOND);
            next(Stage::Answered);
        }
        Stage::Answered if !answered_with(&tab, "second") => next(Stage::Answered),
        Stage::Answered => {
            let original = tab.session_id().unwrap_or_default();
            let thread = tab.snapshot();
            let second = thread
                .entries(&ide_thread::Lane::Main)
                .iter()
                .rev()
                .find_map(|e| match &e.kind {
                    ide_thread::EntryKind::UserMessage(_) => Some(e.id.get()),
                    _ => None,
                });
            if let Some(id) = second {
                tab.rewind(id, 1);
            }
            check(
                "the second prompt went back into the composer",
                tab.composer_text() == SECOND,
                tab.composer_text(),
            );
            let later = root.clone();
            main_after(Duration::from_millis(500), move || {
                live_rewind(later, key, started, Stage::Rewound, original)
            });
        }
        Stage::Rewound if !tab.is_live() => next(Stage::Rewound),
        Stage::Rewound => {
            tab.submit();
            next(Stage::Resent);
        }
        Stage::Resent if !answered_with(&tab, "second") => next(Stage::Resent),
        Stage::Resent => live_rewound(&root, &tab, &original),
    }
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
    main_after(Duration::from_millis(400), move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-live-rewind");
            crate::agent::window_closed(&controller);
        }
        live_end(Instant::now());
    });
}

/// Finishes once `claude` has ended: a quit from inside a main-queue block cannot wait for running processes, since
/// the app's answer to the quit comes through that same queue.
fn live_end(since: Instant) {
    let agents = ide_process::Supervisor::global()
        .running()
        .into_iter()
        .filter(|p| p.kind == ide_process::Kind::Agent)
        .count();
    if agents == 0 || since.elapsed() > Duration::from_secs(8) {
        check("closing the tab ends claude", agents == 0, format!("{agents} left"));
        finish("agent-live");
    } else {
        main_after(Duration::from_millis(200), move || live_end(since));
    }
}
