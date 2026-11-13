//! The `editor-insets` scenario, the gate of E81: text that is not in the file, shown at a line's end and in gaps of
//! a line, checked point by point against the list in docs/phase-11.md before any consumer builds on it. Every point
//! names itself in its check; a point that cannot hold reports how the inset fell back instead.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSPasteboard, NSTextInputClient,
};
use objc2_foundation::{NSPoint, NSRange, NSRect};
use splat_i18n::t;

use super::helpers::{self, editor, wait_until};
use super::{check, finish, snapshot};
use crate::editor::insets::{self, Inset, Layer, Place};
use crate::editor::{self, EditorTextView};
use crate::util::ns;
use crate::window::{self, WindowController};

const SCENARIO: &str = "editor-insets";
const FILE: &str = "gate.txt";

const PROJECT: &str = "insets-project";

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn controller() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn gate_view() -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*controller()?, &root().join(FILE))
}

fn gate_text() -> String {
    let long = vec!["word"; 60].join(" ");
    format!(
        "let value = 1;\n\nstart of line\ne\u{301}x \u{1F44D}\u{1F3FD}y\n{long}\nfold:\n    inner value\n    more\nend\n"
    )
}

/// The UTF-16 offset of the first `needle` in the gate's text, plus `into` code units.
fn at(needle: &str, into: usize) -> usize {
    let text = gate_text();
    let byte = text.find(needle).unwrap_or(0);
    text[..byte].encode_utf16().count() + into
}

/// The insets of the gate, by name.
struct Places {
    value: usize,
    empty_line: usize,
    line_start: usize,
    combining: usize,
    emoji_inside: usize,
    emoji_end: usize,
    wrapped: usize,
    header: usize,
    folded: usize,
}

fn places() -> Places {
    Places {
        value: at("let value", 9),
        empty_line: at("\n\n", 1),
        line_start: at("start of line", 0),
        combining: at("e\u{301}x", 2),
        emoji_inside: at("\u{1F44D}", 1),
        emoji_end: at("\u{1F44D}", 4),
        wrapped: at("word", 5 * 30 - 1),
        header: at("fold:", 0),
        folded: at("inner value", 5),
    }
}

fn caret(view: &EditorTextView, at: usize) -> Option<NSRect> {
    crate::lsp::rect_of(view, at..at)
}

fn shown_at(view: &EditorTextView, layer: Layer, index: usize) -> Option<insets::Shown> {
    insets::shown(view)
        .into_iter()
        .find(|s| s.layer == layer && s.index == index)
}

fn redraw(view: &EditorTextView) {
    if let Some(window) = view.window() {
        window.displayIfNeeded();
    }
    view.display();
}

fn picture(name: &str) {
    if let Some(window) = controller().and_then(|c| c.window()) {
        snapshot(&window, name);
    }
}

/// Presses and lets go of the mouse at `point` of `view`, through the view a hit test finds there, as AppKit does.
pub fn click_at(view: &EditorTextView, point: NSPoint) {
    let (Some(window), Some(mtm)) = (view.window(), MainThreadMarker::new()) else {
        return;
    };
    let location = view.convertPoint_toView(point, None);
    let event = |kind| {
        NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind,
            location,
            NSEventModifierFlags::empty(),
            0.0,
            window.windowNumber(),
            None,
            0,
            1,
            1.0,
        )
    };
    let (Some(down), Some(up)) = (event(NSEventType::LeftMouseDown), event(NSEventType::LeftMouseUp)) else {
        return;
    };
    let Some(target) = window.contentView().and_then(|content| content.hitTest(location)) else {
        return;
    };
    NSApplication::sharedApplication(mtm).postEvent_atStart(&up, true);
    target.mouseDown(&down);
}

pub fn run() {
    let text = gate_text();
    helpers::open_project(SCENARIO, PROJECT, &[(FILE, text.as_str())], |controller| {
        editor::new_untitled(&controller, 13.0);
        wait_until(
            || controller_editor(t("Ohne Titel")).is_some(),
            |ok| {
                check("an empty document opens", ok, "");
                empty_document();
            },
        );
    });
}

fn controller_editor(title: &str) -> Option<Retained<EditorTextView>> {
    editor(&*controller()?, title)
}

/// An empty document has no character before anything: an inline inset goes to the end of its only line.
fn empty_document() {
    let Some(view) = controller_editor(t("Ohne Titel")) else {
        return finish(SCENARIO);
    };
    let undo_before = view.undoManager().is_some_and(|u| u.canUndo());
    insets::set(&view, Layer::Inlay, vec![Inset::new(0, ": i32", Place::Inline)]);
    wait_until(
        || controller_editor(t("Ohne Titel")).is_some_and(|view| insets::shown(&view).len() == 1),
        move |_| empty_shown(undo_before),
    );
}

fn empty_shown(undo_before: bool) {
    let Some(view) = controller_editor(t("Ohne Titel")) else {
        return finish(SCENARIO);
    };
    redraw(&view);
    let shown = insets::shown(&view);
    let line = caret(&view, 0);
    check(
        "empty document: the inset shows after the end of the empty line, without a kern",
        shown.len() == 1
            && !shown[0].in_gap
            && insets::kerns(&view).is_empty()
            && line.is_some_and(|c| {
                shown[0].rect.origin.x > c.origin.x && (shown[0].rect.origin.y - c.origin.y).abs() < 1.0
            }),
        format!("{shown:?}, caret {line:?}, kerns {:?}", insets::kerns(&view)),
    );
    check(
        "empty document: the text and its undo stay as they were",
        view.document().text().to_string().is_empty()
            && view.undoManager().is_some_and(|u| u.canUndo()) == undo_before
            && !view.document().is_dirty(),
        format!("{:?}", view.document().text().to_string()),
    );
    picture("insets-empty");
    insets::clear(&view, Layer::Inlay);
    if let Some(controller) = controller() {
        controller.open_file(&root().join(FILE));
    }
    wait_until(|| gate_view().is_some(), |_| gate());
}

/// The caret positions of the gate's text before any inset, to compare with after.
struct Before {
    carets: Vec<(usize, NSRect)>,
}

impl Before {
    fn x(&self, at: usize) -> Option<f64> {
        self.carets.iter().find(|(a, _)| *a == at).map(|(_, r)| r.origin.x)
    }
}

fn gate() {
    let Some(view) = gate_view() else {
        check("the gate's file opens", false, "");
        return finish(SCENARIO);
    };
    redraw(&view);
    let p = places();
    let watched = [
        p.value - 1,
        p.value,
        p.value + 1,
        p.value + 2,
        p.combining - 2,
        p.combining,
        p.combining + 1,
        p.emoji_end - 4,
        p.emoji_end,
        p.wrapped - 1,
        p.wrapped,
    ];
    let before = Before {
        carets: watched.iter().filter_map(|&at| Some((at, caret(&view, at)?))).collect(),
    };
    let undo_before = view.undoManager().is_some_and(|u| u.canUndo());
    let clicked = Rc::new(Cell::new(0_u32));
    let counted = Rc::clone(&clicked);
    let mut clickable = Inset::new(0, "Ada, vor 3 Tagen · Clickable", Place::LineEnd);
    clickable.action = Some(Rc::new(move |_: &EditorTextView| counted.set(counted.get() + 1)));
    clickable.tooltip = Some("a clickable inset".into());
    insets::set(
        &view,
        Layer::Blame,
        vec![clickable, Inset::new(p.header, "at the header", Place::LineEnd)],
    );
    let mut hinted = Inset::new(p.value, ": i32", Place::Inline);
    hinted.tooltip = Some("the type of value".into());
    insets::set(
        &view,
        Layer::Inlay,
        vec![
            hinted,
            Inset::new(p.empty_line, ": empty", Place::Inline),
            Inset::new(p.line_start, "start:", Place::Inline),
            Inset::new(p.combining, "é", Place::Inline),
            Inset::new(p.emoji_inside, "👍", Place::Inline),
            Inset::new(p.wrapped, ": wrapped", Place::Inline),
            Inset::new(p.folded, ": folded", Place::Inline),
        ],
    );
    redraw(&view);
    picture("insets-gate");
    line_start_and_graphemes(&view, &before, undo_before);
    wait_until(
        || gate_view().is_some_and(|view| shown_at(&view, Layer::Blame, 0).is_some()),
        move |_| clicks(&clicked),
    );
}

fn line_start_and_graphemes(view: &EditorTextView, before: &Before, undo_before: bool) {
    let p = places();
    let kerns = insets::kerns(view);
    let kerned = |range: std::ops::Range<usize>| kerns.iter().find(|(r, _)| *r == range).map(|(_, w)| *w);
    let fallback = |index: usize, line_start: usize| {
        shown_at(view, Layer::Inlay, index).is_some_and(|s| !s.in_gap)
            && !kerns
                .iter()
                .any(|(r, _)| r.start + 1 == line_start || r.start == line_start)
    };
    check(
        "line start: an inset where no character precedes it on its line goes to the line's end, without a kern",
        fallback(1, p.empty_line) && fallback(2, p.line_start),
        format!(
            "{:?} / {:?}",
            shown_at(view, Layer::Inlay, 1),
            shown_at(view, Layer::Inlay, 2)
        ),
    );
    let moved = |at: usize| {
        caret(view, at)
            .map(|r| r.origin.x)
            .zip(before.x(at))
            .map(|(n, b)| n - b)
    };
    let width = kerned(p.value - 1..p.value);
    let drawn = shown_at(view, Layer::Inlay, 0);
    let (kept, after) = (moved(p.value - 1), moved(p.value + 1));
    check(
        "a gap: the character before keeps its place, the one after moves by the inset's width, the inset fills the gap",
        kept.is_some_and(|k| k.abs() < 0.5)
            && after.zip(width).is_some_and(|(a, w)| (a - w).abs() < 0.75)
            && drawn
                .as_ref()
                .zip(before.x(p.value))
                .zip(width)
                .is_some_and(|((s, end), w)| {
                    s.in_gap && (s.rect.origin.x - end).abs() < 0.75 && (s.rect.size.width - w).abs() < 0.75
                }),
        format!(
            "before kept {kept:?}, after moved {after:?} for kern {width:?}, drawn {drawn:?}, glyph end {:?}",
            before.x(p.value)
        ),
    );
    let line = |at: usize| {
        view.document()
            .text()
            .utf16_to_byte(at)
            .map_or(0, |b| view.document().text().line_of(b))
    };
    let grapheme_line = line(p.combining);
    let unmoved = [p.combining, p.combining + 1, p.emoji_end]
        .iter()
        .all(|&at| moved(at).is_some_and(|m| m.abs() < 0.5));
    check(
        "emoji and combining marks: TextKit 2 ignores a kern on a grapheme's last code point, so the inset falls back to the line's end",
        fallback(3, p.combining)
            && fallback(4, p.emoji_end)
            && !kerns.iter().any(|(r, _)| line(r.start) == grapheme_line)
            && unmoved,
        format!(
            "{:?} / {:?}, kerns {kerns:?}",
            shown_at(view, Layer::Inlay, 3),
            shown_at(view, Layer::Inlay, 4)
        ),
    );
    caret_at_gap(view, before);
    check(
        "unchanged undo: showing insets adds no undo step and leaves the document clean",
        view.undoManager().is_some_and(|u| u.canUndo()) == undo_before && !view.document().is_dirty(),
        format!("can undo {}", view.undoManager().is_some_and(|u| u.canUndo())),
    );
}

/// The caret right after the gap, where TextKit 2 draws it, against the gap's edges.
fn caret_at_gap(view: &EditorTextView, before: &Before) {
    let p = places();
    view.setSelectedRange(NSRange::new(p.value, 0));
    if let Some(window) = view.window() {
        window.makeKeyAndOrderFront(None);
        window.makeFirstResponder(Some(view));
    }
    redraw(view);
    let reported = caret(view, p.value).map(|r| r.origin.x);
    let indicator = indicator_x(view);
    let gap = shown_at(view, Layer::Inlay, 0).map(|s| (s.rect.origin.x, s.rect.origin.x + s.rect.size.width));
    let x = indicator.or(reported);
    let at_edge = x
        .zip(gap)
        .is_some_and(|(x, (left, right))| (x - left).abs() < 1.0 || (x - right).abs() < 1.0);
    check(
        "caret right of the gap: it stands at an edge of the inset, not inside it",
        at_edge,
        format!(
            "indicator {indicator:?}, reported {reported:?}, gap {gap:?}, glyph end before {:?}",
            before.x(p.value)
        ),
    );
    picture("insets-caret");
}

/// Where the text view's insertion indicator stands, in its coordinates, wherever AppKit put it.
fn indicator_x(view: &EditorTextView) -> Option<f64> {
    fn find(within: &objc2_app_kit::NSView, view: &EditorTextView) -> Option<f64> {
        within.subviews().iter().find_map(|sub| {
            let name = sub.class().name().to_str().unwrap_or_default().to_string();
            if name.contains("InsertionIndicator") && !sub.isHidden() {
                let frame = sub.convertRect_toView(sub.bounds(), Some(view));
                return Some(frame.origin.x + frame.size.width / 2.0);
            }
            find(&sub, view)
        })
    }
    find(&*view.window()?.contentView()?, view)
}

/// Clicks left and right of a gap land beside it in the text; a click on an inset that does something runs it and
/// leaves the text alone.
fn clicks(clicked: &Cell<u32>) {
    let Some(view) = gate_view() else {
        return finish(SCENARIO);
    };
    redraw(&view);
    let p = places();
    let gap = shown_at(&view, Layer::Inlay, 0);
    let mut seen = Vec::new();
    let mut click_lands = |name: &str, point: Option<NSPoint>, expected: usize| {
        view.setSelectedRange(NSRange::new(0, 0));
        if let Some(point) = point {
            click_at(&view, point);
        }
        let got = view.selectedRange().location;
        seen.push(format!("{name}: {got} (wanted {expected})"));
        got == expected
    };
    let middle_y = |r: NSRect| r.origin.y + r.size.height / 2.0;
    let anchor = crate::lsp::rect_of(&view, p.value - 1..p.value);
    let next = crate::lsp::rect_of(&view, p.value..p.value + 1);
    let left_of_char = anchor.map(|r| NSPoint::new(r.origin.x + 1.5, middle_y(r)));
    let in_gap_left = gap
        .as_ref()
        .map(|s| NSPoint::new(s.rect.origin.x + 2.0, middle_y(s.rect)));
    let in_gap_right = gap
        .as_ref()
        .map(|s| NSPoint::new(s.rect.origin.x + s.rect.size.width - 2.0, middle_y(s.rect)));
    let right_of_next = next.map(|r| NSPoint::new(r.origin.x + r.size.width - 1.0, middle_y(r)));
    let ok = [
        click_lands("left half of the character before the gap", left_of_char, p.value - 1),
        click_lands("left in the gap", in_gap_left, p.value),
        click_lands("right in the gap", in_gap_right, p.value),
        click_lands("right half of the character after the gap", right_of_next, p.value + 1),
    ];
    check(
        "caret and click positions left and right of the gap",
        ok.iter().all(|&o| o),
        seen.join("; "),
    );
    let clickable = shown_at(&view, Layer::Blame, 0);
    view.setSelectedRange(NSRange::new(3, 0));
    let before = clicked.get();
    if let Some(s) = &clickable {
        click_at(
            &view,
            NSPoint::new(s.rect.origin.x + s.rect.size.width / 2.0, middle_y(s.rect)),
        );
    }
    let tip = clickable
        .as_ref()
        .and_then(|s| insets::tooltip(&view, NSPoint::new(s.rect.origin.x + 2.0, s.rect.origin.y + 2.0)));
    check(
        "a clickable inset: the click runs it and does not reach the text; it has its tooltip",
        clicked.get() == before + 1
            && view.selectedRange().location == 3
            && tip.as_deref() == Some("a clickable inset"),
        format!(
            "{clickable:?}, clicks {}, selection {}, tooltip {tip:?}",
            clicked.get(),
            view.selectedRange().location
        ),
    );
    let gap_tip = gap
        .as_ref()
        .and_then(|s| insets::tooltip(&view, NSPoint::new(s.rect.origin.x + 2.0, s.rect.origin.y + 2.0)));
    check(
        "an inset that does nothing lets a click through to the text and still has its tooltip",
        gap.as_ref()
            .is_some_and(|s| !insets::hits(&view, NSPoint::new(s.rect.origin.x + 2.0, s.rect.origin.y + 2.0)))
            && gap_tip.as_deref() == Some("the type of value"),
        format!("{gap_tip:?}"),
    );
    rotor(&view);
    marked_text(&view);
}

/// Whether every view over the editor's text is left out of the accessibility tree.
fn overlay_hidden(view: &EditorTextView) -> bool {
    view.subviews().iter().all(|sub| !sub.isAccessibilityElement())
}

fn rotor(view: &EditorTextView) {
    let (label, first) = insets::rotor_step(view, None, true);
    let (_, second) = insets::rotor_step(view, first.as_ref().map(|i| i.at), true);
    let (_, back) = insets::rotor_step(view, second.as_ref().map(|i| i.at), false);
    check(
        "VoiceOver rotor \"Hinweise\": steps through the insets with their text and line, both ways",
        label.as_deref() == Some(t("Hinweise"))
            && first
                .as_ref()
                .is_some_and(|i| i.at == 0 && i.label.contains("Clickable") && i.label.contains('1'))
            && second
                .as_ref()
                .is_some_and(|i| i.at == places().value && i.label.contains(": i32"))
            && back == first,
        format!("{label:?}: {first:?} → {second:?} → {back:?}"),
    );
    check(
        "the overlay stays out of the accessibility tree; the rotor carries the insets",
        overlay_hidden(view),
        "",
    );
}

/// Japanese input with an open marked range right at a gap: the gap stays, the marked text takes no kern, and the
/// committed text neither.
fn marked_text(view: &EditorTextView) {
    let p = places();
    view.setSelectedRange(NSRange::new(p.value, 0));
    let replace = NSRange::new(objc2_foundation::NSNotFound.cast_unsigned(), 0);
    // SAFETY: The marked text is an NSString, one of the two types this method accepts.
    unsafe { view.setMarkedText_selectedRange_replacementRange(&ns("にほん"), NSRange::new(3, 0), replace) };
    redraw(view);
    let kerns = insets::kerns(view);
    let line = view
        .document()
        .text()
        .line_content(0)
        .map(|r| view.document().text().to_string()[r].to_string());
    check(
        "Japanese input: with marked text open at the gap, the gap stays and the marked text takes no kern",
        view.hasMarkedText()
            && kerns.iter().any(|(r, _)| *r == (p.value - 1..p.value))
            && !kerns.iter().any(|(r, _)| r.start >= p.value && r.start < p.value + 3)
            && line.as_deref() == Some("let valueにほん = 1;"),
        format!("{line:?}, kerns {kerns:?}"),
    );
    picture("insets-marked");
    // SAFETY: The inserted object is an NSString, one of the two types this method accepts.
    unsafe { view.insertText_replacementRange(&ns("日本"), replace) };
    wait_until(
        || gate_view().is_some_and(|view| !view.hasMarkedText() && view.document().text().to_string().contains("日本")),
        |_| committed(),
    );
}

fn committed() {
    let Some(view) = gate_view() else {
        return finish(SCENARIO);
    };
    redraw(&view);
    let p = places();
    let kerns = insets::kerns(&view);
    let line = view
        .document()
        .text()
        .line_content(0)
        .map(|r| view.document().text().to_string()[r].to_string());
    check(
        "Japanese input: the committed text has no kern and the inset stays before it",
        !view.hasMarkedText()
            && line.as_deref() == Some("let value日本 = 1;")
            && kerns.iter().any(|(r, _)| *r == (p.value - 1..p.value))
            && !kerns.iter().any(|(r, _)| r.start >= p.value && r.start < p.value + 2)
            && insets::of(&view, Layer::Inlay).first().is_some_and(|i| i.at == p.value),
        format!("{line:?}, kerns {kerns:?}"),
    );
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    wait_until(
        || gate_view().is_some_and(|view| view.document().text().to_string() == gate_text()),
        |_| undone(),
    );
}

fn undone() {
    let Some(view) = gate_view() else {
        return finish(SCENARIO);
    };
    let p = places();
    let kerns = insets::kerns(&view);
    check(
        "unchanged undo: one undo takes back the typing and nothing else, the gap still kerned",
        view.document().text().to_string() == gate_text()
            && !view.document().is_dirty()
            && kerns.iter().any(|(r, _)| *r == (p.value - 1..p.value)),
        format!("dirty {}, kerns {kerns:?}", view.document().is_dirty()),
    );
    copy_and_save(&view);
    wait_until(
        || gate_view().is_some_and(|view| !view.document().is_dirty()),
        |_| wrap(),
    );
}

fn copy_and_save(view: &EditorTextView) {
    let line = gate_text().lines().next().unwrap_or_default().to_string();
    view.setSelectedRange(NSRange::new(0, line.encode_utf16().count()));
    // SAFETY: `copy:` takes the sender, here none.
    let _: () = unsafe { objc2::msg_send![view, copy: Option::<&objc2::runtime::AnyObject>::None] };
    let pasteboard = NSPasteboard::generalPasteboard();
    // SAFETY: The string type is AppKit's constant.
    let copied = unsafe { pasteboard.stringForType(objc2_app_kit::NSPasteboardTypeString) }.map(|s| s.to_string());
    let rich = pasteboard
        .types()
        .is_some_and(|types| types.iter().any(|t| t.to_string().contains("rtf")));
    check(
        "copying takes the text only, not the gap or the inset",
        copied.as_deref() == Some(line.as_str()) && !rich,
        format!("{copied:?}, rich {rich}"),
    );
    let end = view.document().text().len_utf16();
    super::helpers::type_at(view, NSRange::new(end, 0), "x");
    editor::save_view(view);
    let on_disk = std::fs::read_to_string(root().join(FILE)).unwrap_or_default();
    check(
        "the saved file holds the text only, without an inset",
        on_disk == format!("{}x", gate_text()) && !on_disk.contains(": i32") && !view.document().is_dirty(),
        format!("{} bytes", on_disk.len()),
    );
}

/// With wrapped lines, the gap and its inset stay on the visual line of the character before it.
fn wrap() {
    let Some(view) = gate_view() else {
        return finish(SCENARIO);
    };
    let p = places();
    if !editor::wraps(&view) {
        editor::toggle_wrap(&view);
    }
    if let Some(window) = view.window() {
        let mut frame = window.frame();
        frame.size.width = 700.0;
        window.setFrame_display(frame, true);
    }
    let wrapped = move || {
        gate_view().is_some_and(|view| {
            redraw(&view);
            editor::wraps(&view) && shown_at(&view, Layer::Inlay, 5).is_some_and(|s| s.in_gap)
        })
    };
    wait_until(wrapped, move |_| {
        let Some(view) = gate_view() else {
            return finish(SCENARIO);
        };
        redraw(&view);
        let drawn = shown_at(&view, Layer::Inlay, 5);
        let anchor = crate::lsp::rect_of(&view, p.wrapped - 1..p.wrapped);
        let line_top = caret(&view, at("word", 0)).map(|r| r.origin.y);
        let visible = view.visibleRect();
        check(
            "line wrap: the wrapped line's gap stays beside its character, on its visual line and inside the view",
            drawn.as_ref().zip(anchor).is_some_and(|(s, a)| {
                s.in_gap
                    && (s.rect.origin.y - a.origin.y).abs() < 1.0
                    && (s.rect.origin.x - (a.origin.x + a.size.width)).abs() < 0.75
                    && s.rect.origin.x + s.rect.size.width <= visible.origin.x + visible.size.width + 0.5
            }) && line_top.zip(anchor).is_some_and(|(top, a)| a.origin.y > top),
            format!("{drawn:?}, anchor {anchor:?}, line top {line_top:?}"),
        );
        picture("insets-wrapped");
        editor::toggle_wrap(&view);
        wait_until(|| gate_view().is_some_and(|view| !editor::wraps(&view)), |_| fold());
    });
}

/// A fold hides the insets of its lines, and the one at the end of its first line moves past the placeholder.
fn fold() {
    let Some(view) = gate_view() else {
        return finish(SCENARIO);
    };
    let p = places();
    view.setSelectedRange(NSRange::new(p.header, 0));
    editor::selftest::fold_at_cursor(&view);
    let folded = || gate_view().is_some_and(|view| editor::selftest::folded(&view) == 1);
    wait_until(folded, move |_| {
        let Some(view) = gate_view() else {
            return finish(SCENARIO);
        };
        redraw(&view);
        let placeholders = editor::selftest::placeholders(&view);
        let header = shown_at(&view, Layer::Blame, 1);
        let folded = shown_at(&view, Layer::Inlay, 6);
        check(
            "folding: insets of hidden lines are not drawn, and one at the first line's end follows the placeholder",
            editor::selftest::folded(&view) == 1
                && folded.is_none()
                && header.as_ref().zip(placeholders.first()).is_some_and(|(h, (rect, _))| {
                    h.rect.origin.x >= rect.origin.x + rect.size.width && (h.rect.origin.y - rect.origin.y).abs() < 3.0
                }),
            format!("{header:?}, placeholders {placeholders:?}, folded inset {folded:?}"),
        );
        picture("insets-folded");
        editor::selftest::unfold_all(&view);
        check(
            "the editor stays on TextKit 2 throughout",
            view.textLayoutManager().is_some(),
            "",
        );
        appearances(&view);
        budget();
    });
}

/// Light, dark and increased contrast: the text of an inset stands out from its box by E66's ratio, 3:1 and 4.5:1
/// with increased contrast; a picture of each. AppKit gives a window the appearances of increased contrast only through
/// the system setting, not by name, so those two measure the colours the app picks for the setting.
fn appearances(view: &EditorTextView) {
    use objc2_app_kit::{
        NSAppearanceNameAccessibilityHighContrastAqua, NSAppearanceNameAccessibilityHighContrastDarkAqua,
        NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    };
    let Some(controller) = controller() else { return };
    let mut seen = Vec::new();
    let mut ok = true;
    let names = [
        (crate::util::constant!(NSAppearanceNameAqua), "light", 3.0, false),
        (crate::util::constant!(NSAppearanceNameDarkAqua), "dark", 3.0, false),
        (
            crate::util::constant!(NSAppearanceNameAccessibilityHighContrastAqua),
            "light-contrast",
            4.5,
            true,
        ),
        (
            crate::util::constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua),
            "dark-contrast",
            4.5,
            true,
        ),
    ];
    for (name, label, needed, increased) in names {
        super::agent::appearance(&controller, name);
        insets::force_increased_contrast(increased);
        redraw(view);
        let ratio = insets::contrast(view).unwrap_or(0.0);
        ok &= ratio >= needed;
        seen.push(format!("{label} {ratio:.2}:1"));
        picture(&format!("insets-{label}"));
    }
    insets::force_increased_contrast(false);
    if let Some(window) = controller.window() {
        objc2_app_kit::NSAppearanceCustomization::setAppearance(&*window, None);
    }
    check(
        "the insets' text stands out from its box by 3:1, 4.5:1 with increased contrast (E66)",
        ok,
        seen.join(", "),
    );
}

/// The median CPU time a keystroke in 10 000 lines may take in the self-tests: E25's 16 ms is the frame, this the
/// budget, well below it so that a step back shows before the user feels it.
pub const KEYSTROKE: Duration = Duration::from_millis(5);

/// What drawing insets, diagnostics or proposals may add to a keystroke's median.
pub const ADDED: Duration = Duration::from_millis(2);

/// The CPU time of the main thread, so other work on this Mac does not count against a budget.
pub fn thread_time() -> Duration {
    let mut now = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `now` is a valid timespec to fill.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &raw mut now) };
    Duration::new(
        u64::try_from(now.tv_sec).unwrap_or(0),
        u32::try_from(now.tv_nsec).unwrap_or(0),
    )
}

/// Keystrokes with and without insets in alternating blocks, so that the load of this Mac weighs on both alike: the
/// lowest median and the highest slowest of each, with first.
pub fn compare(view: &EditorTextView, on: impl Fn(), off: impl Fn()) -> ((Duration, Duration), (Duration, Duration)) {
    let (mut with, mut without) = (Vec::new(), Vec::new());
    for _ in 0..3 {
        on();
        redraw(view);
        with.push(keystrokes(view, 40));
        off();
        redraw(view);
        without.push(keystrokes(view, 40));
    }
    let summary = |runs: &[(Duration, Duration)]| {
        (
            runs.iter().map(|r| r.0).min().unwrap_or_default(),
            runs.iter().map(|r| r.1).max().unwrap_or_default(),
        )
    };
    (summary(&with), summary(&without))
}

/// Types `count` keystrokes at the caret of `view`, each with what follows it on screen, and gives the median and the
/// slowest in CPU time.
pub fn keystrokes(view: &EditorTextView, count: usize) -> (Duration, Duration) {
    let mut times = Vec::new();
    for n in 0..count {
        let start = thread_time();
        editor::type_text(view, if n % 7 == 6 { "\n" } else { "x" });
        insets::settle(view);
        if let Some(window) = view.window() {
            window.displayIfNeeded();
        }
        times.push(thread_time().saturating_sub(start));
    }
    times.sort();
    (times[times.len() / 2], times[times.len() - 1])
}

/// E25 with insets on: after a jump, a keystroke in a file of 10 000 lines with an inset on every line stays within
/// `KEYSTROKE`, edit, layout and drawing together.
fn budget() {
    let big: String = (0..10_000)
        .map(|n| format!("    let value_{n} = compute({n}, other_{n});\n"))
        .collect();
    let path = root().join("big.rs.txt");
    if let Err(error) = std::fs::write(&path, &big) {
        check("the big file is written", false, error.to_string());
        return finish(SCENARIO);
    }
    let Some(window) = controller() else {
        return finish(SCENARIO);
    };
    window.open_file(&path);
    let opened = path.clone();
    let laid_out = move || {
        controller()
            .and_then(|c| editor::editor_for(&c, &opened))
            .is_some_and(|view| view.line_start(9_999).is_some())
    };
    wait_until(laid_out, move |_| {
        let Some(view) = controller().and_then(|c| editor::editor_for(&c, &path)) else {
            check("the big file opens", false, "");
            return finish(SCENARIO);
        };
        let starts: Vec<usize> = (0..10_000).filter_map(|line| view.line_start(line)).collect();
        let line = 5000;
        editor::scroll_to_byte(&view, big.lines().take(line).map(|l| l.len() + 1).sum());
        redraw(&view);
        let at = starts.get(line).copied().unwrap_or(0) + 30;
        view.setSelectedRange(NSRange::new(at, 0));
        let hints = || {
            (0..10_000)
                .filter_map(|line| view.line_start(line))
                .map(|start| Inset::new(start + 13, ": i32", Place::Inline))
                .collect::<Vec<_>>()
        };
        let ((median, slowest), (plain_median, plain_slowest)) = compare(
            &view,
            || insets::set(&view, Layer::Inlay, hints()),
            || insets::clear(&view, Layer::Inlay),
        );
        check(
            "E25 with insets: after a jump to line 5000 of 10 000 a keystroke stays under 5 ms, and an inset on each line \
             adds under 2 ms",
            plain_median < KEYSTROKE && median.saturating_sub(plain_median) < ADDED,
            format!(
                "with them median {median:?}, slowest {slowest:?}; without median {plain_median:?}, slowest {plain_slowest:?}"
            ),
        );
        let _ = editor::save_all();
        finish(SCENARIO);
    });
}
