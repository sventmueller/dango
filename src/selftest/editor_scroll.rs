//! The `editor-scroll` scenario, without a language server: a long file with folds, extra carets and selections all
//! through it, scrolled by jumps, by the mouse wheel and by a trackpad's fine steps, down and back up. After every step a
//! real capture of the window must show each fold placeholder, inset after one, caret and selection the editor puts
//! on screen at its place, and nothing of the kind where it puts none.

mod picture;

use std::path::PathBuf;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSEvent, NSScreen};
use objc2_core_graphics::{CGEvent, CGEventField, CGScrollEventUnit};
use objc2_foundation::{NSPoint, NSRange, NSRect, NSSize};

use self::picture::Picture;
use super::helpers::wait_until;
use super::{check, finish};
use crate::editor::insets::{self, Inset, Layer, Place};
use crate::editor::{self, EditorTextView, selftest as edit};
use crate::util::main_after;
use crate::window;

const SCENARIO: &str = "editor-scroll";
const FILE: &str = "long.txt";
const BLOCKS: usize = 600;
/// A block's lines: its header, three indented lines the fold hides, the closing brace and an empty line.
const LINES: usize = 6;

fn folder() -> PathBuf {
    ide_core::paths::home().join("scroll-project")
}

fn root() -> PathBuf {
    let folder = folder();
    folder.canonicalize().unwrap_or(folder)
}

fn view() -> Option<Retained<EditorTextView>> {
    let controller = window::for_project(&root())?;
    editor::editor_for(&controller, &root().join(FILE))
}

fn text() -> String {
    (0..BLOCKS)
        .map(|b| format!("block {b:04} {{\n    alpha {b:04} = 1;\n    beta {b:04} = 2;\n    gamma {b:04} = 3;\n}}\n\n"))
        .collect()
}

fn folded(block: usize) -> bool {
    block.is_multiple_of(3)
}

fn with_caret(block: usize) -> bool {
    block % 3 == 1
}

fn with_selection(block: usize) -> bool {
    block % 3 == 2
}

fn with_inset(block: usize) -> bool {
    block.is_multiple_of(6)
}

/// Where the caret of a block's `beta` line stands: inside the indentation, where nothing else is drawn.
fn caret_at(view: &EditorTextView, block: usize) -> Option<usize> {
    Some(view.line_start(block * LINES + 2)? + 2)
}

/// The word `gamma` of a block.
fn gamma(view: &EditorTextView, block: usize) -> Option<std::ops::Range<usize>> {
    let start = view.line_start(block * LINES + 3)? + 4;
    Some(start..start + 5)
}

pub fn run() {
    let folder = folder();
    let written = std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(folder.join(FILE), text()));
    if let Err(error) = written {
        check("the file is made", false, error.to_string());
        return finish(SCENARIO);
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder);
    main_after(Duration::from_millis(500), || {
        if let Some(controller) = window::for_project(&root()) {
            controller.open_file(&root().join(FILE));
        }
        wait_until(
            || view().is_some_and(|v| v.line_start(BLOCKS * LINES - 1).is_some()),
            |ok| {
                check("the long file opens", ok, "");
                if ok {
                    prepare();
                } else {
                    finish(SCENARIO);
                }
            },
        );
    });
}

/// Puts an inset after the first line of every sixth block, then, once the lines on screen show them, folds every
/// third block, puts a caret in every third and a selection in the others, the view's own caret at the top.
fn prepare() {
    let Some(view) = view() else { return finish(SCENARIO) };
    let hints: Vec<Inset> = (0..BLOCKS)
        .filter(|b| with_inset(*b))
        .filter_map(|b| Some(Inset::new(view.line_start(b * LINES)?, "hint", Place::LineEnd)))
        .collect();
    insets::set(&view, Layer::Inlay, hints);
    main_after(Duration::from_millis(300), fold_and_place);
}

fn fold_and_place() {
    let Some(view) = view() else { return finish(SCENARIO) };
    for block in (0..BLOCKS).filter(|b| folded(*b)) {
        if let Some(at) = view.line_start(block * LINES) {
            view.setSelectedRange(NSRange::new(at, 0));
            edit::fold_at_cursor(&view);
        }
    }
    let mut carets: Vec<std::ops::Range<usize>> = (0..BLOCKS)
        .filter_map(|b| {
            if with_caret(b) {
                caret_at(&view, b).map(|at| at..at)
            } else if with_selection(b) {
                gamma(&view, b)
            } else {
                None
            }
        })
        .collect();
    carets.push(0..0);
    edit::set_carets(&view, carets);
    check(
        "the blocks fold and the carets stand",
        edit::folded(&view) == BLOCKS / 3 && edit::carets(&view).len() == BLOCKS / 3 * 2 + 1,
        format!("{} folded, {} carets", edit::folded(&view), edit::carets(&view).len()),
    );
    main_after(Duration::from_millis(500), || step(0, Tally::default()));
}

#[derive(Clone, Copy)]
enum Move {
    /// To the folded line of the block this far into the file, in percent.
    Jump(usize),
    /// A mouse wheel's notch, in lines; negative goes down.
    Wheel(i32),
    /// Folds or unfolds every block on screen that the scenario folded.
    Toggle,
    /// A trackpad's event, in points. Without the phases of a swipe: AppKit follows a swipe that began only with the
    /// window server's own events.
    Swipe(i32),
}

/// The steps: a name, the move, how many events of it, and which way the text must have gone (1 down, -1 up).
const STEPS: &[(&str, Move, usize, i8)] = &[
    ("jump into the middle", Move::Jump(40), 1, 0),
    ("wheel down", Move::Wheel(-3), 4, 1),
    ("wheel down", Move::Wheel(-3), 4, 1),
    ("wheel down", Move::Wheel(-3), 4, 1),
    ("wheel down", Move::Wheel(-3), 4, 1),
    ("wheel up", Move::Wheel(3), 4, -1),
    ("wheel up", Move::Wheel(3), 4, -1),
    ("wheel up", Move::Wheel(3), 4, -1),
    ("wheel up", Move::Wheel(3), 4, -1),
    ("jump further down", Move::Jump(75), 1, 0),
    ("trackpad down", Move::Swipe(-40), 8, 1),
    ("trackpad down", Move::Swipe(-40), 8, 1),
    ("trackpad down", Move::Swipe(-40), 8, 1),
    ("trackpad up", Move::Swipe(40), 8, -1),
    ("trackpad up", Move::Swipe(40), 8, -1),
    ("trackpad up", Move::Swipe(40), 8, -1),
    ("jump near the top", Move::Jump(10), 1, 0),
    ("jump to the top, folded while on screen", Move::Jump(0), 1, 0),
    ("unfold on screen", Move::Toggle, 1, 0),
    ("fold again on screen", Move::Toggle, 1, 0),
    ("wheel down after folding", Move::Wheel(-3), 2, 1),
];

/// What the steps found, by kind of check.
#[derive(Default)]
struct Tally {
    seen: [usize; KINDS.len()],
    wrong: [Vec<String>; KINDS.len()],
}

const MOVED: usize = 0;
const CAPTURED: usize = 1;
const PLACEHOLDER: usize = 2;
const NO_PLACEHOLDER: usize = 3;
const INSET: usize = 4;
const CARET: usize = 5;
const NO_CARET: usize = 6;
const SELECTION: usize = 7;

const KINDS: [&str; 8] = [
    "every scroll step moves the text the way it was asked",
    "every step is a real capture of the window",
    "every fold placeholder on screen shows right after its folded line, at every step",
    "no placeholder shows after a line that is not folded",
    "an inset at a folded line's end shows after the placeholder, not on it",
    "every extra caret on screen shows on its line, at every step",
    "no caret shows on a line without one",
    "every extra selection on screen is tinted, and the same word unselected is not",
];

impl Tally {
    fn note(&mut self, kind: usize, wrong: Option<String>) {
        self.seen[kind] += 1;
        if let Some(wrong) = wrong {
            self.wrong[kind].push(wrong);
        }
    }

    fn report(self) {
        for (kind, name) in KINDS.iter().enumerate() {
            let wrong = &self.wrong[kind];
            let mut detail = format!("{} looked at, {} wrong", self.seen[kind], wrong.len());
            for w in wrong.iter().take(12) {
                detail.push_str("; ");
                detail.push_str(w);
            }
            check(name, self.seen[kind] > 0 && wrong.is_empty(), detail);
        }
    }
}

fn step(index: usize, tally: Tally) {
    let Some(&(_, movement, count, _)) = STEPS.get(index) else {
        tally.report();
        return finish(SCENARIO);
    };
    let before = view().map_or(0.0, |v| v.visibleRect().origin.y);
    if matches!(movement, Move::Wheel(_) | Move::Swipe(_))
        && let Some(view) = view()
    {
        draw_ahead(&view);
    }
    play(movement, count, move || {
        main_after(Duration::from_millis(60), move || look(index, before, tally));
    });
}

/// Has AppKit draw a screen's height above and below what shows, as responsive scrolling does before it moves the
/// text; what it drew there shows later without being drawn again.
fn draw_ahead(view: &EditorTextView) {
    let seen = view.visibleRect();
    view.prepareContentInRect(rect(
        seen.origin.x,
        seen.origin.y - seen.size.height,
        seen.size.width,
        seen.size.height * 3.0,
    ));
    if let Some(window) = view.window() {
        window.displayIfNeeded();
    }
}

/// Sends `count` events of `movement`, one per frame, then calls `then`.
fn play(movement: Move, count: usize, then: impl FnOnce() + Send + 'static) {
    let Some(view) = view() else { return then() };
    match movement {
        Move::Jump(percent) => {
            let block = BLOCKS * percent / 100;
            if let Some(at) = view.line_start(block * LINES) {
                editor::scroll_to_byte(&view, at);
            }
        }
        Move::Toggle => {
            let span = editor::utf16_in(&view, view.visibleRect());
            for block in (0..BLOCKS).filter(|b| folded(*b)) {
                let line = block * LINES;
                if view.line_start(line).is_some_and(|at| span.contains(&at)) {
                    edit::toggle_fold(&view, line);
                }
            }
        }
        Move::Wheel(lines) => wheel(&view, CGScrollEventUnit::Line, lines),
        Move::Swipe(points) => wheel(&view, CGScrollEventUnit::Pixel, points),
    }
    if count > 1 {
        main_after(Duration::from_millis(16), move || play(movement, count - 1, then));
    } else {
        then();
    }
}

/// Gives `view` a scroll event over its middle, as the window does with one from the window server. Posted to the app's
/// queue instead, the event reaches no view.
fn wheel(view: &EditorTextView, unit: CGScrollEventUnit, delta: i32) {
    let Some(window) = view.window() else { return };
    let mtm = view.mtm();
    let seen = view.visibleRect();
    let middle = NSPoint::new(
        seen.origin.x + seen.size.width / 2.0,
        seen.origin.y + seen.size.height / 2.0,
    );
    let on_screen = window.convertPointToScreen(view.convertPoint_toView(middle, None));
    let top = NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |screen| screen.frame().size.height);
    let Some(event) = CGEvent::new_scroll_wheel_event2(None, unit, 1, delta, 0, 0) else {
        return;
    };
    CGEvent::set_location(
        Some(&event),
        objc2_core_foundation::CGPoint::new(on_screen.x, top - on_screen.y),
    );
    let number = i64::try_from(window.windowNumber()).unwrap_or(0);
    CGEvent::set_integer_value_field(Some(&event), CGEventField::MouseEventWindowUnderMousePointer, number);
    CGEvent::set_integer_value_field(
        Some(&event),
        CGEventField::MouseEventWindowUnderMousePointerThatCanHandleThisEvent,
        number,
    );
    if unit == CGScrollEventUnit::Pixel {
        CGEvent::set_integer_value_field(Some(&event), CGEventField::ScrollWheelEventIsContinuous, 1);
    }
    if let Some(event) = NSEvent::eventWithCGEvent(&event) {
        view.scrollWheel(&event);
    }
}

/// Captures the window as it shows after step `index`, compares it with what the editor put on screen, and goes on.
fn look(index: usize, before: f64, mut tally: Tally) {
    let Some(view) = view() else {
        tally.report();
        return finish(SCENARIO);
    };
    let (name, _, _, direction) = STEPS[index];
    let label = format!("{index} {name}");
    let after = view.visibleRect().origin.y;
    let went = match after - before {
        d if d > 0.5 => 1,
        d if d < -0.5 => -1,
        _ => 0,
    };
    tally.note(
        MOVED,
        (direction != 0 && went != direction).then(|| format!("{label}: from y {before:.0} to {after:.0}")),
    );
    let picture_name = format!("editor-scroll-{index:02}");
    let path =
        super::picture_path(&picture_name).unwrap_or_else(|| std::env::temp_dir().join(format!("{picture_name}.png")));
    let number = view.window().map_or(0, |w| w.windowNumber()).to_string();
    let picture = if super::capture(&["-o", "-l", &number], &path) {
        Picture::load(&path, &view)
    } else {
        Err("screencapture failed".to_string())
    };
    match picture {
        Ok(picture) => {
            tally.note(CAPTURED, None);
            compare(&view, &picture, &label, &mut tally);
        }
        Err(error) => tally.note(CAPTURED, Some(format!("{label}: {error}"))),
    }
    main_after(Duration::from_millis(50), move || step(index + 1, tally));
}

/// The part of `view` on screen, less a margin at the top and bottom where a line shows only in part.
fn inner(view: &EditorTextView) -> NSRect {
    let seen = view.visibleRect();
    NSRect::new(
        NSPoint::new(seen.origin.x, seen.origin.y + 4.0),
        NSSize::new(seen.size.width, seen.size.height - 8.0),
    )
}

fn inside(outer: NSRect, rect: NSRect) -> bool {
    rect.origin.y >= outer.origin.y && rect.origin.y + rect.size.height <= outer.origin.y + outer.size.height
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

/// The first lines of the blocks folded now.
fn folded_now(view: &EditorTextView) -> std::collections::BTreeSet<usize> {
    edit::gutter(view)
        .into_iter()
        .filter_map(|(line, folded)| folded.then_some(line))
        .collect()
}

/// The lines shown in full on screen, without those a fold hides.
fn lines_on_screen(view: &EditorTextView) -> Vec<usize> {
    let folds = folded_now(view);
    let span = editor::utf16_in(view, view.visibleRect());
    let (first, last) = {
        let document = view.document();
        let text = document.text();
        let line = |at: usize| text.utf16_to_byte(at).map_or(0, |b| text.line_of(b));
        (line(span.start), line(span.end))
    };
    let area = inner(view);
    (first..=last)
        .filter(|line| !(folds.contains(&(line - line % LINES)) && (1..=3).contains(&(line % LINES))))
        .filter(|line| {
            view.line_start(*line)
                .and_then(|at| crate::lsp::rect_of(view, at..at))
                .is_some_and(|r| inside(area, r))
        })
        .collect()
}

fn compare(view: &EditorTextView, picture: &Picture, label: &str, tally: &mut Tally) {
    let lines = lines_on_screen(view);
    compare_folds(view, picture, label, &lines, tally);
    compare_carets(view, picture, label, &lines, tally);
}

/// Each folded line on screen ends in its placeholder, after which its inset follows; no other line ends in one.
fn compare_folds(view: &EditorTextView, picture: &Picture, label: &str, lines: &[usize], tally: &mut Tally) {
    let placeholders = edit::placeholders(view);
    let folds = folded_now(view);
    for &line in lines {
        let Some(end) = view.line_start(line + 1).map(|at| at - 1) else {
            continue;
        };
        let Some(caret) = crate::lsp::rect_of(view, end..end) else {
            continue;
        };
        let block = line / LINES;
        if !folds.contains(&line) && line % LINES == 0 && with_inset(block) {
            let wrong = inset_at(view, picture, caret.origin.x + END_GAP, caret);
            tally.note(INSET, wrong.map(|w| format!("{label}: line {line}, not folded: {w}")));
            continue;
        }
        if !folds.contains(&line) {
            let after = picture.ink(
                view,
                rect(
                    caret.origin.x + 5.0,
                    caret.origin.y + 2.0,
                    25.0,
                    caret.size.height - 4.0,
                ),
                8,
            );
            let wrong = after.share() > 0.03;
            tally.note(
                NO_PLACEHOLDER,
                wrong.then(|| format!("{label}: line {line} has {after:?} after its end")),
            );
            continue;
        }
        let Some((rect_drawn, _)) = placeholders
            .iter()
            .find(|(r, _)| (r.origin.y - 1.0 - caret.origin.y).abs() < 2.0)
        else {
            tally.note(
                PLACEHOLDER,
                Some(format!(
                    "{label}: line {line} is folded, but the editor has no placeholder for it"
                )),
            );
            continue;
        };
        let p = *rect_drawn;
        let body = rect(
            p.origin.x + 2.0,
            p.origin.y + 2.0,
            p.size.width - 4.0,
            p.size.height - 4.0,
        );
        let filled = picture.ink(view, body, 8);
        let brace = picture.ink(view, rect(p.origin.x - 13.0, p.origin.y, 8.0, p.size.height), 60);
        tally.note(
            PLACEHOLDER,
            (filled.share() < 0.6 || brace.marked < 3).then(|| {
                format!("{label}: line {line}: placeholder {filled:?}, line's last character {brace:?}, at {p:?}")
            }),
        );
        if with_inset(block) {
            let wrong = inset_at(view, picture, p.origin.x + p.size.width + END_GAP, caret);
            tally.note(INSET, wrong.map(|w| format!("{label}: line {line}, folded: {w}")));
        }
    }
}

/// Between the end of a line and its first inset, as the editor leaves it.
const END_GAP: f64 = 14.0;

/// Whether the inset `hint` of the line `line` shows from `x` on, with nothing in the room before it.
fn inset_at(view: &EditorTextView, picture: &Picture, x: f64, line: NSRect) -> Option<String> {
    let band = rect(
        x - 12.0,
        line.origin.y + line.size.height * 0.25,
        9.0,
        line.size.height * 0.5,
    );
    let gap = picture.ink(view, band, 8);
    let hint = picture.ink(view, rect(x + 4.0, line.origin.y, 20.0, line.size.height), 40);
    (gap.share() > 0.05 || hint.marked < 5).then(|| format!("before the inset {gap:?}, the inset {hint:?} at x {x:.0}"))
}

/// Each `beta` line with a caret shows it in its indentation, no other line shows one there, and each `gamma` with a
/// selection is tinted while the others are not.
fn compare_carets(view: &EditorTextView, picture: &Picture, label: &str, lines: &[usize], tally: &mut Tally) {
    let extra = editor::selftest::carets(view);
    for &line in lines {
        let block = line / LINES;
        let kind = line % LINES;
        if (1..=3).contains(&kind) {
            let Some(at) = view.line_start(line).map(|s| s + 2) else {
                continue;
            };
            let Some(r) = crate::lsp::rect_of(view, at..at) else {
                continue;
            };
            let strip = picture.ink(
                view,
                rect(r.origin.x - 1.0, r.origin.y + 2.0, 3.5, r.size.height - 4.0),
                40,
            );
            if kind == 2 && with_caret(block) {
                let known = extra.contains(&(at..at));
                tally.note(
                    CARET,
                    (!known || strip.share() < 0.25)
                        .then(|| format!("{label}: line {line}: caret known {known}, drawn {strip:?} at {r:?}")),
                );
            } else {
                tally.note(
                    NO_CARET,
                    (strip.marked > 2).then(|| format!("{label}: line {line} shows {strip:?} without a caret")),
                );
            }
        }
        if kind == 3 {
            let Some(word) = gamma(view, block) else { continue };
            let Some(r) = crate::lsp::rect_of(view, word.clone()) else {
                continue;
            };
            let tinted = picture.tint(
                view,
                rect(r.origin.x, r.origin.y + 1.0, r.size.width, r.size.height - 2.0),
            );
            let selected = with_selection(block);
            let wrong = if selected {
                tinted.share() < 0.5
            } else {
                tinted.share() > 0.05
            };
            tally.note(
                SELECTION,
                wrong.then(|| format!("{label}: line {line}: selected {selected}, tinted {tinted:?}")),
            );
        }
    }
}
