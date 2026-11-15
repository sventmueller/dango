//! The `agent-background` scenario: an AI tab that streams while another tab is in front shows the end of its answer
//! when it comes back, twice in a row, and keeps following the answer after; scrolling up stops following it.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::agent::tab_of;
use super::{check, finish, helpers, snapshot};
use ide_ui_agent::{AgentKey, AgentTab};

const NAME: &str = "agent-background";

pub fn run() {
    helpers::open_project(NAME, "background-project", &[], |controller| {
        let root = helpers::root("background-project");
        let Some(tab) = ide_ui_agent::open_replay(&controller) else {
            check("an AI tab opens", false, "");
            return finish(NAME);
        };
        tab.feed(&history(40));
        begin(&tab);
        let key = tab.key;
        let following = move || tab_of(key).is_some_and(|tab| tab.timeline().is_at_bottom());
        helpers::wait_until(following, move |_| to_background(root, key, 0));
    });
}

/// Puts another tab in front, streams into the hidden AI tab and brings it back.
fn to_background(root: PathBuf, key: AgentKey, round: u32) {
    let (Some(controller), Some(tab)) = (helpers::window_of(&root), tab_of(key)) else {
        return finish(NAME);
    };
    let ai = controller.tab_with_key(&tab.tab_key());
    let other = controller.tab_ids().into_iter().find(|id| Some(*id) != ai);
    let (Some(ai), Some(other)) = (ai, other) else {
        check(
            "the window has a second tab",
            false,
            format!("{:?}", controller.tab_ids()),
        );
        return finish(NAME);
    };
    controller.select_tab_by_id(other);
    let height = table_height(&tab);
    for n in 0..60 {
        append(
            &tab,
            &format!("Background line {round}.{n}, long enough to wrap a little in the timeline.\n\n"),
        );
    }
    check(
        "a hidden AI tab builds nothing while it streams",
        (table_height(&tab) - height).abs() < 0.5,
        format!("{height} → {}", table_height(&tab)),
    );
    controller.select_tab_by_id(ai);
    let back = move || followed(key, height + 100.0);
    helpers::wait_until(back, move |_| returned(root, key, round, height));
}

fn returned(root: PathBuf, key: AgentKey, round: u32, before: f64) {
    let Some(tab) = tab_of(key) else {
        return finish(NAME);
    };
    check(
        if round == 0 {
            "an AI tab that streamed in the background shows the end when it comes back"
        } else {
            "an AI tab that streamed in the background shows the end when it comes back again"
        },
        tab.timeline().is_at_bottom() && table_height(&tab) > before + 100.0,
        format!(
            "at bottom {}, height {before} → {}",
            tab.timeline().is_at_bottom(),
            table_height(&tab)
        ),
    );
    let height = table_height(&tab);
    for n in 0..30 {
        append(
            &tab,
            &format!("Visible line {round}.{n}, long enough to wrap a little in the timeline.\n\n"),
        );
    }
    helpers::wait_until(
        move || followed(key, height + 50.0),
        move |_| {
            let Some(tab) = tab_of(key) else {
                return finish(NAME);
            };
            check(
                if round == 0 {
                    "back in front, the AI tab keeps following its answer"
                } else {
                    "back in front again, the AI tab keeps following its answer"
                },
                tab.timeline().is_at_bottom() && table_height(&tab) > height + 50.0,
                format!(
                    "at bottom {}, height {height} → {}",
                    tab.timeline().is_at_bottom(),
                    table_height(&tab)
                ),
            );
            if round == 0 {
                return to_background(root, key, 1);
            }
            scroll_up_while_streaming(key);
        },
    );
}

/// A user who scrolls a little up while the answer streams stays where they scrolled, and follows again at the end.
fn scroll_up_while_streaming(key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return finish(NAME);
    };
    let clip = tab.timeline().view().contentView();
    let up = clip.bounds().origin.y - 30.0;
    clip.scrollToPoint(objc2_foundation::NSPoint::new(0.0, up));
    for n in 0..10 {
        append(&tab, &format!("Line {n} streamed while the user reads above.\n\n"));
    }
    helpers::wait_until(
        move || tab_of(key).is_some_and(|tab| !tab.timeline().is_at_bottom()),
        move |_| {
            let Some(tab) = tab_of(key) else {
                return finish(NAME);
            };
            let clip = tab.timeline().view().contentView();
            let y = clip.bounds().origin.y;
            check(
                "scrolling up a little while the answer streams stops following it",
                !tab.timeline().follows() && (y - up).abs() < 0.5,
                format!("follows {}, at {y}, scrolled to {up}", tab.timeline().follows()),
            );
            let end = table_height(&tab) - clip.bounds().size.height;
            clip.scrollToPoint(objc2_foundation::NSPoint::new(0.0, end));
            let height = table_height(&tab);
            for n in 0..10 {
                append(&tab, &format!("Line {n} streamed after the user came back.\n\n"));
            }
            helpers::wait_until(
                move || followed(key, height + 50.0),
                move |_| {
                    let Some(tab) = tab_of(key) else {
                        return finish(NAME);
                    };
                    check(
                        "scrolling back to the end follows the answer again",
                        tab.timeline().follows() && tab.timeline().is_at_bottom(),
                        format!("follows {}", tab.timeline().follows()),
                    );
                    if let Some(window) = tab.root().window() {
                        snapshot(&window, NAME);
                    }
                    finish(NAME);
                },
            );
        },
    );
}

/// Whether the tab `key` shows the end of its timeline, grown taller than `above`.
fn followed(key: AgentKey, above: f64) -> bool {
    tab_of(key).is_some_and(|tab| tab.timeline().is_at_bottom() && table_height(&tab) > above)
}

fn table_height(tab: &AgentTab) -> f64 {
    tab.timeline()
        .view()
        .documentView()
        .map_or(0.0, |table| table.frame().size.height)
}

fn history(entries: usize) -> String {
    let mut lines = Vec::new();
    for n in 0..entries / 2 {
        lines.push(format!(">> {}", json!({"type":"user", "uuid":format!("u{n}"), "message":{"role":"user","content":[{"type":"text","text":format!("Question {n}")}]}})));
        lines.push(json!({"type":"assistant", "message":{"id":format!("m{n}"),"role":"assistant","content":[{"type":"text","text":"A completed **answer**."}]}, "uuid":format!("a{n}")}).to_string());
    }
    lines.join("\n")
}

fn frame(event: Value) -> String {
    json!({"type":"stream_event", "event":event, "parent_tool_use_id":null}).to_string()
}

fn begin(tab: &AgentTab) {
    tab.feed(&frame(
        json!({"type":"message_start", "message":{"id":"live", "role":"assistant", "usage":{}}}),
    ));
    tab.feed(&frame(
        json!({"type":"content_block_start", "index":0, "content_block":{"type":"text", "text":"Start\n\n"}}),
    ));
    tab.timeline().scroll_to_end();
}

fn append(tab: &AgentTab, text: &str) {
    tab.feed(&frame(
        json!({"type":"content_block_delta", "index":0, "delta":{"type":"text_delta", "text":text}}),
    ));
}
