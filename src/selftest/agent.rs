//! The `agent` scenario: an AI tab fed by recordings of real Claude sessions, without a process or the network.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use ide_thread::{Filter, ThreadIndex};
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAccessibilityHighContrastAqua,
    NSAppearanceNameAccessibilityHighContrastDarkAqua, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
};
use objc2_foundation::{NSRect, NSSize};
use serde_json::json;

use super::{check, finish, snapshot};
use crate::agent::AgentTab;
use crate::util::{constant, main_after};
use crate::window::{self, Panel, WindowController};

const SUBAGENT: &str = include_str!("../../crates/ide-claude/tests/fixtures/recorded_subagent_background.ndjson");
const EDIT: &str = include_str!("../../crates/ide-claude/tests/fixtures/recorded_edit_diff.ndjson");
const QUESTION: &str = include_str!("../../crates/ide-claude/tests/fixtures/recorded_question.ndjson");
const PLAN: &str = include_str!("../../crates/ide-claude/tests/fixtures/plan_todos.ndjson");
const COMPACTION: &str = include_str!("../../crates/ide-claude/tests/fixtures/compaction.ndjson");
const RETRY: &str = include_str!("../../crates/ide-claude/tests/fixtures/api_retry.ndjson");
const LATE: &str = include_str!("../../crates/ide-claude/tests/fixtures/late_runs.ndjson");
const HOOKS: &str = include_str!("../../crates/ide-claude/tests/fixtures/hooks.ndjson");

const NAME: &str = "agent";

/// The first `count` lines of `text`, and the rest.
fn split(text: &str, count: usize) -> (String, String) {
    let lines: Vec<&str> = text.lines().collect();
    let count = count.min(lines.len());
    (lines[..count].join("\n"), lines[count..].join("\n"))
}

/// The lines of `text` up to and including the first that contains `marker`, and the rest.
fn until(text: &str, marker: &str) -> (String, String) {
    let at = text.lines().position(|l| l.contains(marker)).map_or(0, |n| n + 1);
    split(text, at)
}

/// A turn with a slash command and an answer in rich markdown, written like the CLI's frames.
fn markdown_turn() -> String {
    let answer = "## Deploy to staging\n\nThe build is **green**; three steps remain:\n\n1. Tag the release \
                  `v0.4.2`\n2. Push the image\n3. Run the smoke tests\n\n| Step | Time | State |\n|---|---:|---|\n\
                  | Build | 2 min | done |\n| Image | 40 s | next |\n| Smoke tests | 1 min | waiting |\n\n```rust\n\
                  fn main() {\n    let target = \"staging\"; // from the command\n    deploy(target, 3);\n}\n```\n\n> The \
                  smoke tests run against `staging.example.com`.";
    [
        format!(
            ">> {}",
            json!({"type": "user", "uuid": "51e1f7e5-0000-4000-8000-000000000001", "origin": {"kind": "human"},
                   "message": {"role": "user", "content": [{"type": "text", "text": "/deploy staging"}]}})
        ),
        json!({"type": "system", "subtype": "init", "cwd": "/tmp/selftest", "session_id": "51e1f7e5-0000-4000-8000-0000000000aa",
               "tools": ["Agent", "Bash", "Edit", "Read", "Write"], "mcp_servers": [], "model": "claude-opus-5-5",
               "permissionMode": "default", "slash_commands": ["compact", "deploy", "model"], "apiKeySource": "none",
               "claude_code_version": "2.1.319", "output_style": "default", "agents": ["general-purpose"], "skills": [],
               "plugins": [], "capabilities": [], "effort": "xhigh", "uuid": "51e1f7e5-0000-4000-8000-000000000002"})
        .to_string(),
        json!({"type": "assistant", "message": {"model": "claude-opus-5-5", "id": "msg_selftest_md", "type": "message",
               "role": "assistant", "content": [
                   {"type": "thinking", "thinking": "The user wants the staging deploy; check the build first.", "signature": "s"},
                   {"type": "text", "text": answer}],
               "stop_reason": null, "usage": {"input_tokens": 10, "output_tokens": 200}},
               "parent_tool_use_id": null, "session_id": "51e1f7e5-0000-4000-8000-0000000000aa",
               "uuid": "51e1f7e5-0000-4000-8000-000000000003"})
        .to_string(),
        json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 2100, "duration_api_ms": 1900,
               "num_turns": 1, "result": "ok", "stop_reason": "end_turn", "session_id": "51e1f7e5-0000-4000-8000-0000000000aa",
               "total_cost_usd": 0.0412, "usage": {"input_tokens": 1200, "cache_creation_input_tokens": 0,
               "cache_read_input_tokens": 40000, "output_tokens": 340},
               "modelUsage": {"claude-opus-5-5": {"inputTokens": 1200, "outputTokens": 340, "cacheReadInputTokens": 40000,
               "cacheCreationInputTokens": 0, "webSearchRequests": 0, "costUSD": 0.0412, "contextWindow": 200000,
               "maxOutputTokens": 64000}}, "permission_denials": [], "terminal_reason": "completed",
               "uuid": "51e1f7e5-0000-4000-8000-000000000004"})
        .to_string(),
    ]
    .join("\n")
}

/// A thousand entries, half of them user messages, as a long session would collect them.
fn long_session() -> String {
    let mut lines = Vec::new();
    for n in 0..500 {
        lines.push(format!(
            ">> {}",
            json!({"type": "user", "uuid": format!("51e1f7e5-1000-4000-8000-{n:012}"), "origin": {"kind": "human"},
                   "message": {"role": "user", "content": [{"type": "text", "text": format!("Question {n}: what does step {n} do?")}]}})
        ));
        lines.push(
            json!({"type": "assistant", "message": {"model": "claude-opus-5-5", "id": format!("msg_long_{n}"), "type": "message",
                   "role": "assistant", "content": [{"type": "text", "text": format!("Step **{n}** writes `out/{n}.txt` and checks it.\n\n- one\n- two")}],
                   "stop_reason": null, "usage": {"input_tokens": 1, "output_tokens": 1}},
                   "parent_tool_use_id": null, "session_id": "long", "uuid": format!("51e1f7e5-2000-4000-8000-{n:012}")})
            .to_string(),
        );
    }
    lines.join("\n")
}

fn folder() -> PathBuf {
    let folder = ide_core::paths::home().join("agent-project");
    for (path, text) in [("src/main.rs", "fn main() {}\n"), ("README.md", "# Agent\n")] {
        let file = folder.join(path);
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&folder));
        let _ = std::fs::write(file, text);
    }
    folder.canonicalize().unwrap_or(folder)
}

pub(super) fn tab_of(key: u64) -> Option<Rc<AgentTab>> {
    crate::agent::all().into_iter().find(|t| t.key == key)
}

pub(super) fn controller_of(root: &PathBuf) -> Option<objc2::rc::Retained<WindowController>> {
    window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(root))
}

/// Draws `view` on the window background into `<name>.png`, for views outside the window such as a popover.
pub(super) fn view_picture(view: &objc2_app_kit::NSView, name: &str) {
    let Some(path) = super::picture_path(name) else {
        return;
    };
    view.layoutSubtreeIfNeeded();
    let bounds = view.bounds();
    let Some(bitmap) = view.bitmapImageRepForCachingDisplayInRect(bounds) else {
        return;
    };
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
    // SAFETY: An empty properties dictionary is valid for PNG.
    let data = unsafe {
        bitmap.representationUsingType_properties(
            objc2_app_kit::NSBitmapImageFileType::PNG,
            &objc2_foundation::NSDictionary::new(),
        )
    };
    if let Some(data) = data {
        let _ = std::fs::write(path, data.to_vec());
    }
}

pub(super) fn appearance(controller: &WindowController, name: &objc2_app_kit::NSAppearanceName) {
    if let Some(window) = controller.window() {
        window.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
    }
}

/// An answer to `get_usage` as the CLI gives it, written by hand.
fn usage_answer() -> serde_json::Value {
    json!({
        "session": {"total_cost_usd": 0.42},
        "subscription_type": "max",
        "rate_limits": {
            "five_hour": {"utilization": 42.0, "resets_at": "2026-11-09T22:00:00Z"},
            "seven_day": {"utilization": 79.0, "resets_at": "2026-11-12T03:00:00Z"}
        }
    })
}

/// Hooks of one turn: three that ran fine and one that failed.
fn hook_frames() -> String {
    let mut lines: Vec<String> = HOOKS.lines().take(4).map(str::to_string).collect();
    for n in 3..5 {
        lines.push(
            json!({"type": "system", "subtype": "hook_response", "hook_id": format!("h{n}"),
                   "hook_name": "PostToolUse:Write", "hook_event": "PostToolUse", "output": "formatted",
                   "stdout": "formatted", "stderr": "", "exit_code": 0, "outcome": "success",
                   "session_id": "11111111-1111-4111-8111-111111111111",
                   "uuid": format!("b8000000-0000-4000-8000-00000000010{n}")})
            .to_string(),
        );
    }
    lines.join("\n")
}

pub(super) fn picture(controller: &WindowController, name: &str) {
    if let Some(window) = controller.window() {
        window.displayIfNeeded();
        snapshot(&window, name);
    }
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = folder();
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    let Some(controller) = controller_of(&root) else {
        check(
            "the trusted folder gets a window",
            false,
            format!("{:?}", window::open_projects()),
        );
        finish(NAME);
        return;
    };
    check("a trusted folder does not ask again", !controller.has_sheet(), "");
    let before = controller.window().map(|w| w.frame());
    let Some(tab) = crate::agent::open_replay(&controller) else {
        check("an AI tab opens", false, "");
        finish(NAME);
        return;
    };
    let after = controller.window().map(|w| {
        w.layoutIfNeeded();
        w.frame()
    });
    check(
        "an AI tab opens in a window of the same size",
        before
            .zip(after)
            .is_some_and(|(a, b)| same(a.size.width, b.size.width) && same(a.size.height, b.size.height)),
        format!("{before:?} → {after:?}"),
    );
    follows_window(&controller, &tab);
    let seen = tab.inspect();
    check(
        "an AI tab opens empty with its composer",
        seen.rows == 0 && seen.composer,
        format!("{} rows", seen.rows),
    );
    let hits = tab.composer_clicks();
    check(
        "a click on the empty composer, its placeholder or its margin puts the keyboard focus into its text",
        !hits.is_empty() && hits.iter().all(|&hit| hit),
        format!("{hits:?}"),
    );
    check(
        "the composer has the model and the mode pill",
        seen.pills.len() == 2 && seen.pills.iter().all(|p| !p.is_empty()),
        seen.pills.join(" | "),
    );
    picture(&controller, "agent-empty");
    let key = tab.key;
    main_after(Duration::from_millis(300), move || permission(root, key));
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0
}

/// Widens and narrows the window of `tab`: the sidebar keeps its width, the AI tab takes the change and its column
/// fills it up to its widest.
fn follows_window(controller: &WindowController, tab: &AgentTab) {
    let Some(window) = controller.window() else { return };
    let start = window.frame();
    let measure = |width: f64| {
        let frame = NSRect::new(start.origin, NSSize::new(width, start.size.height));
        window.setFrame_display(frame, true);
        window.layoutIfNeeded();
        (
            window.frame().size.width,
            controller.inspect().sidebar_width,
            tab.root().frame().size.width,
            tab.column_width(),
        )
    };
    let first = measure(start.size.width);
    let (_, sidebar, root, _) = first;
    let wider = measure(start.size.width + 400.0);
    let narrower = measure(760.0);
    window.setFrame_display(start, true);
    check(
        "a wider or narrower window changes the AI tab, the sidebar keeps its width",
        same(wider.0, start.size.width + 400.0)
            && same(narrower.0, 760.0)
            && same(wider.1, sidebar)
            && same(narrower.1, sidebar)
            && same(wider.2, root + 400.0)
            && same(narrower.2, root - (start.size.width - 760.0)),
        format!("{first:?} in {start:?}; wider {wider:?}, narrower {narrower:?}"),
    );
    let fills = |(_, _, root, column): (f64, f64, f64, f64)| same(column, root.min(crate::agent::MAX_COLUMN));
    check(
        "the conversation's column fills the AI tab up to its widest",
        fills(first) && fills(wider) && fills(narrower),
        format!("{first:?}, wider {wider:?}, narrower {narrower:?}"),
    );
}

/// A rich answer, file changes, and a permission that waits.
fn permission(root: PathBuf, key: u64) {
    let (Some(_), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    tab.feed(&markdown_turn());
    tab.feed(EDIT);
    let (before, _) = split(SUBAGENT, 56);
    tab.feed(&before);
    let seen = tab.inspect();
    check(
        "the timeline shows the recorded entries",
        seen.rows >= 12,
        format!("{} rows, title {:?}", seen.rows, seen.title),
    );
    check(
        "a permission request waits with its card",
        tab.snapshot().pending_requests().len() == 1,
        format!("{} pending", tab.snapshot().pending_requests().len()),
    );
    tab.timeline().scroll_to_end();
    let key = tab.key;
    main_after(Duration::from_millis(300), move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-permission");
        }
        running(root, key);
    });
}

/// A subagent and a background shell that run.
fn running(root: PathBuf, key: u64) {
    let (Some(_), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let (_, rest) = split(SUBAGENT, 56);
    let (to_running, _) = split(&rest, 174 - 56);
    tab.feed(&to_running);
    tab.open_all(crate::agent::Part::Subagent);
    tab.timeline().scroll_to_end();
    let seen = tab.inspect();
    check(
        "the subagent shows with its card",
        seen.subagents.iter().any(|s| s == "list files") && seen.visible.iter().any(|v| v.contains("Agent")),
        format!("{:?} / visible {:?}", seen.subagents, seen.visible),
    );
    check(
        "the background shell is listed",
        seen.background.iter().any(|b| b.contains("30-tick")),
        format!("{:?}", seen.background),
    );
    check(
        "only the rows in view have cards",
        seen.built < seen.rows * 3,
        format!("{} cards built for {} rows", seen.built, seen.rows),
    );
    check(
        "on a subscription the status line shows the plan's limits instead of dollars",
        tab.snapshot().usage().subscription
            && seen.status.iter().any(|s| s.contains(&crate::agent::percent(0.79)))
            && !seen.status.iter().any(|s| s.contains('$')),
        seen.status.join(" | "),
    );
    let key = tab.key;
    main_after(Duration::from_millis(400), move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-running");
        }
        activity(root, key);
    });
}

fn activity(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let (lines, badge) = crate::activity::inspect(&controller);
    check(
        "the activity popover lists the background shell",
        lines.iter().any(|l| l.contains("30-tick")),
        lines.join(" | "),
    );
    check(
        "the activity item counts what runs",
        !badge.is_empty() && badge != "0",
        badge,
    );
    let key = tab.key;
    main_after(Duration::from_millis(500), move || {
        if let Some(controller) = controller_of(&root) {
            if let Some(view) = crate::activity::popover_view(&controller) {
                view_picture(&view, "agent-activity");
            }
            crate::activity::close(&controller);
        }
        panel(root, key);
    });
}

fn panel(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    tab.timeline().table().scrollRowToVisible(0);
    controller.activate_panel(Panel::Agents);
    if let Ok(report) = <ide_claude::control::UsageReport as ide_claude::control::Answer>::read(usage_answer()) {
        crate::agent_panel::set_usage_report(report);
    }
    let seen = crate::agent_panel::inspect(&controller);
    let sessions = seen.as_ref().map(|s| s.sessions.clone()).unwrap_or_default();
    let running = seen.as_ref().map(|s| s.running.clone()).unwrap_or_default();
    check(
        "the AI panel lists the session",
        !sessions.is_empty(),
        sessions.join(" | "),
    );
    let active = seen.as_ref().map(|s| s.active.clone()).unwrap_or_default();
    check("the AI panel counts the live sessions", active.contains('0'), active);
    check(
        "the AI panel shows what runs",
        running
            .iter()
            .any(|r| r.contains("list files") || r.contains("30-tick")),
        running.join(" | "),
    );
    let usage = seen.map(|s| s.usage.join(" | ")).unwrap_or_default();
    check(
        "the AI panel shows the plan's usage from get_usage",
        usage.contains("42") && usage.contains("79") && usage.contains('%'),
        usage,
    );
    let key = tab.key;
    main_after(Duration::from_millis(600), move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-panel");
        }
        dark(root, key);
    });
}

fn dark(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let (_, rest) = split(SUBAGENT, 174);
    tab.feed(&rest);
    tab.feed(LATE);
    tab.timeline().scroll_to_end();
    if let Some(window) = controller.window() {
        window.setAppearance(NSAppearance::appearanceNamed(constant!(NSAppearanceNameDarkAqua)).as_deref());
    }
    let key = tab.key;
    main_after(Duration::from_millis(500), move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        picture(&controller, "agent-dark");
        appearance(
            &controller,
            constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua),
        );
        main_after(Duration::from_millis(400), move || {
            let Some(controller) = controller_of(&root) else {
                return finish(NAME);
            };
            picture(&controller, "agent-contrast-dark");
            appearance(&controller, constant!(NSAppearanceNameAccessibilityHighContrastAqua));
            main_after(Duration::from_millis(400), move || {
                let Some(controller) = controller_of(&root) else {
                    return finish(NAME);
                };
                picture(&controller, "agent-contrast");
                appearance(&controller, constant!(NSAppearanceNameAqua));
                controller.activate_panel(Panel::Files);
                requests(root, key);
            });
        });
    });
}

/// Questions, plans, to-dos and status notices in a second tab.
fn requests(root: PathBuf, first: u64) {
    let (Some(controller), Some(first)) = (controller_of(&root), tab_of(first)) else {
        return finish(NAME);
    };
    let Some(tab) = crate::agent::open_replay(&controller) else {
        return finish(NAME);
    };
    tab.feed(COMPACTION);
    tab.feed(RETRY);
    let (asked, _) = until(QUESTION, "\"subtype\":\"can_use_tool\"");
    tab.feed(&asked);
    let (plan, _) = until(PLAN, "\"request_id\":\"perm-plan\"");
    tab.feed(&plan);
    tab.timeline().scroll_to_end();
    let pending = tab.snapshot().pending_requests().len();
    check(
        "a question and a plan wait for answers",
        pending >= 2,
        format!("{pending} pending"),
    );
    let key = tab.key;
    main_after(Duration::from_millis(300), move || {
        if let Some(tab) = tab_of(key) {
            let misfits = tab.timeline().misfits();
            check(
                "every row is as tall as its card",
                misfits.is_empty(),
                misfits.join(" | "),
            );
        }
    });
    check(
        "two AI tabs are open in the window",
        crate::agent::of_window(&controller).len() == 2,
        format!("{}", crate::agent::of_window(&controller).len()),
    );
    first.feed(RETRY);
    let first = first.key;
    main_after(Duration::from_millis(400), move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-requests");
        }
        super::agent_plan::run(root, key, first, context);
    });
}

/// An Edit permission with its content editable, the hooks of a turn on one line, and the focus view.
pub(super) fn context(root: PathBuf, key: u64, first: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let (edit, _) = until(EDIT, "\"subtype\":\"can_use_tool\"");
    tab.feed(&edit);
    tab.feed(&hook_frames());
    tab.timeline().scroll_to_end();
    main_after(Duration::from_millis(400), move || {
        let Some(tab) = tab_of(key) else { return finish(NAME) };
        let seen = tab.inspect();
        check(
            "the hooks of a turn that ran fine share one line",
            seen.visible.iter().any(|v| v == "Hooks: 3 erfolgreich")
                && !seen.visible.iter().any(|v| v.contains("PostToolUse:Write")),
            seen.visible.join(" | "),
        );
        let editors = tab.timeline().editor_texts();
        check(
            "a Write permission lets the user change the content first",
            editors.iter().any(|t| t == "hello world\n"),
            format!("{editors:?}"),
        );
        let all = seen.rows;
        tab.toggle_focus_view();
        let focused = tab.inspect().rows;
        check(
            "the focus view shows only the conversation",
            focused > 0 && focused < all,
            format!("{focused} of {all} rows"),
        );
        main_after(Duration::from_millis(300), move || {
            if let Some(controller) = controller_of(&root) {
                picture(&controller, "agent-focus");
            }
            if let Some(tab) = tab_of(key) {
                tab.toggle_focus_view();
                tab.type_in_composer("schau dir @REA");
            }
            main_after(Duration::from_millis(800), move || mentions(root, key, first));
        });
    });
}

/// Completing `@` mentions, the editor's selection as context, and a terminal for `@terminal`.
fn mentions(root: PathBuf, key: u64, first: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let labels = tab.suggestion_labels();
    check(
        "typing @ offers the project's files",
        labels.iter().any(|l| l == "README.md"),
        labels.join(", "),
    );
    tab.submit();
    tab.type_in_composer(&format!("{}@ter", tab.composer_text()));
    let labels = tab.suggestion_labels();
    tab.submit();
    check(
        "a chosen completion becomes a mention",
        tab.composer_text() == "schau dir @README.md @terminal ",
        format!("{:?} after {}", tab.composer_text(), labels.join(", ")),
    );
    let readme = root.join("README.md");
    controller.open_file(&readme);
    if let Some(editor) = crate::editor::editor_for(&controller, &readme) {
        editor.setSelectedRange(objc2_foundation::NSRange::new(0, 7));
    }
    tab.select();
    tab.focus_composer();
    check("the editor's selection is offered as context", tab.has_offer(), "");
    tab.use_selection();
    let names = tab.attachment_names();
    check(
        "the offered selection becomes an attachment",
        names.iter().any(|n| n.ends_with("README.md")),
        names.join(", "),
    );
    crate::terminal::open_tab(&controller, Some(&root));
    tab.select();
    main_after(Duration::from_millis(400), move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-context");
        }
        main_after(Duration::from_millis(2500), move || terminal(root, first));
    });
}

/// The terminal's output for `@terminal`, its tab for its process, and a session's own task overview.
fn terminal(root: PathBuf, first: u64) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let shells: Vec<u32> = ide_process::Supervisor::global()
        .running()
        .into_iter()
        .filter(|p| p.kind == ide_process::Kind::Terminal)
        .map(|p| p.pid)
        .collect();
    let tab = shells
        .iter()
        .find_map(|pid| crate::terminal::tab_of_process(&controller, *pid));
    check(
        "a terminal's process leads to its tab",
        tab.is_some(),
        format!("{shells:?} → {tab:?}"),
    );
    let (lines, _) = crate::activity::inspect(&controller);
    crate::activity::close(&controller);
    check(
        "the activity view shows a terminal's tab",
        lines
            .iter()
            .any(|l| l.contains(kind_terminal()) && l.contains("Zeigen")),
        lines.join(" | "),
    );
    check(
        "@terminal hands over the terminal's output",
        crate::agent::terminal_context(&controller),
        "",
    );
    let Some(first) = tab_of(first) else {
        return finish(NAME);
    };
    first.select();
    first.show_tasks(None);
    let overview = crate::activity::overview_lines(&controller);
    check(
        "the session's overview lists its subagents and tasks",
        overview.iter().any(|l| l.contains("list files")) && overview.iter().any(|l| l.contains("review code")),
        overview.join(" | "),
    );
    main_after(Duration::from_millis(500), move || {
        if let Some(controller) = controller_of(&root) {
            if let Some(view) = crate::activity::popover_view(&controller) {
                view_picture(&view, "agent-tasks");
            }
            crate::activity::close(&controller);
        }
        browser(root);
    });
}

const PAGE_TITLE: &str = "Fixture page";
const PAGE_SELECTION: &str = "words picked on the page";

/// Serves one page on this Mac whose text is selected once it loads; its address.
fn page_server() -> Option<String> {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    let page = format!(
        "<!doctype html><title>{PAGE_TITLE}</title><p id=p>{PAGE_SELECTION}</p><script>\
         const r = document.createRange(); r.selectNodeContents(document.getElementById('p'));\
         getSelection().removeAllRanges(); getSelection().addRange(r);</script>"
    );
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                page.len()
            );
            let _ = stream
                .write_all(head.as_bytes())
                .and_then(|()| stream.write_all(page.as_bytes()));
        }
    });
    Some(format!("http://127.0.0.1:{port}/"))
}

/// `@browser` hands over the address, title and selected text of the page in view.
fn browser(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let Some(url) = page_server() else {
        check("the page server starts", false, "");
        return rewind(root);
    };
    crate::browser::open_tab(&controller, Some(&url));
    main_after(Duration::from_millis(2500), move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        crate::agent::browser_context(&controller, move |text| {
            let text = text.unwrap_or_default();
            check(
                "@browser hands over the page's address, title and selection",
                text.contains(&url) && text.contains(PAGE_TITLE) && text.contains(PAGE_SELECTION),
                text.replace('\n', " | "),
            );
            super::agent_outside::run(root, rewind);
        });
    });
}

fn kind_terminal() -> &'static str {
    splat_i18n::t("Terminal")
}

/// Rewinding the conversation to just before the second prompt of a recorded session.
fn rewind(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let Some(tab) = crate::agent::open_replay(&controller) else {
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
    main_after(Duration::from_millis(400), move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-rewind");
        }
        super::agent_context::run(root, long);
    });
}

/// A thousand entries: built and scrolled without building a card for each (E25).
fn long(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let Some(tab) = crate::agent::open_replay(&controller) else {
        return finish(NAME);
    };
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
    check(
        "a finished session would notify while unseen",
        crate::agent::notifications_posted() > 0,
        format!("{}", crate::agent::notifications_posted()),
    );
    finish(NAME);
}

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
