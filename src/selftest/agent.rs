//! The `agent` scenario: an AI tab fed by recordings of real Claude sessions, without a process or the network.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAccessibilityHighContrastAqua,
    NSAppearanceNameAccessibilityHighContrastDarkAqua, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
};
use objc2_foundation::{NSRect, NSSize};
use serde_json::json;

use super::{check, finish, helpers, snapshot};
use crate::activity::Part;
use crate::util::constant;
use crate::window::{self, Panel, WindowController};
use ide_ui_agent::{AgentKey, AgentTab};

const SUBAGENT: &str = include_str!("../../crates/ide-claude/tests/fixtures/recorded_subagent_background.ndjson");
const EDIT: &str = include_str!("../../crates/ide-claude/tests/fixtures/recorded_edit_diff.ndjson");
pub(super) const QUESTION: &str = include_str!("../../crates/ide-claude/tests/fixtures/recorded_question.ndjson");
const PLAN: &str = include_str!("../../crates/ide-claude/tests/fixtures/plan_todos.ndjson");
const COMPACTION: &str = include_str!("../../crates/ide-claude/tests/fixtures/compaction.ndjson");
const RETRY: &str = include_str!("../../crates/ide-claude/tests/fixtures/api_retry.ndjson");
const LATE: &str = include_str!("../../crates/ide-claude/tests/fixtures/late_runs.ndjson");
const HOOKS: &str = include_str!("../../crates/ide-claude/tests/fixtures/hooks.ndjson");

pub(super) const NAME: &str = "agent";

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
pub(super) fn long_session() -> String {
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

/// Time for the window to draw what changed before its picture is taken; nothing reports when drawing is done.
pub(super) const DRAWN: Duration = Duration::from_millis(400);

pub(super) fn tab_of(key: AgentKey) -> Option<Rc<AgentTab>> {
    ide_ui_agent::all().into_iter().find(|t| t.key == key)
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
    let files = [("src/main.rs", "fn main() {}\n"), ("README.md", "# Agent\n")];
    helpers::open_project(NAME, "agent-project", &files, opened);
}

fn opened(controller: objc2::rc::Retained<WindowController>) {
    let root = helpers::root("agent-project");
    check("a trusted folder does not ask again", !controller.has_sheet(), "");
    let before = controller.window().map(|w| w.frame());
    let Some(tab) = ide_ui_agent::open_replay(&controller) else {
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
    long_title(&tab);
    let seen = tab.inspect();
    check(
        "an AI tab opens empty with its composer",
        seen.rows == 0 && seen.composer,
        format!("{} rows", seen.rows),
    );
    tab.use_mode("bypassPermissions");
    let extras = tab.foot_extras();
    tab.use_mode("default");
    check(
        "Umgehen shows no warning box at the composer",
        extras
            .as_ref()
            .is_some_and(|views| views.iter().all(|(_, height)| *height < 1.0)),
        format!("{extras:?}"),
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
    subagent_asks(&controller, root.clone(), move || sidebar_let_go(root, key));
}

/// The AI tab comes to the front and another one right after, before the sidebar the first got held at is let go, as
/// when AppKit gives the window of a closing menu the focus back for a moment: the AI tab's sidebar is free to drag
/// all the same.
fn sidebar_let_go(root: PathBuf, key: AgentKey) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let ai = controller.tab_with_key(&tab.tab_key());
    let other = controller.tab_ids().into_iter().find(|id| Some(*id) != ai);
    if let (Some(ai), Some(other)) = (ai, other) {
        controller.select_tab_by_id(ai);
        controller.select_tab_by_id(other);
    }
    let held = root.clone();
    helpers::wait_until(
        move || {
            controller_of(&held)
                .and_then(|c| ai.and_then(|id| c.sidebar_range_of(id)))
                .is_some_and(|(min, max)| max - min > 1.0)
        },
        move |_| {
            let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
                return finish(NAME);
            };
            let range = ai.and_then(|id| controller.sidebar_range_of(id));
            check(
                "an AI tab that was in front only for a moment leaves its sidebar free to drag",
                other.is_some() && range.is_some_and(|(min, max)| max - min > 1.0),
                format!("{range:?}"),
            );
            tab.select();
            helpers::after(DRAWN, move || permission(root, key));
        },
    );
}

/// Whether the tab `key` shows one request on its last card, ready to answer, every row as tall as its card.
fn asks_on_card(key: AgentKey) -> bool {
    tab_of(key).is_some_and(|tab| {
        tab.snapshot().pending_requests().len() == 1
            && tab
                .timeline()
                .last_card_buttons()
                .iter()
                .any(|b| b == splat_i18n::t("Einmal erlauben"))
            && tab.timeline().misfits().is_empty()
    })
}

/// A subagent whose tool waits for permission, its timeline folded: the request shows on its card all the same.
fn subagent_asks(controller: &WindowController, root: PathBuf, then: impl FnOnce() + Send + 'static) {
    let Some(tab) = ide_ui_agent::open_replay(controller) else {
        check("a second AI tab opens", false, "");
        return then();
    };
    let session = "51e1f7e5-0000-4000-8000-0000000000ee";
    let assistant = |id: &str, parent: Option<&str>, content: serde_json::Value| {
        json!({"type": "assistant", "message": {"model": "claude-opus-5-5", "id": id, "type": "message", "role": "assistant",
               "content": content, "stop_reason": null, "usage": {"input_tokens": 10, "output_tokens": 20}},
               "parent_tool_use_id": parent, "session_id": session, "uuid": format!("{id}-uuid")})
        .to_string()
    };
    let lines = [
        assistant(
            "msg_sub_1",
            None,
            json!([{"type": "tool_use", "id": "toolu_sub", "name": "Agent",
                    "input": {"description": "Wetter holen", "prompt": "Fetch the weather", "subagent_type": "general-purpose"}}]),
        ),
        assistant(
            "msg_sub_2",
            Some("toolu_sub"),
            json!([{"type": "tool_use", "id": "toolu_fetch", "name": "WebFetch",
                    "input": {"url": "https://example.org/weather", "prompt": "Read the weather"}}]),
        ),
        json!({"type": "control_request", "request_id": "perm-sub", "request": {"subtype": "can_use_tool",
               "tool_name": "WebFetch", "input": {"url": "https://example.org/weather", "prompt": "Read the weather"},
               "permission_suggestions": [], "tool_use_id": "toolu_fetch"}})
        .to_string(),
    ];
    tab.feed(&lines.join("\n"));
    let other = tab.key;
    helpers::wait_until(
        move || asks_on_card(other),
        move |_| {
            let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(other)) else {
                return then();
            };
            let buttons = tab.timeline().last_card_buttons();
            let misfits = tab.timeline().misfits();
            let dock = objc2::MainThreadMarker::new().and_then(|mtm| {
                objc2_app_kit::NSApplication::sharedApplication(mtm)
                    .dockTile()
                    .badgeLabel()
            });
            check(
                "the Dock icon shows no number, also while a session waits",
                dock.is_none(),
                format!("{dock:?}"),
            );
            let mark = tab.tab_mark();
            check(
                "an AI tab that waits for the user has a red dot",
                mark.as_deref() == Some(&*objc2_app_kit::NSColor::systemRedColor()),
                format!("{mark:?}"),
            );
            check(
                "a subagent's request that waits shows on its folded card, ready to answer, the row as tall as the card",
                tab.snapshot().pending_requests().len() == 1
                    && buttons.iter().any(|b| b == splat_i18n::t("Einmal erlauben"))
                    && misfits.is_empty(),
                format!("{buttons:?}, misfits {misfits:?}"),
            );
            picture(&controller, "agent-subagent-asks");
            let progress = |n: u64, summary: &str| {
                json!({"type": "system", "subtype": "task_progress", "task_id": "task_sub", "tool_use_id": "toolu_sub",
                   "description": "Wetter holen", "usage": {"total_tokens": 1000 * n, "tool_uses": n, "duration_ms": 900 * n},
                   "last_tool_name": "WebFetch", "summary": summary, "session_id": session, "uuid": format!("progress-{n}")})
            .to_string()
            };
            let started = json!({"type": "system", "subtype": "task_started", "task_id": "task_sub", "tool_use_id": "toolu_sub",
                             "description": "Wetter holen", "subagent_type": "general-purpose", "task_type": "local_agent",
                             "session_id": session, "uuid": "task-started"});
            tab.feed(&[started.to_string(), progress(1, "Reading the forecast")].join("\n"));
            tab.timeline().settle();
            tab.feed(&progress(
                2,
                &"Comparing the forecasts of three weather services for the next days. ".repeat(6),
            ));
            let grown = tab.timeline().misfits();
            tab.timeline().settle();
            tab.feed(&progress(3, "Writing the report"));
            let shrunk = tab.timeline().misfits();
            check(
                "a running subagent's card that grows or shrinks with its progress has its row's height before the window \
             draws again",
                grown.is_empty() && shrunk.is_empty(),
                format!("grown {grown:?}, shrunk {shrunk:?}"),
            );
            if let Some(id) = controller.tab_with_key(&tab.tab_key()) {
                controller.close_tab(id);
            }
            then();
        },
    );
}

/// A title wider than the head takes the room up to the buttons and is cut there, not after a count of characters.
fn long_title(tab: &AgentTab) {
    let title = ide_thread::title_from(&"könnten wir hier eine Vorschau einbauen ".repeat(8));
    let (shown, whole, head) = tab.title_widths(&title);
    check(
        "a long session title fills the head up to its buttons and is cut at its width",
        title.ends_with('…') && shown < whole && shown > head * 0.5,
        format!("title {shown:.0} of {whole:.0} pt in a head of {head:.0} pt"),
    );
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0
}

/// Widens and narrows the window of `tab`: the sidebar keeps its width, the AI tab takes the change and its column
/// fills it but for its margin.
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
            tab.composer_place(),
        )
    };
    let first = measure(start.size.width);
    let (_, sidebar, root, _, _) = first;
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
    type Seen = (f64, f64, f64, f64, (f64, f64));
    let fills = |(_, _, root, column, _): Seen| same(column, root - 2.0 * ide_ui_agent::SIDE_MARGIN);
    check(
        "the conversation's column grows with the AI tab, with the same margin at any width",
        fills(first) && fills(wider) && fills(narrower),
        format!("{first:?}, wider {wider:?}, narrower {narrower:?}"),
    );
    let held = |(_, _, _, column, (width, off)): Seen| {
        let wanted = (column - 2.0 * ide_ui_agent::INPUT_INSET).min(ide_ui_agent::INPUT_WIDTH);
        same(width, wanted) && same(off, 0.0)
    };
    check(
        "the composer fills a narrow AI tab and keeps its widest in the middle of a wide one",
        held(first) && held(wider) && held(narrower),
        format!("{:?}, wider {:?}, narrower {:?}", first.4, wider.4, narrower.4),
    );
}

/// A rich answer, file changes, and a permission that waits.
fn permission(root: PathBuf, key: AgentKey) {
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
    scroller_rests_at_the_end(&tab);
    let key = tab.key;
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-permission");
        }
        running(root, key);
    });
}

fn scroller_rests_at_the_end(tab: &AgentTab) {
    let scroll = tab.timeline().view();
    if scroll.scrollerStyle() != objc2_app_kit::NSScrollerStyle::Overlay {
        return;
    }
    let at_end = scroll.hasVerticalScroller();
    let clip = scroll.contentView();
    clip.scrollToPoint(objc2_foundation::NSPoint::new(0.0, 0.0));
    scroll.reflectScrolledClipView(&clip);
    let away = scroll.hasVerticalScroller();
    tab.timeline().scroll_to_end();
    check(
        "following the end does not flash the scroller, scrolling away brings it back",
        !at_end && away,
        format!("at the end {at_end}, away {away}"),
    );
}

/// A subagent and a background shell that run.
fn running(root: PathBuf, key: AgentKey) {
    let (Some(_), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let (_, rest) = split(SUBAGENT, 56);
    let (to_running, _) = split(&rest, 174 - 56);
    tab.feed(&to_running);
    tab.open_all(ide_ui_agent::Part::Subagent);
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
    for (part, wanted) in [
        (Part::Subagents, "Subagenten: 1 laufend, 0 beendet"),
        (Part::Background, "Hintergrundprozesse: 1 laufend, 0 beendet"),
    ] {
        let (tip, enabled, accent) = tab.work_button(part);
        check(
            "the head's button counts what runs in its tooltip and shows the accent colour",
            tip == wanted && enabled && accent,
            format!("{tip:?}, enabled {enabled}, accent {accent}"),
        );
    }
    check(
        "only the rows in view have cards",
        seen.built < seen.rows * 3,
        format!("{} cards built for {} rows", seen.built, seen.rows),
    );
    check(
        "on a subscription the status line shows the plan's limits instead of dollars",
        tab.snapshot().usage().subscription
            && seen.status.iter().any(|s| s.contains(&ide_ui_agent::percent(0.79)))
            && !seen.status.iter().any(|s| s.contains('$')),
        seen.status.join(" | "),
    );
    let key = tab.key;
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-running");
        }
        activity(root, key);
    });
}

fn activity(root: PathBuf, key: AgentKey) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let (lines, tooltip) = crate::activity::inspect(&controller);
    check(
        "the activity popover lists the background shell",
        lines.iter().any(|l| l.contains("30-tick")),
        lines.join(" | "),
    );
    check(
        "the activity item counts what runs in its tooltip",
        tooltip
            .rsplit(' ')
            .next()
            .is_some_and(|n| n.parse::<usize>().is_ok_and(|n| n > 0)),
        tooltip,
    );
    let key = tab.key;
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            if let Some(view) = crate::activity::popover_view(&controller) {
                view_picture(&view, "agent-activity");
            }
            crate::activity::close(&controller);
        }
        panel(root, key);
    });
}

fn panel(root: PathBuf, key: AgentKey) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    tab.timeline().table().scrollRowToVisible(0);
    controller.activate_panel(Panel::Agents);
    if let Ok(report) = <ide_claude::control::UsageReport as ide_claude::control::Answer>::read(usage_answer()) {
        ide_ui_agent::inject_usage(report);
    }
    let status = tab.inspect().status;
    let wanted = format!(
        "{} · {} {} · {} {}",
        splat_i18n::t("Abo"),
        ide_ui_agent::window_name("seven_day"),
        ide_ui_agent::percent(0.79),
        ide_ui_agent::window_name("five_hour"),
        ide_ui_agent::percent(0.42),
    );
    check(
        "the status line names the plan with its week and five hours from get_usage",
        status.last() == Some(&wanted),
        status.join(" | "),
    );
    let rows = ide_ui_agent::agent_panel::inspect(&controller)
        .map(|seen| seen.rows)
        .unwrap_or_default();
    check(
        "the AI panel lists the open session as an AI session, marked open, without its agent's name",
        rows.iter()
            .any(|(kind, open, title)| *kind == "AI" && *open && *title == tab.title()),
        format!("{rows:?}"),
    );
    let edges = ide_ui_agent::agent_panel::edges(&controller);
    let title = controller.panel_title_row();
    check(
        "the AI panel's title offers a new session, its list begins where the title does and ends where the button does",
        title.as_ref().is_some_and(|(_, row)| !row.buttons_start.is_nan())
            && edges.as_ref().zip(title.as_ref()).is_some_and(|(rows, (_, row))| {
                !rows.is_empty()
                    && rows.iter().all(|(start, end)| {
                        (start - row.title_start).abs() < 3.0 && (end - row.buttons_end).abs() < 3.0
                    })
            }),
        format!(
            "{:?}, rows at {edges:?}",
            title.map(|(name, row)| (name, row.title_start, row.buttons_end))
        ),
    );
    let key = tab.key;
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-panel");
        }
        dark(root, key);
    });
}

fn dark(root: PathBuf, key: AgentKey) {
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
    helpers::after(DRAWN, move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        picture(&controller, "agent-dark");
        appearance(
            &controller,
            constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua),
        );
        helpers::after(DRAWN, move || {
            let Some(controller) = controller_of(&root) else {
                return finish(NAME);
            };
            picture(&controller, "agent-contrast-dark");
            appearance(&controller, constant!(NSAppearanceNameAccessibilityHighContrastAqua));
            helpers::after(DRAWN, move || {
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
fn requests(root: PathBuf, first: AgentKey) {
    let (Some(controller), Some(first)) = (controller_of(&root), tab_of(first)) else {
        return finish(NAME);
    };
    let Some(tab) = ide_ui_agent::open_replay(&controller) else {
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
    let fitting = move || tab_of(key).is_some_and(|tab| tab.timeline().misfits().is_empty());
    helpers::wait_until(fitting, move |_| {
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
        ide_ui_agent::of_window(&controller).len() == 2,
        format!("{}", ide_ui_agent::of_window(&controller).len()),
    );
    first.feed(RETRY);
    let first = first.key;
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-requests");
        }
        super::agent_plan::run(root, key, first, context);
    });
}

/// An Edit permission with its content editable, the hooks of a turn on one line, and the focus view.
pub(super) fn context(root: PathBuf, key: AgentKey, first: AgentKey) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let (edit, _) = until(EDIT, "\"subtype\":\"can_use_tool\"");
    tab.feed(&edit);
    tab.feed(&hook_frames());
    tab.timeline().scroll_to_end();
    let shown = move || {
        tab_of(key).is_some_and(|tab| {
            tab.inspect().visible.iter().any(|v| v == "Hooks: 3 erfolgreich")
                && tab.timeline().editor_texts().iter().any(|t| t == "hello world\n")
        })
    };
    helpers::wait_until(shown, move |_| {
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
        helpers::after(DRAWN, move || {
            if let Some(controller) = controller_of(&root) {
                picture(&controller, "agent-focus");
            }
            if let Some(tab) = tab_of(key) {
                tab.toggle_focus_view();
                tab.type_in_composer("schau dir @REA");
            }
            let offered =
                move || tab_of(key).is_some_and(|tab| tab.suggestion_labels().iter().any(|l| l == "README.md"));
            helpers::wait_until(offered, move |_| mentions(root, key, first));
        });
    });
}

/// Completing `@` mentions, the editor's selection as context, and a terminal for `@terminal`.
fn mentions(root: PathBuf, key: AgentKey, first: AgentKey) {
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
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            picture(&controller, "agent-context");
        }
        let shell = root.clone();
        let output = move || controller_of(&shell).is_some_and(|c| ide_ui_agent::terminal_context(&c));
        helpers::wait_until(output, move |_| terminal(root, first));
    });
}

/// The terminal's output for `@terminal`, its tab for its process, and a session's own task overview.
fn terminal(root: PathBuf, first: AgentKey) {
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
        ide_ui_agent::terminal_context(&controller),
        "",
    );
    let Some(first) = tab_of(first) else {
        return finish(NAME);
    };
    first.select();
    first.show_work(Part::All, None);
    let overview = crate::activity::overview_lines(&controller);
    check(
        "the session's overview lists its subagents and tasks",
        overview.iter().any(|l| l.contains("list files")) && overview.iter().any(|l| l.contains("review code")),
        overview.join(" | "),
    );
    let key = first.key;
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            if let Some(view) = crate::activity::popover_view(&controller) {
                view_picture(&view, "agent-tasks");
            }
            crate::activity::close(&controller);
        }
        work_lists(root, key);
    });
}

/// The head's lists of the session's subagents and of its background processes, each of its own kind.
fn work_lists(root: PathBuf, key: AgentKey) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    tab.show_work(Part::Subagents, None);
    let subagents = crate::activity::overview_lines(&controller);
    check(
        "the subagent list holds the session's subagents and no shell",
        subagents.iter().any(|l| l.contains("list files")) && !subagents.iter().any(|l| l.contains("30-tick")),
        subagents.join(" | "),
    );
    crate::activity::close(&controller);
    tab.show_work(Part::Background, None);
    let background = crate::activity::overview_lines(&controller);
    check(
        "the background list keeps the stopped shell, as stopped, and no subagent",
        background
            .iter()
            .any(|l| l.contains("30-tick") && l.contains(splat_i18n::t("Gestoppt")))
            && !background.iter().any(|l| l.contains("list files")),
        background.join(" | "),
    );
    let (tip, enabled, accent) = tab.work_button(Part::Background);
    check(
        "the head's background button counts the ended shell and is calm",
        tip == "Hintergrundprozesse: 0 laufend, 1 beendet" && enabled && !accent,
        format!("{tip:?}, enabled {enabled}, accent {accent}"),
    );
    helpers::after(DRAWN, move || {
        if let Some(controller) = controller_of(&root) {
            if let Some(view) = crate::activity::popover_view(&controller) {
                view_picture(&view, "agent-background-list");
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
        return super::agent_rewind::rewind(root);
    };
    crate::browser::open_tab(&controller, Some(&url));
    let page = root.clone();
    let loaded = move || controller_of(&page).is_some_and(|c| c.tab_titles().iter().any(|t| t == PAGE_TITLE));
    helpers::wait_until(loaded, move |_| {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        ide_ui_agent::browser_context(&controller, move |text| {
            let text = text.unwrap_or_default();
            check(
                "@browser hands over the page's address, title and selection",
                text.contains(&url) && text.contains(PAGE_TITLE) && text.contains(PAGE_SELECTION),
                text.replace('\n', " | "),
            );
            super::agent_outside::run(root, super::agent_rewind::rewind);
        });
    });
}

fn kind_terminal() -> &'static str {
    splat_i18n::t("Terminal")
}
