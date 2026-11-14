//! The `agent-actions` scenario: what the user does in a Claude tab, against the stand-in of `agent_stand_in`
//! instead of `claude`. The composer's menus pick commands, models, efforts and modes, and a mode the CLI refuses
//! comes back with its reason; permission cards store a rule, take an edited input, refuse invalid JSON and deny;
//! questions and a plan are answered; a background command and a subagent are shown, moved and stopped; the turn is
//! stopped and the queued prompt goes out; then Remote Control, renaming, bookmarks, pasting, rewinding and the
//! sandbox restart the session, and the AI panel's context menu acts on it. Each answer is checked in what the app
//! wrote to the stand-in.

use ide_core::LockIgnoringPoison;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use ide_thread::{EntryKind, Status};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSApplication, NSMenu, NSModalResponse, NSTextField, NSView};
use serde_json::Value;

use super::agent::{controller_of, picture, tab_of};
use super::agent_stand_in::{AREAS, COLOUR, EXTRAS, SESSION, StandIn, TESTS};
use super::helpers::{alert_texts, answer_alert, in_any_mode_after};
use super::{check, finish, helpers};
use ide_ui_agent::{AgentTab, TabMenu};

const NAME: &str = "agent-actions";
/// Time for the timeline to build and lay out the cards that scrolling or a press changed; it reports no end.
const CARDS_BUILT: Duration = Duration::from_millis(300);
/// The tools of the stand-in's turn, each with a card of its own.
const TOOLS: [&str; 5] = ["Grep", "WebSearch", "Frobnicate", "Read", "WebFetch"];

/// The messages of the alerts the scenario answered, first first.
static ANSWERED: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub(super) fn answered() -> Vec<String> {
    ANSWERED.lock_ignoring_poison().clone()
}

/// Answers the next alert with `response` as soon as it shows, typing `text` into its field first when given.
pub(super) fn answer_when_shown(response: NSModalResponse, text: Option<&'static str>) {
    poll_alert(response, text, 50);
}

fn poll_alert(response: NSModalResponse, text: Option<&'static str>, tries: u32) {
    in_any_mode_after(Duration::from_millis(200), move || {
        let shown = MainThreadMarker::new().and_then(|mtm| NSApplication::sharedApplication(mtm).modalWindow());
        let Some(window) = shown else {
            if tries > 0 {
                poll_alert(response, text, tries - 1);
            }
            return;
        };
        if let (Some(text), Some(content)) = (text, window.contentView()) {
            fill_field(&content, text);
        }
        let message = alert_texts().into_iter().find(|t| !t.is_empty()).unwrap_or_default();
        ANSWERED.lock_ignoring_poison().push(message);
        answer_alert(response);
    });
}

fn fill_field(view: &NSView, text: &str) -> bool {
    if let Some(field) = view.downcast_ref::<NSTextField>()
        && field.isEditable()
    {
        field.setStringValue(&crate::util::ns(text));
        return true;
    }
    view.subviews().iter().any(|child| fill_field(&child, text))
}

pub(super) fn stand_in() -> StandIn {
    StandIn {
        dir: ide_core::paths::home().join("claude-stand-in"),
    }
}

/// Calls `then` once `ready` holds; when it does not in time, that is the failed check `what` and the end.
pub(super) fn soon(what: &'static str, ready: impl Fn() -> bool + 'static, then: impl FnOnce() + 'static) {
    helpers::wait_until(ready, move |ok| {
        if ok {
            then();
        } else {
            check(what, false, "waited in vain");
            finish(NAME);
        }
    });
}

/// The titles of `menu`'s items, separators as empty titles.
pub(super) fn titles(menu: &NSMenu) -> Vec<String> {
    menu.itemArray().iter().map(|item| item.title().to_string()).collect()
}

pub(super) fn index_of(menu: &NSMenu, title: &str) -> Option<isize> {
    titles(menu)
        .iter()
        .position(|t| t == title)
        .and_then(|n| isize::try_from(n).ok())
}

/// The pending request of the tab, with its entry and the CLI's id for it.
fn pending(tab: &AgentTab) -> Option<(u64, String)> {
    let thread = tab.snapshot();
    let entry = thread.pending_requests().into_iter().next()?;
    let id = match &entry.kind {
        EntryKind::PermissionRequest(r) => Some(r.request_id.clone()),
        EntryKind::Question(q) => q.request_id.clone(),
        EntryKind::PlanApproval(p) => p.request_id.clone(),
        _ => None,
    }?;
    Some((entry.id.get(), id))
}

/// Waits for the stand-in's request `request` to wait for an answer, then hands its entry to `then`.
fn when_asked(key: u64, request: &'static str, then: impl FnOnce(u64) + 'static) {
    soon(
        "the stand-in's request waits for an answer",
        move || tab_of(key).and_then(|t| pending(&t)).is_some_and(|(_, r)| r == request),
        move || {
            let Some(id) = tab_of(key).and_then(|t| pending(&t)).map(|(id, _)| id) else {
                return finish(NAME);
            };
            if let Some(tab) = tab_of(key) {
                tab.timeline().scroll_to_end();
            }
            helpers::after(CARDS_BUILT, move || then(id));
        },
    );
}

/// Waits for the app's answer to request `request`, then checks it.
fn when_answered(key: u64, request: &'static str, then: impl FnOnce(Value) + 'static) {
    soon(
        "the app answers the stand-in's request",
        move || stand_in().answer_to(request).is_some(),
        move || {
            let answer = stand_in().answer_to(request).unwrap_or_default();
            if tab_of(key).is_none() {
                return finish(NAME);
            }
            then(answer);
        },
    );
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let files = [("hello.txt", "hello world\n"), ("README.md", "# Actions\n")];
    let Ok(root) = helpers::make_project("actions-project", &files) else {
        check("the project is made", false, "");
        return finish(NAME);
    };
    crate::project::trust(&root);
    let Ok(prepared) = StandIn::new(&stand_in().dir, &root) else {
        check("the stand-in is written", false, "");
        return finish(NAME);
    };
    ide_ui_agent::set_test_claude(prepared.program());
    crate::app::open_project(mtm, root.clone());
    helpers::wait_for_window(&root.clone(), move |controller| {
        let Some(controller) = controller else {
            return finish(NAME);
        };
        tool_cards(root, &controller);
    });
}

/// The tab that runs the stand-in, once a tab without a program said so.
fn start(root: PathBuf) {
    helpers::wait_for_window(&root.clone(), move |controller| {
        let Some(controller) = controller else {
            return finish(NAME);
        };
        missing_program(&controller);
        let Some(tab) = ide_ui_agent::open_tab(&controller, None) else {
            return finish(NAME);
        };
        tab.keep_remote_control_off();
        let key = tab.key;
        soon(
            "the session starts with the stand-in",
            move || {
                tab_of(key).is_some_and(|t| t.is_live())
                    && !stand_in().requests("initialize").is_empty()
                    && tab_of(key)
                        .is_some_and(|t| t.inspect().remote.is_some() && t.menu(TabMenu::Slash).numberOfItems() == 2)
            },
            move || menus(root, key),
        );
    });
}

/// A tab whose program is not there says so instead of starting.
fn missing_program(controller: &crate::window::WindowController) {
    let Some(tab) = ide_ui_agent::open_replay(controller) else {
        return;
    };
    tab.launch_with(None);
    let none = tab.problem_name();
    tab.launch_with(Some(stand_in().dir.join("gone")));
    let gone = tab.problem_name();
    let shown = tab.problem_texts();
    check(
        "without the program, or with a program that is gone, the tab says the CLI is not found and how to get it",
        none == Some("not found")
            && gone == Some("not found")
            && shown.iter().any(|t| t == "Claude Code nicht gefunden")
            && shown.iter().any(|t| t == "Erneut versuchen"),
        format!("{none:?}, {gone:?}, {shown:?}"),
    );
    let model = tab.menu(TabMenu::Model);
    let history = tab.menu(TabMenu::History);
    let slash = tab.menu(TabMenu::Slash);
    check(
        "before the CLI said anything, the menus say there are no models, sessions or commands yet",
        titles(&model).get(1).map(String::as_str) == Some("Standardmodell")
            && model.itemAtIndex(1).is_some_and(|i| !i.isEnabled())
            && titles(&history) == ["Keine früheren Sitzungen"]
            && titles(&slash) == ["Noch keine Befehle bekannt"],
        format!("{:?} {:?} {:?}", titles(&model), titles(&history), titles(&slash)),
    );
}

/// Every kind of tool shows in a card of its own, folded and opened.
fn tool_cards(root: PathBuf, controller: &crate::window::WindowController) {
    let Some(tab) = ide_ui_agent::open_replay(controller) else {
        return finish(NAME);
    };
    let frames: Vec<String> = super::agent_stand_in::tools_turn(&root)
        .iter()
        .map(|f| f.to_string())
        .collect();
    tab.select();
    tab.feed(&frames.join("\n"));
    let key = tab.key;
    helpers::wait_until(
        move || tab_of(key).is_some_and(|tab| tab.inspect().rows >= 6),
        move |_| {
            let Some(tab) = tab_of(key) else { return finish(NAME) };
            tab.timeline().scroll_to_end();
            tab.timeline().settle();
            let folded = tab.timeline().visible_summaries();
            for part in [ide_ui_agent::Part::Input, ide_ui_agent::Part::Output] {
                tab.open_all(part);
            }
            let seen = folded.clone();
            let all_shown = move || {
                tab_of(key).is_some_and(|tab| {
                    tab.timeline().scroll_to_end();
                    tab.timeline().settle();
                    let opened = tab.timeline().visible_summaries();
                    TOOLS
                        .iter()
                        .all(|name| [&seen, &opened].iter().any(|s| s.iter().any(|v| v.contains(name))))
                })
            };
            helpers::wait_until(all_shown, move |_| {
                let Some(tab) = tab_of(key) else { return finish(NAME) };
                tab.timeline().scroll_to_end();
                tab.timeline().settle();
                let opened = tab.timeline().visible_summaries();
                let named = |seen: &[String], name: &str| seen.iter().any(|s| s.contains(name));
                check(
                    "searches, fetches, MCP and unknown tools each show a card that opens to its input and output",
                    TOOLS.iter().all(|name| named(&folded, name) || named(&opened, name)) && tab.inspect().rows >= 6,
                    format!("{folded:?} / {opened:?}"),
                );
                if let Some(controller) = controller_of(&root) {
                    picture(&controller, "agent-actions-tools");
                }
                start(root);
            });
        },
    );
}

/// Before the first prompt: the slash, model and mode menus, filled from what `initialize` answered.
fn menus(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let args = stand_in().args();
    check(
        "the CLI starts with stream JSON both ways and the mode the tab starts in",
        args.iter().any(|a| a == "--input-format=stream-json") && args.iter().any(|a| a.contains("default")),
        args.join(" "),
    );
    let slash = tab.menu(TabMenu::Slash);
    let tip = slash.itemAtIndex(0).and_then(|i| i.toolTip()).map(|t| t.to_string());
    check(
        "the slash menu lists the CLI's commands with what they do",
        titles(&slash) == ["/deploy", "/compact"] && tip.as_deref() == Some("Deploy to staging"),
        format!("{:?}, {tip:?}", titles(&slash)),
    );
    slash.performActionForItemAtIndex(0);
    check(
        "picking a command puts it into the composer",
        tab.composer_text() == "/deploy ",
        tab.composer_text(),
    );
    let model = tab.menu(TabMenu::Model);
    let heading = model.itemAtIndex(0).is_some_and(|i| !i.isEnabled());
    check(
        "the model menu lists the CLI's models under a heading, then the effort levels",
        heading
            && titles(&model).starts_with(&["Modell".to_string(), "Opus".to_string(), "Haiku".to_string()])
            && titles(&model).iter().any(|t| t == "Denktiefe"),
        format!("{:?}", titles(&model)),
    );
    if let Some(haiku) = index_of(&model, "Haiku") {
        model.performActionForItemAtIndex(haiku);
    }
    let model = tab.menu(TabMenu::Model);
    model.performActionForItemAtIndex(model.numberOfItems() - 1);
    let (picked, effort, _) = tab.picks();
    let mode = tab.menu(TabMenu::Mode);
    check(
        "the mode menu lists the five permission modes, the current one ticked",
        mode.numberOfItems() == 5
            && mode
                .itemAtIndex(0)
                .is_some_and(|i| i.state() == objc2_app_kit::NSControlStateValueOn),
        format!("{:?}", titles(&mode)),
    );
    answer_when_shown(NSAlertFirstButtonReturn, None);
    mode.performActionForItemAtIndex(2);
    soon(
        "the refused mode comes back",
        move || !answered().is_empty() && tab_of(key).is_some_and(|t| t.picks().2 == "default"),
        move || {
            let set_model = stand_in().requests("set_model");
            let set_effort = stand_in().requests("apply_flag_settings");
            check(
                "a picked model and effort go to the CLI and stay picked",
                picked.as_deref() == Some("haiku")
                    && effort.as_deref() == Some("max")
                    && set_model.last().is_some_and(|r| r["model"] == "haiku")
                    && set_effort.last().is_some_and(|r| r["settings"]["effortLevel"] == "max"),
                format!("{picked:?} {effort:?}, {set_model:?}, {set_effort:?}"),
            );
            check(
                "a mode the CLI refuses is not kept, and an alert says why",
                answered().first().map(String::as_str) == Some("Freigabemodus nicht gewechselt")
                    && !stand_in().requests("set_permission_mode").is_empty(),
                format!("{:?}", answered()),
            );
            first_prompt(root, key);
        },
    );
}

fn first_prompt(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    tab.type_and_send("Schreib hello.txt neu");
    when_asked(key, "p1", move |id| rules(root, key, id));
}

/// Write offers two rules: the menu lists both, the second is stored; a prompt typed meanwhile waits in the queue.
fn rules(root: PathBuf, key: u64, id: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    tab.type_and_send("Zuerst das");
    let queued = (tab.queued(), tab.queue_rows());
    tab.remove_queued(0);
    let removed = (tab.queued().len(), tab.queue_rows());
    tab.type_and_send("Danach die Tests");
    check(
        "a prompt sent while the turn runs waits in the queue, with a row that removes it",
        queued == (vec!["Zuerst das".to_string()], 1) && removed == (0, 0) && tab.queued() == ["Danach die Tests"],
        format!("{queued:?}, {removed:?}, {:?}", tab.queued()),
    );
    let Some(menu) = tab.rule_menu(id) else {
        check("a request with two rules offers a menu of them", false, "");
        return finish(NAME);
    };
    let shown = titles(&menu);
    check(
        "the menu names each rule with where it is stored",
        shown.len() == 2 && shown.iter().all(|t| t.starts_with("Write(hello.txt) · ")) && shown[0] != shown[1],
        format!("{shown:?}"),
    );
    if let Some(controller) = controller_of(&root) {
        picture(&controller, "agent-actions-rules");
    }
    menu.performActionForItemAtIndex(1);
    when_answered(key, "p1", move |answer| {
        check(
            "the rule picked goes to the CLI to be stored where the user chose",
            answer["behavior"] == "allow"
                && answer["decisionClassification"] == "user_permanent"
                && answer["updatedPermissions"][0]["destination"] == "userSettings",
            answer.to_string(),
        );
        when_asked(key, "p2", move |id| edited(root, key, id));
    });
}

/// WebFetch's input is edited as JSON: broken JSON is refused with an alert, valid JSON goes along.
fn edited(root: PathBuf, key: u64, id: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let editable = tab.timeline().edit_input(id, "{ kaputt");
    answer_when_shown(NSAlertFirstButtonReturn, None);
    tab.timeline().press_last("Einmal erlauben", 0);
    check(
        "allowing with input that is not JSON is refused with an alert and sends nothing",
        editable
            && answered().last().map(String::as_str) == Some("Die geänderte Eingabe ist kein gültiges JSON.")
            && stand_in().answer_to("p2").is_none(),
        format!("{editable}, {:?}", answered()),
    );
    tab.timeline()
        .edit_input(id, r#"{"url": "https://example.org/docs", "prompt": "summarize"}"#);
    tab.timeline().press_last("Einmal erlauben", 0);
    when_answered(key, "p2", move |answer| {
        check(
            "an edited input is allowed once with the user's changes",
            answer["behavior"] == "allow"
                && answer["updatedInput"]["url"] == "https://example.org/docs"
                && answer["decisionClassification"] == "user_temporary",
            answer.to_string(),
        );
        when_asked(key, "p3", move |_| always(root, key));
    });
}

/// A command with one rule: its button stores it at once.
fn always(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let pressed = tab.timeline().press_last("Immer erlauben: ", 0);
    when_answered(key, "p3", move |answer| {
        check(
            "the one rule a command offers is stored from its button",
            pressed && answer["updatedPermissions"][0]["rules"][0]["ruleContent"] == "ls:*",
            answer.to_string(),
        );
        when_asked(key, "p4", move |_| deny(root, key));
    });
}

/// A destructive command offers no rule; the user refuses it.
fn deny(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let buttons = tab.timeline().last_card_buttons();
    tab.timeline().press_last("Ablehnen", 0);
    when_answered(key, "p4", move |answer| {
        check(
            "a destructive command offers no rule, and refusing tells the model",
            !buttons.iter().any(|b| b.starts_with("Immer erlauben"))
                && answer["behavior"] == "deny"
                && answer["message"] == "The user denied this tool call.",
            format!("{buttons:?}: {answer}"),
        );
        when_asked(key, "q1", move |id| questions(root, key, id));
    });
}

/// Two questions: one option picked, which sends nothing yet; then options ticked and words of the user's own.
fn questions(root: PathBuf, key: u64, id: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    tab.timeline().press_last("blue", 0);
    let early = stand_in().answer_to("q1");
    helpers::after(CARDS_BUILT, move || {
        let Some(tab) = tab_of(key) else { return finish(NAME) };
        let ticked = tab.timeline().tick_options(id, 1, &["unit", "ui"]);
        let typed = tab.timeline().type_field(id, 1, "lint");
        tab.timeline().press_last("Senden", 1);
        when_answered(key, "q1", move |answer| {
            let answers = &answer["updatedInput"]["answers"];
            check(
                "the questions are answered together: the option picked, the options ticked and the user's words",
                early.is_none()
                    && ticked == 2
                    && typed
                    && answers[COLOUR] == "blue"
                    && answers[TESTS] == "unit, ui, lint",
                format!("{early:?}, {ticked}, {typed}: {answer}"),
            );
            when_asked(key, "q2", move |id| words(root, key, id));
        });
    });
}

/// Two multiple-choice questions answered only in words: one button sends both.
fn words(root: PathBuf, key: u64, id: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let typed = tab.timeline().type_field(id, 0, "Alles") && tab.timeline().type_field(id, 1, "Nichts");
    let pressed = tab.timeline().press_last("Senden", 0);
    when_answered(key, "q2", move |answer| {
        let answers = &answer["updatedInput"]["answers"];
        check(
            "one button sends every question that has words typed, even where its own button was not pressed",
            typed && pressed && answers[AREAS] == "Alles" && answers[EXTRAS] == "Nichts",
            format!("{typed} {pressed}: {answer}"),
        );
        when_asked(key, "x1", move |id| plan(root, key, id));
    });
}

/// The plan goes back with the user's comment.
fn plan(root: PathBuf, key: u64, id: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let (in_card, in_tab) = keyboard_targets(&root, &tab, id);
    check(
        "a comment from the keyboard goes to the plan whose text has the focus, else to the plan that waits in the tab; Tab and ⇧Tab leave the plan's text",
        in_card && in_tab,
        format!("{in_card} {in_tab}"),
    );
    let typed = tab.timeline().type_field(id, 0, "Bitte zuerst die Tests");
    let buttons = tab.timeline().last_card_buttons();
    let pressed = tab.timeline().press_last("Überarbeiten lassen", 0)
        || tab.timeline().press_last("Mit Kommentaren überarbeiten lassen", 0);
    when_answered(key, "x1", move |answer| {
        check(
            "a plan sent back for revision carries the user's comment",
            typed && pressed && answer["behavior"] == "deny" && answer.to_string().contains("Bitte zuerst die Tests"),
            format!("{buttons:?}: {answer}"),
        );
        soon(
            "the background command and the subagent start",
            move || {
                tab_of(key).is_some_and(|t| {
                    call_entry(&t, "toolu_bg").is_some()
                        && call_entry(&t, "toolu_sub").is_some()
                        && call_entry(&t, "toolu_fg").is_some()
                })
            },
            move || work(root, key),
        );
    });
}

/// Where a comment typed from the keyboard goes: to plan `id` while its text has the focus, and still to it while
/// the timeline of its tab has the focus.
fn keyboard_targets(root: &std::path::Path, tab: &AgentTab, id: u64) -> (bool, bool) {
    let Some(controller) = controller_of(&root.to_path_buf()) else {
        return (false, false);
    };
    let Some(window) = controller.window() else {
        return (false, false);
    };
    let text = tab.timeline().inputs().borrow().plan_text.get(&id).cloned();
    let target = |place: u8| {
        ide_ui_agent::plan::keyboard_target(&controller)
            .is_some_and(|(at, plan, where_)| at.key == tab.key && plan == id && where_ == place)
    };
    let in_card = text.as_ref().is_some_and(|text| {
        ide_ui_agent::plan::select(tab, id, 0, "Tests") && window.makeFirstResponder(Some(text)) && target(0)
    });
    let leaves = text.as_ref().is_some_and(|text| {
        let focused = |w: &objc2_app_kit::NSWindow| {
            w.firstResponder()
                .is_some_and(|r| std::ptr::eq(&*r as *const _ as *const (), &**text as *const _ as *const ()))
        };
        // SAFETY: Tab and ⇧Tab are standard text commands a text view takes with any sender.
        let () = unsafe { objc2::msg_send![&**text, insertTab: None::<&objc2::runtime::AnyObject>] };
        let forward = !focused(&window);
        window.makeFirstResponder(Some(text));
        // SAFETY: As above.
        let () = unsafe { objc2::msg_send![&**text, insertBacktab: None::<&objc2::runtime::AnyObject>] };
        forward && !focused(&window)
    });
    let in_tab = window.makeFirstResponder(Some(tab.timeline().table())) && target(0);
    (in_card && leaves, in_tab)
}

/// The entry of tool call `call`.
fn call_entry(tab: &AgentTab, call: &str) -> Option<u64> {
    tab.entry_ids(|kind| matches!(kind, EntryKind::ToolCall(c) if c.call_id == call && (call != "toolu_bg" || c.background_task.is_some())))
        .first()
        .copied()
}

/// The background command's output is shown, it is moved to the background, the subagent is stopped, and the
/// usage is asked for while the turn runs.
fn work(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let entries = (
        call_entry(&tab, "toolu_bg"),
        call_entry(&tab, "toolu_fg"),
        call_entry(&tab, "toolu_sub"),
    );
    let (Some(bg), Some(fg), Some(sub)) = entries else {
        check(
            "the background command, the running test and the subagent show",
            false,
            format!("{entries:?}"),
        );
        return finish(NAME);
    };
    let pressed = [
        tab.timeline().press_on(bg, "Ausgabe zeigen"),
        tab.timeline().press_on(fg, "Im Hintergrund ausführen"),
        tab.timeline().press_on(sub, "Stopp"),
    ];
    tab.press_work(crate::activity::Part::Background);
    tab.press_work(crate::activity::Part::Subagents);
    let listed = tab.has_work(crate::activity::Part::Subagents);
    let at = root.clone();
    soon(
        "the background output opens in an editor",
        move || controller_of(&at).is_some_and(|c| c.tab_titles().iter().any(|t| t == "bg1.txt")),
        move || {
            let moved = stand_in().requests("background_tasks");
            let stopped = stand_in().requests("stop_task");
            check(
                "the cards' buttons fetch and open the background output, move the test away and stop the subagent",
                pressed == [true, true, true]
                    && listed
                    && !stand_in().requests("get_task_output").is_empty()
                    && moved.iter().any(|r| r["tool_use_id"] == "toolu_fg")
                    && stopped.iter().any(|r| r["task_id"] == "sub1"),
                format!("{pressed:?} {listed}, {moved:?}, {stopped:?}"),
            );
            soon(
                "the running turn asks for the usage",
                || !stand_in().requests("get_usage").is_empty(),
                move || stop(root, key),
            );
        },
    );
}

/// The send button with an empty composer stops the turn; the queued prompt goes out once the session rests.
fn stop(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    if let Some(controller) = controller_of(&root) {
        controller.select_tab(&tab.tab_key());
    }
    tab.send_or_stop();
    soon(
        "the queued prompt goes out after the stop",
        move || {
            stand_in().prompts().iter().any(|p| p == "Danach die Tests")
                && tab_of(key).is_some_and(|t| t.status() == Status::Idle && t.queued().is_empty())
        },
        move || {
            check(
                "stopping interrupts the CLI, and the queued prompt follows when the session rests",
                !stand_in().requests("interrupt").is_empty() && stand_in().prompts().len() == 2,
                format!("{:?}", stand_in().prompts()),
            );
            remote(root, key);
        },
    );
}

/// The phone connects the session to Remote Control and its menu ends it again.
fn remote(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let before = tab.inspect().remote;
    tab.remote_control(&tab.root());
    soon(
        "Remote Control connects",
        move || tab_of(key).is_some_and(|t| t.inspect().remote != before),
        move || {
            let Some(tab) = tab_of(key) else { return finish(NAME) };
            let menu = tab.menu(TabMenu::Remote);
            let shown = titles(&menu);
            if let Some(end) = index_of(&menu, "Remote Control beenden") {
                menu.performActionForItemAtIndex(end);
            }
            soon(
                "the end of Remote Control reaches the CLI",
                || stand_in().requests("remote_control").len() == 2,
                move || {
                    let asked: Vec<Value> = stand_in()
                        .requests("remote_control")
                        .into_iter()
                        .map(|r| r["enabled"].clone())
                        .collect();
                    check(
                        "the phone connects the session, and its menu opens the page, copies the link or ends it",
                        shown.len() == 4 && asked == [Value::Bool(true), Value::Bool(false)],
                        format!("{shown:?}, {asked:?}"),
                    );
                    names(root, key);
                },
            );
        },
    );
}

/// Renaming, bookmarking, the clock's menu, opening a file of a card and pasting.
fn names(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    answer_when_shown(NSAlertFirstButtonReturn, Some("Aktionen im Test"));
    tab.rename();
    check(
        "renaming asks for the name and the tab takes it",
        tab.title() == "Aktionen im Test",
        tab.title(),
    );
    tab.toggle_bookmark();
    check(
        "the bookmark marks the session in the tab and the AI panel",
        tab.is_bookmarked() && ide_ui_agent::agent_panel::is_bookmarked(tab.thread_id()),
        "",
    );
    let listed = move || {
        tab_of(key).is_some_and(|tab| {
            titles(&tab.menu(TabMenu::History))
                .iter()
                .any(|t| t.starts_with("Aktionen im Test · "))
        })
    };
    helpers::wait_until(listed, move |_| history(root, key));
}

/// The clock's menu, opening a file of a card and the keyboard on a card.
fn history(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let history = tab.menu(TabMenu::History);
    let past = titles(&history);
    check(
        "the clock lists the folder's sessions with their age",
        past.iter().any(|t| t.starts_with("Aktionen im Test · ")),
        format!("{past:?}"),
    );
    let before = ide_ui_agent::all().len();
    if let Some(index) = past.iter().position(|t| t.starts_with("Aktionen im Test · ")) {
        history.performActionForItemAtIndex(isize::try_from(index).unwrap_or(0));
    }
    let open_tabs = ide_ui_agent::all().len() - before;
    let pressed =
        call_entry(&tab, "toolu_write").is_some_and(|write| tab.timeline().press_on(write, "Im Editor öffnen"));
    let shown = controller_of(&root).map(|c| c.tab_titles()).unwrap_or_default();
    check(
        "a session of the clock that is open is shown, not opened twice, and a Write card opens its file",
        open_tabs == 0 && pressed && shown.iter().any(|t| t == "hello.txt"),
        format!("{open_tabs} tabs, {shown:?}"),
    );
    let ls = call_entry(&tab, "toolu_ls");
    let closed = ls.is_some_and(|ls| tab.timeline().is_open(ls, ide_ui_agent::Part::Output));
    let keyed = ls.is_some_and(|ls| tab.timeline().key_on(ls, " "));
    let opened = ls.is_some_and(|ls| tab.timeline().is_open(ls, ide_ui_agent::Part::Output));
    check(
        "Space on a selected command card opens its output",
        keyed && opened != closed,
        format!("{closed} -> {opened}"),
    );
    keys(root, key);
}

/// A prompt sent with Return that takes a pasted file, picture and page along.
fn attachments(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    paste(&tab, &root);
    tab.select();
    tab.timeline().scroll_to_end();
    tab.type_in_composer("Mit Anhängen");
    tab.composer_key(36, "\r", objc2_app_kit::NSEventModifierFlags::empty());
    soon(
        "the prompt with attachments is answered",
        move || stand_in().prompts().len() == 3 && tab_of(key).is_some_and(|t| t.status() == Status::Idle),
        move || {
            let sent = stand_in()
                .written()
                .into_iter()
                .rfind(|f| f["type"] == "user")
                .map(|f| f.to_string())
                .unwrap_or_default();
            check(
                "a prompt takes its pasted file, picture and web text along",
                sent.contains("README.md") && sent.contains("image/png") && sent.contains("docs.example.org"),
                sent.chars().take(400).collect::<String>(),
            );
            rewind(root, key);
        },
    );
}

/// The keyboard in the composer: the arrows and Tab pick a completion, Esc closes the list, ⌘Esc leaves the composer
/// and comes back, and the window's menu commands show the session's work.
fn keys(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    tab.select();
    tab.type_in_composer("schau @REA");
    soon(
        "the composer offers completions",
        move || tab_of(key).is_some_and(|t| !t.suggestion_labels().is_empty()),
        move || {
            use objc2::sel;
            let Some(tab) = tab_of(key) else { return finish(NAME) };
            let listed = tab.suggestion_labels();
            tab.composer_does(sel!(moveDown:));
            tab.composer_does(sel!(moveUp:));
            tab.composer_does(sel!(insertTab:));
            let picked = tab.composer_text();
            tab.type_in_composer("schau @REA");
            soon(
                "the composer offers completions again",
                move || tab_of(key).is_some_and(|t| !t.suggestion_labels().is_empty()),
                move || {
                    let Some(tab) = tab_of(key) else { return finish(NAME) };
                    tab.composer_does(sel!(cancelOperation:));
                    let closed = tab.suggestion_labels().is_empty();
                    check(
                        "the arrows and Tab pick a completion in the composer, and Esc closes the list",
                        picked == "schau @README.md " && closed,
                        format!("{listed:?}, {picked:?}, {closed}"),
                    );
                    focus(root, key);
                },
            );
        },
    );
}

/// ⌘Esc moves into the composer and out again, and the window's menu commands show the session's work.
fn focus(root: PathBuf, key: u64) {
    let (Some(controller), Some(tab)) = (controller_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    tab.type_in_composer("");
    tab.composer_key(48, "\t", objc2_app_kit::NSEventModifierFlags::Control);
    let before = tab.composer_has_focus();
    ide_ui_agent::toggle_focus(&controller);
    let inside = tab.composer_has_focus() != before;
    ide_ui_agent::toggle_focus(&controller);
    let left = tab.composer_has_focus() == before;
    let shown = ide_ui_agent::can_show_work(&controller, crate::activity::Part::Subagents);
    ide_ui_agent::show_work(&controller, crate::activity::Part::Subagents);
    check(
        "⌘Esc moves the focus into the composer and back, and the menu offers the session's subagents",
        inside && left && shown,
        format!("{inside} {left} {shown}"),
    );
    attachments(root, key);
}

/// A file, a picture and nothing usable pasted into the composer.
fn paste(tab: &AgentTab, root: &std::path::Path) {
    let pasteboard = objc2_app_kit::NSPasteboard::pasteboardWithUniqueName();
    let url = objc2_foundation::NSURL::fileURLWithPath(&crate::util::ns(&root.join("README.md").to_string_lossy()));
    pasteboard.clearContents();
    let wrote = url.absoluteString().is_some_and(|text| {
        pasteboard.setString_forType(&text, crate::util::constant!(objc2_app_kit::NSPasteboardTypeFileURL))
    });
    let file = tab.take_pasteboard(&pasteboard);
    pasteboard.clearContents();
    let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let data = objc2_foundation::NSData::with_bytes(&png);
    pasteboard.setData_forType(Some(&data), crate::util::constant!(objc2_app_kit::NSPasteboardTypePNG));
    let image = tab.take_pasteboard(&pasteboard);
    pasteboard.clearContents();
    let nothing = tab.take_pasteboard(&pasteboard);
    pasteboard.clearContents();
    let page = "Erste Zeile der Seite\nZweite Zeile der Seite\nDritte Zeile der Seite\nVierte Zeile der Seite";
    pasteboard.setString_forType(
        &crate::util::ns(page),
        crate::util::constant!(objc2_app_kit::NSPasteboardTypeString),
    );
    pasteboard.setString_forType(
        &crate::util::ns("https://docs.example.org/page"),
        &crate::util::ns("org.chromium.source-url"),
    );
    let web = tab.take_pasteboard(&pasteboard);
    let names = tab.attachment_names();
    check(
        "a pasted file, picture and page from a browser become attachments, an empty paste does nothing",
        wrote
            && file
            && image
            && web
            && !nothing
            && names.len() == 3
            && names[1] == "image"
            && names[2] == "Text von docs.example.org (external)",
        format!("{file} {image} {web} {nothing}: {names:?}"),
    );
}

/// The code and then the conversation go back to before the second prompt, which starts the stand-in again.
fn rewind(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let prompts = tab.entry_ids(|kind| matches!(kind, EntryKind::UserMessage(_)));
    let Some(&second) = prompts.get(1) else {
        check("both prompts are in the conversation", false, format!("{prompts:?}"));
        return finish(NAME);
    };
    let offered = tab.rewind_titles(second);
    let menu = tab.rewind_menu_of(second);
    let had_checkpoint = tab.checkpoint_count() > 0;
    if had_checkpoint {
        ide_ui_agent::answer_alerts(&[NSAlertFirstButtonReturn, NSAlertFirstButtonReturn]);
    } else {
        answer_when_shown(NSAlertFirstButtonReturn, None);
    }
    let seen_before = answered().len();
    if let Some(menu) = &menu {
        menu.performActionForItemAtIndex(1);
    }
    soon(
        "the code rewind reports",
        move || !ide_ui_agent::alerts_seen().is_empty() || answered().len() > seen_before,
        move || {
            check(
                "the rewind menu offers the conversation, the code and both, and the code goes back",
                offered.len() == 3 && (had_checkpoint || !stand_in().requests("rewind_files").is_empty()),
                format!(
                    "{offered:?}, checkpoint {had_checkpoint}, {:?}",
                    ide_ui_agent::alerts_seen()
                ),
            );
            let Some(tab) = tab_of(key) else { return finish(NAME) };
            let _ = std::fs::remove_file(stand_in().dir.join("args.txt"));
            if let Some(menu) = tab.rewind_menu_of(second) {
                menu.performActionForItemAtIndex(0);
            }
            soon(
                "the session starts again as a branch",
                move || {
                    stand_in().args().iter().any(|a| *a == format!("--resume={SESSION}"))
                        && tab_of(key).is_some_and(|t| t.is_live())
                },
                move || {
                    let args = stand_in().args();
                    let composer = tab_of(key).map(|t| t.composer_text()).unwrap_or_default();
                    check(
                        "rewinding the conversation resumes the session up to before the prompt, whose text returns",
                        args.iter().any(|a| a == "--fork-session") && composer == "Danach die Tests",
                        format!("{composer:?}: {}", args.join(" ")),
                    );
                    refused(root, key);
                },
            );
        },
    );
}

/// The CLI refuses the rewind with the first prompt: the whole conversation comes back, the session resumes without
/// the rewind, and the prompt goes out again.
fn refused(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let _ = super::agent_stand_in::next_start(
        &stand_in(),
        &[super::agent_stand_in::rewind_rejected()],
        &super::agent_stand_in::short_answer(),
    );
    let sent = stand_in().prompts().len();
    tab.send_or_stop();
    soon(
        "the refused rewind resumes the whole conversation and sends the prompt again",
        move || {
            stand_in().prompts().len() >= sent + 2
                && tab_of(key).is_some_and(|t| {
                    t.is_live()
                        && t.status() == Status::Idle
                        && !t.rewind_pending()
                        && t.entry_ids(|k| matches!(k, EntryKind::Notice(ide_thread::Notice::RewindRejected { .. })))
                            .len()
                            == 1
                })
        },
        move || {
            let args = stand_in().args();
            check(
                "a refused rewind brings the conversation back and resumes the session without the rewind",
                args.iter().any(|a| *a == format!("--resume={SESSION}"))
                    && !args.iter().any(|a| a.starts_with("--resume-session-at")),
                args.join(" "),
            );
            sandbox(root, key);
        },
    );
}

/// The sandbox switch starts a resting session again with the sandbox on.
fn sandbox(root: PathBuf, key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let _ = std::fs::remove_file(stand_in().dir.join("args.txt"));
    tab.toggle_sandbox();
    soon(
        "the session starts again with the sandbox",
        move || !stand_in().args().is_empty() && tab_of(key).is_some_and(|t| t.is_live()),
        move || {
            let args = stand_in().args().join(" ");
            check(
                "switching the sandbox on restarts the resting session with it",
                args.contains("sandbox"),
                args,
            );
            super::agent_sessions::panel(root, key);
        },
    );
}
