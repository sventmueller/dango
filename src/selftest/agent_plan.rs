//! The plan step of the `agent` scenario: a plan in an AI tab of its own, commented on in its card and in its
//! document (comments added, edited, removed, their passages marked), sent back with each comment under the passage
//! it quotes, and the revised plan rejected. Pictures in light and dark.

use std::path::PathBuf;
use std::time::Duration;

use ide_claude::replay::SENT;
use ide_thread::{Decision, EntryKind, RequestState};
use objc2_app_kit::{NSAppearanceNameAqua, NSAppearanceNameDarkAqua};
use objc2_foundation::NSRange;
use serde_json::json;
use splat_i18n::t;

use super::agent::{appearance, controller_of, picture, tab_of, view_picture};
use super::{check, finish};
use crate::agent::AgentTab;
use crate::agent::plan::{self, PLAN_IN_CARD, PLAN_IN_DOC, PLAN_REJECT, PLAN_REVISE};
use crate::util::{constant, main_after};

const NAME: &str = "agent";
const SETTLE: Duration = Duration::from_millis(300);
const SESSION: &str = "22222222-2222-4222-8222-222222222222";

const PLAN: &str = "## Fix the dropped last line\n\nThe parser loses the last line of a file that does not end in \
                    a newline.\n\n1. Add a failing test for a file without a trailing newline\n2. Make \
                    `Lexer::next_line` return the rest of the input at the end\n3. Run the parser tests and the fuzz \
                    corpus\n\n**Risk:** the fuzz corpus takes about ten minutes.";

const REVISED: &str = "## Fix the dropped last line, revised\n\n1. Add `tests/no_newline.rs`, which reads \
                       `fixtures/no_newline.txt`\n2. Make the tokenizer end the input with its last line\n3. Run the \
                       parser tests";

/// What the step sends back: each comment under its passage, the comment on the whole plan last.
const FEEDBACK: &str = "> Add a failing test for a file without a trailing newline\n\nName the file the test reads\n\n\
                        > Lexer::next_line\n\nFix it in the tokenizer instead\n\nKeep the change small";

/// The requests tab and the first tab, which the scenario goes on with, and where.
type Next = (u64, u64, fn(PathBuf, u64, u64));

/// Plan `n` as Claude presents it, written from the SDK's typings: the tool call, then the request that waits.
fn plan_frames(n: u32, plan: &str) -> String {
    let call = format!("toolu_plan_{n}");
    let message = json!({"model": "claude-opus-5-5", "id": format!("msg_plan_{n}"), "type": "message",
        "role": "assistant", "stop_reason": null,
        "content": [{"type": "tool_use", "id": call, "name": "ExitPlanMode", "input": {"plan": plan}}]});
    let call_frame = json!({"type": "assistant", "message": message, "parent_tool_use_id": null,
        "session_id": SESSION, "uuid": format!("e7000000-0000-4000-8000-00000000000{n}")});
    let ask = json!({"type": "control_request", "request_id": format!("perm-plan-{n}"),
        "request": {"subtype": "can_use_tool", "tool_name": "ExitPlanMode", "input": {"plan": plan},
            "tool_use_id": call, "requires_user_interaction": true}});
    format!("{call_frame}\n{ask}")
}

/// The session's start and the prompt, then the first plan.
pub(super) fn start_frames() -> String {
    let init = json!({"type": "system", "subtype": "init", "cwd": "/tmp/vibeedit-fixture/plan", "session_id": SESSION,
        "tools": ["Bash", "ExitPlanMode"], "mcp_servers": [], "model": "claude-opus-5-5", "permissionMode": "plan",
        "slash_commands": [], "apiKeySource": "none", "claude_code_version": "2.1.319", "output_style": "default",
        "uuid": "e7000000-0000-4000-8000-000000000000"});
    let prompt = json!({"type": "user", "uuid": "e7000000-0000-4000-8000-0000000000a0",
        "message": {"role": "user", "content": [{"type": "text", "text": "Plan how to fix the dropped last line"}]}});
    format!("{init}\n{SENT}{prompt}\n{}", plan_frames(1, PLAN))
}

/// The CLI's result for plan `n` sent back with `feedback`, which comes back as the tool's error.
fn sent_back(n: u32, feedback: &str) -> String {
    json!({"type": "user", "message": {"role": "user", "content": [{"tool_use_id": format!("toolu_plan_{n}"),
        "type": "tool_result", "content": feedback, "is_error": true}]}, "parent_tool_use_id": null,
        "session_id": SESSION, "uuid": format!("e7000000-0000-4000-8000-0000000000b{n}")})
    .to_string()
}

fn entry_id(tab: &AgentTab, key: &str) -> Option<u64> {
    tab.snapshot().entry_by_key(key).map(|e| e.id.get())
}

fn plan_state(tab: &AgentTab, key: &str) -> Option<RequestState> {
    match &tab.snapshot().entry_by_key(key)?.kind {
        EntryKind::PlanApproval(plan) => Some(plan.state.clone()),
        _ => None,
    }
}

fn comments(tab: &AgentTab, id: u64) -> Vec<String> {
    let inputs = tab.timeline().inputs().borrow();
    inputs
        .plan_notes
        .get(&id)
        .map(|n| n.iter().map(|n| n.comment.clone()).collect())
        .unwrap_or_default()
}

/// The ids of plan `id`'s comments, in the order they are listed.
fn note_ids(tab: &AgentTab, id: u64) -> Vec<u64> {
    let inputs = tab.timeline().inputs().borrow();
    inputs
        .plan_notes
        .get(&id)
        .map(|n| n.iter().map(|n| n.id).collect())
        .unwrap_or_default()
}

/// Comments `comment` on what is marked in plan `id`'s card or document (`place`), through its popover.
fn comment_on(tab: &AgentTab, id: u64, place: u8, comment: &str) -> bool {
    tab.plan_comment(id, place);
    let typed = plan::type_comment(comment);
    tab.plan_commit(id);
    typed
}

pub fn run(root: PathBuf, requests: u64, first: u64, then: fn(PathBuf, u64, u64)) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let Some(tab) = crate::agent::open_replay(&controller) else {
        return finish(NAME);
    };
    tab.feed(&start_frames());
    tab.timeline().scroll_to_end();
    let waits = plan_state(&tab, "toolu_plan_1") == Some(RequestState::Pending);
    let Some(id) = entry_id(&tab, "toolu_plan_1").filter(|_| waits) else {
        check("the plan waits for approval", false, "");
        return finish(NAME);
    };
    let key = tab.key;
    main_after(SETTLE, move || in_card(root, key, id, (requests, first, then)));
}

/// A comment on a passage marked in the plan's card.
fn in_card(root: PathBuf, key: u64, id: u64, next: Next) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let selected = plan::select(
        &tab,
        id,
        PLAN_IN_CARD,
        "Add a failing test for a file without a trailing newline",
    );
    let typed = comment_on(&tab, id, PLAN_IN_CARD, "Name the file the test reads");
    check(
        "a passage marked in the plan's card takes a comment",
        selected && typed && comments(&tab, id) == ["Name the file the test reads"],
        format!("{:?}", comments(&tab, id)),
    );
    tab.open_plan(id);
    main_after(SETTLE, move || in_doc(root, key, id, next));
}

/// Comments in the plan's document: on two passages and on the whole plan.
fn in_doc(root: PathBuf, key: u64, id: u64, next: Next) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    check(
        "the card marks the commented passage",
        plan::card_marks(&tab, id) == Some(1),
        format!("{:?}", plan::card_marks(&tab, id)),
    );
    let selected = plan::select(&tab, id, PLAN_IN_DOC, "Lexer::next_line");
    tab.plan_comment(id, PLAN_IN_DOC);
    main_after(SETTLE, move || {
        if let Some((view, _)) = plan::comment_view() {
            view_picture(&view, "agent-plan-comment");
        }
        let Some(tab) = tab_of(key) else { return finish(NAME) };
        plan::type_comment("Is the lexer the right place?");
        tab.plan_commit(id);
        if let Some(text) = plan::text_of(key, id) {
            text.setSelectedRange(NSRange::new(0, 0));
        }
        let general = comment_on(&tab, id, PLAN_IN_DOC, "Keep the change small");
        let third = plan::select(&tab, id, PLAN_IN_DOC, "about ten minutes")
            && comment_on(&tab, id, PLAN_IN_DOC, "Run only the parser's fuzz target");
        let seen = plan::inspect(key, id);
        check(
            "the document lists the comments beside the plan and marks their passages",
            selected
                && general
                && third
                && seen.as_ref().is_some_and(|s| {
                    s.notes == 4 && s.marked == 3 && s.waiting && s.revise == t("Mit Kommentaren überarbeiten lassen")
                }),
            format!("{seen:?}; {:?}", comments(&tab, id)),
        );
        edit(root, key, id, next);
    });
}

/// Editing a comment starts from its text; removing one unmarks its passage.
fn edit(root: PathBuf, key: u64, id: u64, next: Next) {
    let (Some(tab), Some(anchor)) = (tab_of(key), plan::text_of(key, id)) else {
        return finish(NAME);
    };
    let ids = note_ids(&tab, id);
    if let Some(note) = ids.get(1) {
        tab.plan_edit_note(*note, &anchor);
    }
    let current = plan::comment_view().map(|(_, text)| text);
    plan::type_comment("Fix it in the tokenizer instead");
    tab.plan_commit(id);
    let after = comments(&tab, id);
    check(
        "editing a comment starts from its text and changes it",
        current.as_deref() == Some("Is the lexer the right place?")
            && after.get(1).map(String::as_str) == Some("Fix it in the tokenizer instead"),
        format!("{current:?} → {after:?}"),
    );
    if let Some(note) = ids.get(2) {
        tab.plan_remove_note(*note);
    }
    let seen = plan::inspect(key, id);
    check(
        "removing a comment drops it and unmarks its passage",
        seen.as_ref().is_some_and(|s| s.notes == 3 && s.marked == 2) && comments(&tab, id).len() == 3,
        format!("{seen:?}; {:?}", comments(&tab, id)),
    );
    pictures(root, key, id, next);
}

/// The document and the card, in dark and in light.
fn pictures(root: PathBuf, key: u64, id: u64, next: Next) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    picture(&controller, "agent-plan-doc");
    appearance(&controller, constant!(NSAppearanceNameDarkAqua));
    main_after(SETTLE, move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        picture(&controller, "agent-plan-doc-dark");
        if let Some(tab) = tab_of(key) {
            tab.select();
            tab.timeline().scroll_to_end();
        }
        main_after(SETTLE, move || {
            let Some(controller) = controller_of(&root) else {
                return finish(NAME);
            };
            check(
                "the card marks the passages commented on in the document",
                tab_of(key).and_then(|tab| plan::card_marks(&tab, id)) == Some(2),
                format!("{:?}", tab_of(key).and_then(|tab| plan::card_marks(&tab, id))),
            );
            picture(&controller, "agent-plan-card-dark");
            appearance(&controller, constant!(NSAppearanceNameAqua));
            main_after(SETTLE, move || {
                if let Some(controller) = controller_of(&root) {
                    picture(&controller, "agent-plan-card");
                }
                revise(root, key, id, next);
            });
        });
    });
}

/// "Mit Kommentaren überarbeiten lassen", then the CLI's answer and the revised plan.
fn revise(root: PathBuf, key: u64, id: u64, next: Next) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let sent = tab.plan_feedback(id);
    tab.plan(id, PLAN_REVISE);
    let state = plan_state(&tab, "toolu_plan_1");
    let back = RequestState::Decided(Decision::Rejected {
        feedback: Some(FEEDBACK.into()),
        stop: false,
    });
    check(
        "revising sends each comment under the passage it quotes",
        sent.as_deref() == Some(FEEDBACK) && state.as_ref() == Some(&back),
        format!("{sent:?} → {state:?}"),
    );
    let seen = plan::inspect(key, id);
    check(
        "the document takes no answer once the plan went back",
        seen.as_ref()
            .is_some_and(|s| !s.waiting && s.state == t("Plan zurückgegeben")),
        format!("{seen:?}"),
    );
    tab.feed(&format!("{}\n{}", sent_back(1, FEEDBACK), plan_frames(2, REVISED)));
    tab.timeline().scroll_to_end();
    check(
        "the thread keeps the comments the plan went back with",
        plan_state(&tab, "toolu_plan_1").as_ref() == Some(&back),
        format!("{:?}", plan_state(&tab, "toolu_plan_1")),
    );
    main_after(SETTLE, move || reject(root, key, id, next));
}

/// "Ablehnen" on the revised plan.
fn reject(root: PathBuf, key: u64, id: u64, (requests, first, then): Next) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let visible = tab.inspect().visible;
    check(
        "the answered plan's card says it went back with comments",
        visible.iter().any(|v| v == t("Plan mit Kommentaren zurückgegeben")),
        visible.join(" | "),
    );
    if let Some(revised) = entry_id(&tab, "toolu_plan_2") {
        tab.plan(revised, PLAN_REJECT);
    }
    let state = plan_state(&tab, "toolu_plan_2");
    check(
        "rejecting a plan tells the agent not to carry it out",
        state
            == Some(RequestState::Decided(Decision::Rejected {
                feedback: None,
                stop: true,
            })),
        format!("{state:?}"),
    );
    main_after(SETTLE, move || {
        let Some(controller) = controller_of(&root) else {
            return finish(NAME);
        };
        picture(&controller, "agent-plan-answered");
        let visible = tab_of(key).map(|tab| tab.inspect().visible).unwrap_or_default();
        check(
            "a rejected plan's card says so",
            visible.iter().any(|v| v == t("Plan abgelehnt")),
            visible.join(" | "),
        );
        let ai_tab = tab_of(key).and_then(|tab| controller.tab_with_key(&tab.tab_key()));
        if let Some(ai_tab) = ai_tab {
            controller.close_tab(ai_tab);
        }
        main_after(SETTLE, move || closed(root, key, id, (requests, first, then)));
    });
}

/// Closing the plan's AI tab takes its document with it, whose answers would go nowhere.
fn closed(root: PathBuf, key: u64, id: u64, (requests, first, then): Next) {
    let Some(controller) = controller_of(&root) else {
        return finish(NAME);
    };
    let doc = controller.tab_with_key(&format!("plan:{key}:{id}"));
    check(
        "the plan's document closes with its AI tab",
        tab_of(key).is_none() && doc.is_none() && plan::inspect(key, id).is_none(),
        format!("{:?} / {doc:?}", tab_of(key).map(|t| t.key)),
    );
    if let Some(doc) = doc {
        controller.close_tab(doc);
    }
    if let Some(tab) = tab_of(requests) {
        tab.select();
    }
    then(root, requests, first);
}
