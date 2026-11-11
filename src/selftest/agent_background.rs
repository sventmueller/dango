//! The `agent-background` scenario: an AI tab that streams while another tab is in front shows the end of its answer
//! when it comes back, twice in a row, and keeps following the answer after.

use std::path::PathBuf;
use std::time::Duration;

use objc2::MainThreadMarker;
use serde_json::{Value, json};

use super::agent::tab_of;
use super::{check, finish, helpers, snapshot};
use crate::agent::AgentTab;
use crate::util::main_after;

const NAME: &str = "agent-background";

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = ide_core::paths::home().join("background-project");
    std::fs::create_dir_all(&root).expect("scratch project");
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    main_after(Duration::from_millis(400), move || {
        let Some(controller) = helpers::window_of(&root) else {
            return finish(NAME);
        };
        let Some(tab) = crate::agent::open_replay(&controller) else {
            check("an AI tab opens", false, "");
            return finish(NAME);
        };
        tab.feed(&history(40));
        begin(&tab);
        let key = tab.key;
        main_after(Duration::from_millis(300), move || to_background(root, key, 0));
    });
}

/// Puts another tab in front, streams into the hidden AI tab and brings it back.
fn to_background(root: PathBuf, key: u64, round: u32) {
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
    main_after(Duration::from_millis(500), move || returned(root, key, round, height));
}

fn returned(root: PathBuf, key: u64, round: u32, before: f64) {
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
    main_after(Duration::from_millis(500), move || {
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
        if let Some(window) = tab.root().window() {
            snapshot(&window, NAME);
        }
        finish(NAME);
    });
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
