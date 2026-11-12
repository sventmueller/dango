//! The `own-agent` scenario: the app's own agent against a stub provider on the loopback interface, so no request
//! leaves the Mac and nothing is paid. The Agent settings keep an endpoint and its key and load its models, which the
//! "+" menu offers, and the folders the sandbox allows; an AI tab reads a file, asks before editing it, edits it, words
//! a refused call in German and answers, with the turn's cost; the code goes back to before the prompt; the thread
//! opens again from its store and is compacted with `/compact`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use ide_agent::stub::{Answer, Stub};
use ide_agent::{Price, PriceTable, ProviderKind};
use ide_core::settings::Settings;
use ide_thread::{EntryKind, Lane, Notice, Status, ToolOutput, ToolStatus};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSControlStateValueOn, NSMenu};
use serde_json::{Value, json};

use super::agent::{controller_of, picture, tab_of};
use super::{check, finish};
use crate::own_agent::{self, ModelChoice};
use crate::util::main_after;

const NAME: &str = "own-agent";
const MODEL: &str = "claude-opus-5-5";
const LIB: &str = "pub fn answer() -> u32 {\n    41\n}\n";

fn event(name: &str, value: &Value) -> String {
    format!("event: {name}\ndata: {value}\n\n")
}

/// A streamed Anthropic answer of `blocks`, given as content blocks with their deltas, stopping for `stop`.
fn stream(blocks: &[(Value, Value)], stop: &str) -> Answer {
    let mut text = event(
        "message_start",
        &json!({"type": "message_start", "message": {"model": MODEL, "usage": {"input_tokens": 1200, "output_tokens": 1}}}),
    );
    for (index, (block, delta)) in blocks.iter().enumerate() {
        text.push_str(&event(
            "content_block_start",
            &json!({"type": "content_block_start", "index": index, "content_block": block}),
        ));
        text.push_str(&event(
            "content_block_delta",
            &json!({"type": "content_block_delta", "index": index, "delta": delta}),
        ));
        text.push_str(&event(
            "content_block_stop",
            &json!({"type": "content_block_stop", "index": index}),
        ));
    }
    text.push_str(&event(
        "message_delta",
        &json!({"type": "message_delta", "delta": {"stop_reason": stop}, "usage": {"output_tokens": 80}}),
    ));
    text.push_str(&event("message_stop", &json!({"type": "message_stop"})));
    Answer::events_slowly(&text, Duration::from_millis(15))
}

fn tool(id: &str, name: &str, input: &Value) -> (Value, Value) {
    (
        json!({"type": "tool_use", "id": id, "name": name, "input": {}}),
        json!({"type": "input_json_delta", "partial_json": input.to_string()}),
    )
}

fn text(words: &str) -> (Value, Value) {
    (
        json!({"type": "text", "text": ""}),
        json!({"type": "text_delta", "text": words}),
    )
}

/// The provider: its model list, then the turn's two answers.
fn provider() -> Option<Stub> {
    let asked = AtomicUsize::new(0);
    Stub::start(move |seen| {
        if seen.path.starts_with("/v1/models") {
            return Answer::json(
                200,
                &json!({"data": [{"id": MODEL, "display_name": "Claude Opus 5.5", "max_input_tokens": 1_000_000}],
                        "has_more": false}),
            );
        }
        match asked.fetch_add(1, Ordering::SeqCst) {
            0 => stream(
                &[
                    text("Ich lese die Datei und ändere die Zahl."),
                    tool("r1", "read_file", &json!({"path": "src/lib.rs"})),
                    tool(
                        "e1",
                        "edit_file",
                        &json!({"path": "src/lib.rs", "old_string": "41", "new_string": "42"}),
                    ),
                    tool("f1", "web_fetch", &json!({"url": "ftp://example.org/a"})),
                ],
                "tool_use",
            ),
            1 => stream(&[text("Die Antwort ist jetzt 42.")], "end_turn"),
            _ => stream(&[text("Die Zahl in src/lib.rs ist auf 42 geändert.")], "end_turn"),
        }
    })
    .ok()
}

fn wait(
    what: &'static str,
    started: Instant,
    ready: impl Fn() -> bool + Send + 'static,
    then: impl FnOnce() + Send + 'static,
) {
    if ready() {
        return then();
    }
    if started.elapsed() > Duration::from_secs(20) {
        check(what, false, "waited in vain");
        return finish(NAME);
    }
    main_after(Duration::from_millis(100), move || wait(what, started, ready, then));
}

fn soon(what: &'static str, ready: impl Fn() -> bool + Send + 'static, then: impl FnOnce() + Send + 'static) {
    wait(what, Instant::now(), ready, then);
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(stub) = provider() else {
        check("the stub provider starts", false, "");
        return finish(NAME);
    };
    let url = stub.url();
    // The stub serves until the app quits.
    std::mem::forget(stub);
    let root = ide_core::paths::home().join("own-project");
    let _ = std::fs::create_dir_all(root.join("src"));
    let _ = std::fs::write(root.join("src/lib.rs"), LIB);
    let _ = std::fs::write(root.join("AGENTS.md"), "Antworte auf Deutsch.\n");
    let root = root.canonicalize().unwrap_or(root);
    crate::project::trust(&root);
    crate::settings::show_agent(mtm);
    settings(root, url);
}

/// The Agent settings show the endpoint, keep its key and load its models.
fn settings(root: PathBuf, url: String) {
    let Some(tab) = crate::settings::agent_tab() else {
        check("the settings have an Agent tab", false, "");
        return finish(NAME);
    };
    crate::settings::agent_selftest::add_endpoint(&tab, ProviderKind::AnthropicCompatible, "Stub", "ftp://stub");
    let refused = crate::settings::agent_selftest::shown(&tab).1;
    check(
        "an address that is not http is refused",
        refused.contains("ftp://stub"),
        refused,
    );
    crate::settings::agent_selftest::add_endpoint(&tab, ProviderKind::AnthropicCompatible, "Stub", &url);
    let rows = crate::settings::agent_selftest::rows(&tab, crate::settings::AgentList::Endpoints);
    let Some(row) = rows.iter().position(|r| r.first().map(String::as_str) == Some("Stub")) else {
        check("the added endpoint is listed", false, format!("{rows:?}"));
        return finish(NAME);
    };
    check(
        "the services and the added endpoint are listed",
        rows.len() == 7 && rows[row][1] == url,
        format!("{rows:?}"),
    );
    crate::settings::agent_selftest::set_key(&tab, row, "test-key");
    let key = crate::settings::agent_selftest::rows(&tab, crate::settings::AgentList::Endpoints)[row][2].clone();
    check(
        "the key is kept in the keychain, not the settings",
        key == "im Schlüsselbund",
        key,
    );
    let settings_text = std::fs::read_to_string(ide_core::paths::data_dir().join("settings.json")).unwrap_or_default();
    check(
        "no settings file holds the key",
        !settings_text.contains("test-key"),
        "",
    );
    sandbox_folders(&tab, &root);
    crate::settings::agent_selftest::choose_endpoint(&tab, row);
    soon(
        "the endpoint's models load",
        || {
            crate::settings::agent_tab().is_some_and(|t| {
                crate::settings::agent_selftest::shown(&t)
                    .1
                    .contains("bietet ein Modell")
            })
        },
        move || {
            if let Some(window) = crate::settings::window() {
                super::snapshot(&window, "own-agent-settings");
                if let Some(tab) = crate::settings::agent_tab() {
                    crate::settings::agent_selftest::reveal_folders(&tab);
                }
                let bars = super::helpers::list_bars(&window);
                check(
                    "each list of the agent settings has its + and − right under it with macOS's own pictures",
                    bars.len() == 4 && bars.iter().all(|(gap, names)| super::ci::native_bar(*gap, names)),
                    format!("{bars:?}"),
                );
                super::snapshot(&window, "own-agent-settings-folders");
                let reach = crate::settings::commit_message_selftest::reach();
                check(
                    "the commit message part stays inside the column of the agent settings",
                    reach.is_some_and(|(reach, column)| reach <= column + 0.5),
                    format!("{reach:?}"),
                );
                super::snapshot(&window, "own-agent-settings-commit");
                window.orderOut(None);
            }
            let choice = Settings::load_user().get(&own_agent::MODEL);
            check(
                "new threads take the endpoint and model chosen",
                choice
                    == Some(ModelChoice {
                        endpoint: "custom-1".into(),
                        model: MODEL.into(),
                    }),
                format!("{choice:?}"),
            );
            plus_menu();
            let mut prices = PriceTable::default();
            prices.models.insert(MODEL.into(), Price::new(5.0, 25.0));
            let _ = Settings::load_user().set(&own_agent::PRICES, &prices);
            open(root);
        },
    );
}

/// The folders the sandbox allows are listed, removed and reach new threads. The self-test's home lies in the
/// temporary folder, which the sandbox always allows, so the probes are folders outside it that exist on every Mac.
fn sandbox_folders(tab: &crate::settings::AgentSettings, root: &std::path::Path) {
    let shared = PathBuf::from("/Library/Fonts");
    let old = ide_core::paths::home().join("old-docs");
    let _ = std::fs::create_dir_all(&old);
    crate::settings::agent_selftest::add_folders(tab, vec![shared.clone(), old.clone(), shared.clone()]);
    crate::settings::agent_selftest::remove_folder(tab, 1);
    let rows = crate::settings::agent_selftest::rows(tab, crate::settings::AgentList::Folders);
    let kept = Settings::load_user().get(&own_agent::ALLOWED);
    check(
        "a sandbox folder is added once and removed again",
        rows == [vec![crate::app::display_path(&shared)]] && kept == [shared.clone()],
        format!("{rows:?} / {kept:?}"),
    );
    let choice = ModelChoice {
        endpoint: "custom-1".into(),
        model: MODEL.into(),
    };
    let reach = own_agent::config(ide_thread::ThreadId::new(), root, &choice, true).map(|config| {
        let workspace = &config.workspace;
        let reaches = |path: &str| workspace.reaches(&workspace.resolve(path));
        (reaches("/Library/Fonts/a.ttf"), reaches("/Library/Caches/a.txt"))
    });
    check(
        "a new thread's sandbox reaches the allowed folder only",
        reach == Ok((true, false)),
        format!("{reach:?}"),
    );
}

/// The "+" menu offers the endpoint with its loaded model, ticked as the default.
fn plus_menu() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let menu = NSMenu::new(mtm);
    crate::agent::fill_own_menu(mtm, &menu);
    let entries: Vec<(String, bool, bool)> = menu
        .itemArray()
        .iter()
        .filter(|item| !item.isSeparatorItem())
        .map(|item| {
            (
                item.title().to_string(),
                item.isEnabled(),
                item.state() == NSControlStateValueOn,
            )
        })
        .collect();
    let stub = entries.iter().position(|(title, _, _)| title == "Stub");
    let worktree = crate::worktrees::agents()
        .into_iter()
        .find(|(agent, _)| agent.is_own())
        .is_some_and(|(_, usable)| usable);
    check(
        "a worktree thread can run the own agent once it has a model",
        worktree,
        "",
    );
    check(
        "the + menu offers the endpoint's model as the default",
        stub.is_some_and(|at| entries.get(at + 1) == Some(&(MODEL.to_string(), true, true)))
            && entries
                .last()
                .is_some_and(|(title, enabled, _)| title == "Anbieter und Modelle …" && *enabled),
        format!("{entries:?}"),
    );
}

fn open(root: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::app::open_project(mtm, root.clone());
    main_after(Duration::from_millis(500), move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        let Some(tab) = crate::agent::open_own(&controller, None) else {
            check("an own-agent tab opens", false, "");
            return finish(NAME);
        };
        let key = tab.key;
        soon(
            "the own agent's session starts",
            move || tab_of(key).is_some_and(|t| t.is_live()),
            move || send(root, key),
        );
    });
}

fn send(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let (model, mode, sandbox) = tab.pills();
    check(
        "the pills show the model, the approval mode and the sandbox",
        model == MODEL && mode == "Manuell" && sandbox.starts_with("Sandbox aus"),
        format!("{model} / {mode} / {sandbox}"),
    );
    tab.type_and_send("Mach die Antwort 42");
    soon(
        "the edit waits for approval",
        move || tab_of(key).is_some_and(|t| t.snapshot().pending_requests().len() == 1),
        move || approve(root, key),
    );
}

fn approve(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let thread = tab.snapshot();
    let read = card(&thread, "r1");
    check(
        "reading in the project runs without asking",
        matches!(&read, Some((ToolStatus::Completed, _))),
        format!("{read:?}"),
    );
    let request = thread.pending_requests().first().map(|e| (e.id.get(), e.kind.clone()));
    let Some((id, EntryKind::PermissionRequest(asked))) = request else {
        return finish(NAME);
    };
    check(
        "the edit asks with allow once, always and deny",
        asked.tool_name == "edit_file" && asked.options.len() == 3,
        format!("{} {:?}", asked.tool_name, asked.options),
    );
    picture(&controller, "own-agent-permission");
    tab.permission(id, crate::agent::ALLOW_ONCE, None);
    soon(
        "the turn ends",
        move || tab_of(key).is_some_and(|t| t.snapshot().status() == Status::Idle),
        move || done(root, key),
    );
}

fn card(thread: &ide_thread::Thread, id: &str) -> Option<(ToolStatus, Option<ToolOutput>)> {
    thread.entries(&Lane::Main).iter().find_map(|e| match &e.kind {
        EntryKind::ToolCall(call) if call.call_id == id => Some((call.status, call.output.clone())),
        _ => None,
    })
}

fn done(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let thread = tab.snapshot();
    let lib = std::fs::read_to_string(root.join("src/lib.rs")).unwrap_or_default();
    check(
        "the approved edit changed the file",
        lib.contains("42") && !lib.contains("41"),
        lib,
    );
    let edit = card(&thread, "e1");
    check(
        "the edit card shows its diff",
        matches!(&edit, Some((ToolStatus::Completed, Some(ToolOutput::Diff { .. })))),
        format!("{edit:?}"),
    );
    let fetch = card(&thread, "f1");
    check(
        "a refused call is worded in the interface language",
        matches!(&fetch, Some((ToolStatus::Failed, Some(ToolOutput::Text(text))))
            if text == "„ftp://example.org/a“ ist keine http- oder https-Adresse."),
        format!("{fetch:?}"),
    );
    let answer = thread.entries(&Lane::Main).iter().rev().find_map(|e| match &e.kind {
        EntryKind::AgentMessage(m) => Some(m.markdown.clone()),
        _ => None,
    });
    check(
        "the answer ends the turn",
        answer.as_deref() == Some("Die Antwort ist jetzt 42."),
        format!("{answer:?}"),
    );
    check(
        "the tokens are counted",
        thread.usage().output_tokens == 160,
        format!("{:?}", thread.usage()),
    );
    let turn = thread.usage().last_turn().cloned();
    let (cost, _) = crate::agent::cost_text(thread.usage());
    check(
        "the turn's cost comes from the user's price",
        turn.as_ref()
            .and_then(|t| t.cost_usd)
            .is_some_and(|c| (c - (2400.0 * 5.0 + 160.0 * 25.0) / 1e6).abs() < 1e-9)
            && cost.contains("Zug"),
        format!("{cost} / {turn:?}"),
    );
    picture(&controller, "own-agent-done");
    code_rewind(root, key);
}

/// The prompt's rewind menu takes the code back to before it (I6, E40), from the app's own checkpoint.
fn code_rewind(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let prompt = tab.snapshot().entries(&Lane::Main).iter().find_map(|e| match &e.kind {
        EntryKind::UserMessage(m) if m.message_id.as_deref().is_some_and(|id| id.starts_with("app-")) => {
            Some(e.id.get())
        }
        _ => None,
    });
    let Some(prompt) = prompt else {
        check("the prompt has the app's id", false, "");
        return finish(NAME);
    };
    let choices = tab.rewind_titles(prompt);
    check(
        "the own agent's prompt offers the code rewind",
        tab.checkpoint_count() == 1 && choices == ["Code auf den Stand vor dieser Nachricht"],
        format!("{} / {}", tab.checkpoint_count(), choices.join(", ")),
    );
    let lib = root.join("src/lib.rs");
    soon(
        "the turn's checkpoint is finished",
        move || tab_of(key).is_some_and(|t| t.finished_checkpoints() == 1),
        move || {
            crate::agent::answer_alerts(&[NSAlertFirstButtonReturn]);
            if let Some(tab) = tab_of(key) {
                tab.rewind(prompt, 2);
            }
            soon(
                "the code is back as before the prompt",
                move || {
                    std::fs::read_to_string(&lib).is_ok_and(|text| text.contains("41"))
                        && !crate::agent::alerts_seen().is_empty()
                },
                move || {
                    let seen = crate::agent::alerts_seen();
                    check(
                        "rewinding the own agent's turn restores the file",
                        seen.first().is_some_and(|q| q == "Code zurückgespult"),
                        seen.join(" | "),
                    );
                    close_and_reopen(root, key);
                },
            );
        },
    );
}

fn close_and_reopen(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let thread_id = tab.thread_id().clone();
    tab.pick_mode_id("edits");
    tab.toggle_sandbox();
    let (_, _, sandbox) = tab.pills();
    check(
        "the sandbox switch says what it limits",
        sandbox.starts_with("Sandbox an"),
        sandbox,
    );
    if let Some(id) = controller.tab_with_key(&tab.tab_key()) {
        controller.close_tab(id);
    }
    main_after(Duration::from_millis(300), move || reopen(root, thread_id));
}

/// The closed thread opens again from its store and shows its conversation.
fn reopen(root: PathBuf, thread_id: ide_thread::ThreadId) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let resume = crate::agent::Resume {
        session: thread_id.as_str().to_string(),
        thread: Some(thread_id),
        title: None,
        transcript: None,
    };
    let Some(tab) = crate::agent::open_agent(&controller, crate::agent::Agent::Own, Some(resume)) else {
        return finish(NAME);
    };
    let key = tab.key;
    soon(
        "the stored thread opens again",
        move || tab_of(key).is_some_and(|t| t.is_live() && !t.snapshot().entries(&Lane::Main).is_empty()),
        move || {
            let Some(tab) = tab_of(key) else { return finish(NAME) };
            let thread = tab.snapshot();
            let edits = card(&thread, "e1");
            check(
                "the resumed thread shows the earlier turn",
                thread.title() == "Mach die Antwort 42" && matches!(edits, Some((ToolStatus::Completed, _))),
                format!("{} / {edits:?}", thread.title()),
            );
            let (_, mode, _) = tab.pills();
            check(
                "the thread opens again in the approval mode picked last",
                mode == splat_i18n::t("Änderungen erlauben"),
                mode,
            );
            if let Some(controller) = controller_of(&root) {
                picture(&controller, "own-agent-resumed");
            }
            compact(root, key);
        },
    );
}

/// `/compact` summarizes the thread with the same model and says so in the timeline.
fn compact(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    tab.type_and_send("/compact");
    soon(
        "the thread is compacted",
        move || {
            tab_of(key).is_some_and(|t| {
                let thread = t.snapshot();
                thread.status() == Status::Idle
                    && thread
                        .entries(&Lane::Main)
                        .iter()
                        .any(|e| matches!(e.kind, EntryKind::Notice(Notice::Compaction { manual: true, .. })))
            })
        },
        move || {
            let notice = tab_of(key).and_then(|t| {
                t.snapshot().entries(&Lane::Main).iter().find_map(|e| match &e.kind {
                    EntryKind::Notice(notice @ Notice::Compaction { .. }) => Some(crate::agent::notice_text(notice)),
                    _ => None,
                })
            });
            check(
                "/compact summarizes the thread and says so",
                notice
                    .as_deref()
                    .is_some_and(|n| n.starts_with("Verdichtet") && n.ends_with("(von Hand)")),
                format!("{notice:?}"),
            );
            if let Some(controller) = controller_of(&root) {
                picture(&controller, "own-agent-compacted");
            }
            finish(NAME);
        },
    );
}
