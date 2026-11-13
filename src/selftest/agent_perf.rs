//! Streaming into long timelines, native text reuse and catching up background tabs, without a live agent.

use objc2::rc::Retained;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use objc2::MainThreadOnly;
use objc2_app_kit::{NSAttributedStringAttributeFixing, NSTextView};
use objc2_foundation::{NSAttributedString, NSRange, NSURL};
use serde_json::{Value, json};

use super::{check, finish, helpers, snapshot};
use crate::agent::AgentTab;

const NAME: &str = "agent-perf";

pub fn run() {
    helpers::open_project(NAME, "stream-project", &[], |controller| {
        let mut tabs = Vec::new();
        for entries in [1000, 10_000, 1000, 1000] {
            let tab = crate::agent::open_replay(&controller).expect("replay tab");
            tab.feed(&history(entries));
            begin(&tab);
            tabs.push(tab);
        }
        let background = &tabs[..3];
        let built: Vec<_> = background.iter().map(|tab| tab.timeline().built()).collect();
        for _ in 0..50 {
            for tab in background {
                append(tab, "background **text**\n\n");
            }
        }
        check(
            "background streaming builds no cards",
            background
                .iter()
                .zip(built)
                .all(|(tab, before)| tab.timeline().built() == before),
            "three hidden tabs, 150 updates",
        );
        let bench = Rc::new(RefCell::new(Benchmark {
            tabs,
            case: 0,
            view: None,
            times: Vec::new(),
            before: 0,
            slowest: (Duration::ZERO, Duration::ZERO, Duration::ZERO, 0),
        }));
        helpers::after(TURN, move || prepare_case(bench));
    });
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

fn comparison(tab: &AgentTab, view: &NSTextView) -> Result<(), String> {
    let thread = tab.view_snapshot();
    let Some(entry) = thread.entries(&ide_thread::Lane::Main).last() else {
        return Err("no entry".into());
    };
    let ide_thread::EntryKind::AgentMessage(message) = &entry.kind else {
        return Err("no answer".into());
    };
    let full = crate::agent::answer_text(view.mtm(), &message.markdown);
    let expected = objc2_app_kit::NSTextStorage::new();
    expected.setAttributedString(&full);
    // The live storage substitutes fonts for emoji and other missing glyphs as it fixes attributes.
    expected.fixAttributesInRange(NSRange::new(0, expected.length()));
    // SAFETY: CardText owns its storage; this only retains it for reading attributes.
    let Some(storage) = (unsafe { view.textStorage() }) else {
        return Err("no storage".into());
    };
    if full.string().to_string() != view.string().to_string() {
        return Err("text differs".into());
    }
    if let Some(i) = (0..full.length()).find(|i| attributes(&expected, *i) != attributes(&storage, *i)) {
        return Err(format!(
            "UTF-16 {i}: expected {:?}, got {:?}",
            attributes(&expected, i),
            attributes(&storage, i)
        ));
    }
    Ok(())
}

fn attributes(text: &NSAttributedString, index: usize) -> (Option<String>, Option<String>) {
    use crate::util::constant;
    use objc2_app_kit::{NSFont, NSFontAttributeName, NSLinkAttributeName};
    let font_key = constant!(NSFontAttributeName);
    let link_key = constant!(NSLinkAttributeName);
    // SAFETY: The caller only visits valid UTF-16 positions; a null range pointer asks for no range.
    let font = unsafe { text.attribute_atIndex_effectiveRange(font_key, index, std::ptr::null_mut()) };
    // SAFETY: As above.
    let link = unsafe { text.attribute_atIndex_effectiveRange(link_key, index, std::ptr::null_mut()) };
    (
        font.and_then(|font| font.downcast::<NSFont>().ok())
            .map(|font| format!("{}:{}", font.fontName(), font.pointSize())),
        link.and_then(|link| link.downcast::<NSURL>().ok())
            .and_then(|url| url.absoluteString())
            .map(|url| url.to_string()),
    )
}

fn markdown(tab: &AgentTab) {
    let chunks = [
        "## A heading\n\n",
        "An **open",
        " emphasis** and [reference][target].\n\n",
        "```rust\nfn main() {",
        " println!(\"😀\"); }\n```\n\n",
        "| a | b |\n|---|---|\n",
        "| x | y |\n\n",
        "A note[^a].\n\n",
        "[target]: https://example.com\n\n",
        "[^a]: Defined later.\n\n",
        "العربية und Deutsch.\n\n",
    ];
    let mut errors = Vec::new();
    for (n, chunk) in chunks.into_iter().enumerate() {
        append(tab, chunk);
        match tab.timeline().streaming_text() {
            Some(view) => {
                if let Err(why) = comparison(tab, &view) {
                    errors.push(format!("chunk {n}: {why}"));
                }
            }
            None => errors.push(format!("chunk {n}: no view")),
        }
    }
    check(
        "incremental Markdown matches a full render",
        errors.is_empty(),
        if errors.is_empty() {
            "open emphasis, code, tables, late references, footnotes, Unicode and RTL".to_string()
        } else {
            errors.join("; ")
        },
    );
}

struct Benchmark {
    tabs: Vec<Rc<AgentTab>>,
    case: usize,
    view: Option<Retained<NSTextView>>,
    times: Vec<Duration>,
    before: usize,
    slowest: (Duration, Duration, Duration, usize),
}

/// The running benchmark, carried from step to step.
type Bench = Rc<RefCell<Benchmark>>;

/// A turn of the event loop between the steps of the benchmark, so that each update is timed on its own and drawing
/// in between is not counted; there is no state to wait for.
const TURN: Duration = Duration::from_millis(50);
/// The pause between two timed updates, about two frames, as a stream delivers them.
const SAMPLE_GAP: Duration = Duration::from_millis(30);

const CHUNK: &str = "A completed paragraph with **bold** text and `code`.\n\n";

fn current(bench: &Bench) -> (Rc<AgentTab>, usize, bool) {
    let bench = bench.borrow();
    (
        bench.tabs[bench.case / 2].clone(),
        if bench.case.is_multiple_of(2) { 10_000 } else { 100_000 },
        bench.case.is_multiple_of(2),
    )
}

fn prepare_case(bench: Bench) {
    let (tab, bytes, first) = current(&bench);
    tab.select();
    tab.timeline().scroll_to_end();
    if first {
        markdown(&tab);
    }
    let current = tab
        .view_snapshot()
        .entries(&ide_thread::Lane::Main)
        .last()
        .and_then(|entry| {
            if let ide_thread::EntryKind::AgentMessage(message) = &entry.kind {
                Some(message.markdown.len())
            } else {
                None
            }
        })
        .unwrap_or(0);
    append(&tab, &CHUNK.repeat(bytes.saturating_sub(current) / CHUNK.len()));
    tab.timeline().settle();
    helpers::after(TURN, move || begin_samples(bench));
}

fn begin_samples(bench: Bench) {
    let (tab, _, _) = current(&bench);
    tab.timeline().settle();
    let Some(view) = tab.timeline().streaming_text() else {
        check("streaming answer has a text view", false, "");
        return finish(NAME);
    };
    view.setSelectedRange(NSRange::new(0, 3));
    {
        let mut bench = bench.borrow_mut();
        bench.view = Some(view);
        bench.before = tab.timeline().built();
    }
    helpers::after(SAMPLE_GAP, move || sample(bench));
}

fn sample(bench: Bench) {
    let (tab, _, _) = current(&bench);
    let began = Instant::now();
    let built = tab.timeline().built();
    append(&tab, CHUNK);
    let appended = began.elapsed();
    tab.timeline().settle();
    let elapsed = began.elapsed();
    let done = {
        let mut bench = bench.borrow_mut();
        if elapsed > bench.slowest.0 {
            bench.slowest = (elapsed, appended, elapsed - appended, tab.timeline().built() - built);
        }
        bench.times.push(elapsed);
        bench.times.len() == 160
    };
    if done {
        finish_samples(bench);
    } else {
        helpers::after(SAMPLE_GAP, move || sample(bench));
    }
}

fn finish_samples(bench: Bench) {
    let (tab, bytes, _) = current(&bench);
    let (view, mut times, before, slowest) = {
        let mut bench = bench.borrow_mut();
        let data = (
            bench.view.take().expect("answer"),
            std::mem::take(&mut bench.times),
            bench.before,
            bench.slowest,
        );
        bench.slowest = (Duration::ZERO, Duration::ZERO, Duration::ZERO, 0);
        data
    };
    let reused = tab
        .timeline()
        .streaming_text()
        .is_some_and(|current| std::ptr::eq(&*current, &*view));
    let built = tab.timeline().built() - before;
    check(
        "streaming keeps its native view and text selection",
        reused && view.selectedRange() == NSRange::new(0, 3),
        format!("{built} new cards"),
    );
    check(
        "streaming builds only visible cards",
        built < 32,
        format!("{built} cards during 160 updates"),
    );
    times.sort();
    let p95 = times[times.len() * 95 / 100];
    check(
        "streaming avoids long main-thread stalls",
        slowest.0 < Duration::from_millis(100),
        format!("slowest update {:.2} ms", slowest.0.as_secs_f64() * 1000.0),
    );
    check(
        "streaming updates stay within a frame at p95",
        p95 < Duration::from_millis(16),
        format!(
            "{} rows, ~{} KB answer: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms (update {:.2}, layout {:.2}, {} cards)",
            tab.timeline().count(),
            bytes / 1000,
            times[80].as_secs_f64() * 1000.0,
            p95.as_secs_f64() * 1000.0,
            slowest.0.as_secs_f64() * 1000.0,
            slowest.1.as_secs_f64() * 1000.0,
            slowest.2.as_secs_f64() * 1000.0,
            slowest.3,
        ),
    );
    let result = comparison(&tab, &view);
    let height_error = tab.timeline().height_discrepancy();
    check(
        "visible row heights match the delegate",
        height_error < 2.0,
        format!("{height_error:.2} points difference"),
    );
    check(
        "streamed text remains complete",
        result.is_ok(),
        result
            .err()
            .unwrap_or_else(|| "full-render comparison after 160 updates".into()),
    );
    let thread = tab.view_snapshot();
    if let Some(ide_thread::Entry {
        kind: ide_thread::EntryKind::AgentMessage(message),
        ..
    }) = thread.entries(&ide_thread::Lane::Main).last().map(|entry| &**entry)
    {
        let expected = crate::agent::answer_height(view.mtm(), &message.markdown, view.frame().size.width);
        let height = view.intrinsicContentSize().height;
        check(
            "streaming keeps the full render's height",
            (height - expected).abs() < 2.0,
            format!("{height} / {expected}, width {}", view.frame().size.width),
        );
    }
    let done = {
        let mut bench = bench.borrow_mut();
        bench.case += 1;
        bench.case == 4
    };
    helpers::after(TURN, move || {
        if done {
            finish_benchmark(&bench);
        } else {
            prepare_case(bench);
        }
    });
}

fn finish_benchmark(bench: &Bench) {
    let tab = bench.borrow().tabs[2].clone();
    tab.select();
    tab.timeline().scroll_to_end();
    tab.timeline().settle();
    let text = tab.timeline().streaming_text();
    check(
        "a selected background tab catches up",
        text.is_some_and(|text| text.string().to_string().contains("background text")),
        "latest model is rendered on selection",
    );
    if let Some(window) = tab.root().window() {
        snapshot(&window, NAME);
    }
    finish(NAME);
}
