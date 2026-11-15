//! The `snappy-recolor-markdown` scenario: a code fence opened at the top of 10 000 lines of Markdown turns every
//! line below into code and deleting it turns them back, and the recolouring holds the main thread no longer than a
//! keystroke inside a paragraph; scrolled to the end meanwhile, the lines on screen show the code's style. The preview
//! that opens with the file is closed first, as it lays the whole text out again after every keystroke.

use std::path::PathBuf;
use std::time::Duration;

use ide_ui_common::util::stall;
use objc2::rc::Retained;
use objc2_foundation::NSRange;

use super::snappy::measure;
use super::snappy_recolor::{delete, no_longer};
use super::{check, finish, helpers};
use crate::editor::{self, EditorTextView};

const SCENARIO: &str = "snappy-recolor-markdown";
const PROJECT: &str = "snappy-recolor-markdown-project";
const FILE: &str = "big.md";
const SECTIONS: usize = 2000;
const LINES_PER_SECTION: usize = 5;
/// The line of a section with bold text.
const BOLD_LINE: usize = 2;
/// The column the bold text starts in.
const BOLD_COLUMN: usize = 12;
const FENCE: &str = "```\n";
const SETTLE: Duration = Duration::from_millis(1500);
const KEYSTROKE: Duration = Duration::from_millis(500);
const COLOURED: Duration = Duration::from_secs(20);

/// Ten thousand lines of headings, paragraphs with bold, italic, struck and code text, and list items.
fn markdown() -> String {
    (0..SECTIONS)
        .map(|n| {
            format!(
                "## Section {n}\n\nText with **bold {n}** and *italic* and ~~struck~~ words.\n- an item with `code {n}`\n\n"
            )
        })
        .collect()
}

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn big() -> Option<Retained<EditorTextView>> {
    let controller = helpers::window_of(&root())?;
    editor::editor_for(&controller, &root().join(FILE))
}

/// The UTF-16 offset of `column` in `line`.
fn at(view: &EditorTextView, line: usize, column: usize) -> Option<usize> {
    view.line_start(line).map(|start| start + column)
}

/// Whether the bold text of every section on screen is bold and its heading coloured apart from its text, or, with
/// `fenced` and the sections a line further down, neither: all of it is code.
fn screen_styled(view: &EditorTextView, fenced: bool) -> bool {
    let shift = usize::from(fenced);
    let lines = editor::visible_line_range(view);
    let mut sections = lines
        .filter(|line| line >= &shift && (line - shift) % LINES_PER_SECTION == 0)
        .filter(|line| line + BOLD_LINE < lines_end(view))
        .peekable();
    sections.peek().is_some()
        && sections.all(|heading| {
            let bold = at(view, heading + BOLD_LINE, BOLD_COLUMN).and_then(|i| editor::styled_at(view, i));
            let text = at(view, heading + BOLD_LINE, 0).and_then(|i| editor::styled_at(view, i));
            let title = at(view, heading, 3).and_then(|i| editor::styled_at(view, i));
            match (bold, text, title) {
                (Some((_, bold)), Some((text, _)), Some((title, _))) => {
                    if fenced {
                        !bold && text == title
                    } else {
                        bold && text != title
                    }
                }
                _ => false,
            }
        })
}

/// A line on screen and where it is laid out, to tell whether painting moved the lines on screen.
fn anchor(view: &EditorTextView) -> Option<(usize, f64)> {
    let line = editor::visible_line_range(view).start + 10;
    let start = view.line_start(line)?;
    Some((line, ide_ui_lsp::rect_of(view, start..start + 1)?.origin.y))
}

fn lines_end(view: &EditorTextView) -> usize {
    view.document().text().line_count()
}

fn settled(view: &EditorTextView) -> bool {
    view.document().is_highlighted() && editor::colours_painted(view)
}

fn insert(view: &EditorTextView, at: usize, text: &str) {
    view.setSelectedRange(NSRange::new(at, 0));
    editor::type_text(view, text);
    helpers::post_event();
}

pub fn run() {
    let text = markdown();
    helpers::open_project(SCENARIO, PROJECT, &[(FILE, &text)], |_| {
        helpers::wait_for_editor(&root(), &root().join(FILE), |view| {
            let Some(view) = view else {
                check("the Markdown file of 10 000 lines opens", false, "");
                return finish(SCENARIO);
            };
            check(
                "the preview that opened with the Markdown file closes",
                crate::preview::close_beside(&view, false),
                "",
            );
            helpers::wait_within(
                COLOURED,
                move || settled(&view),
                |_| helpers::after(SETTLE, plain_keystroke),
            );
        });
    });
}

/// As many characters as the fence, typed inside a paragraph's word, which change the style of no other line.
fn plain_keystroke() {
    let Some(view) = big() else { return finish(SCENARIO) };
    let Some(word) = at(&view, BOLD_LINE, 2) else {
        return finish(SCENARIO);
    };
    measure();
    insert(&view, word, "xyzw");
    helpers::after(KEYSTROKE, move || {
        let plain = stall::take_worst();
        if let Some(view) = big() {
            delete(&view, NSRange::new(word, 4));
        }
        helpers::after(SETTLE, move || open_fence(plain));
    });
}

fn open_fence(plain: Duration) {
    let Some(view) = big() else { return finish(SCENARIO) };
    check(
        "the sections on screen show bold text and coloured headings before the fence",
        screen_styled(&view, false),
        format!("{:?}", editor::visible_line_range(&view)),
    );
    measure();
    insert(&view, 0, FENCE);
    helpers::wait_within(
        COLOURED,
        || big().is_some_and(|v| settled(&v) && screen_styled(&v, true)),
        move |done| {
            no_longer(
                "opening a code fence at the top of 10 000 lines of Markdown restyles them in slices as short as a \
                 keystroke's",
                done,
                plain,
            );
            scroll_to_end(plain);
        },
    );
}

fn scroll_to_end(plain: Duration) {
    let Some(view) = big() else { return finish(SCENARIO) };
    measure();
    let near_end = view.line_start(SECTIONS * LINES_PER_SECTION - 40).unwrap_or(0);
    view.scrollRangeToVisible(NSRange::new(near_end, 0));
    view.displayIfNeeded();
    let drawn = anchor(&view);
    helpers::wait_within(
        COLOURED,
        || big().is_some_and(|v| settled(&v) && screen_styled(&v, true)),
        move |shown| {
            check(
                "scrolled to the end, the Markdown on screen is styled as code",
                shown,
                big()
                    .map(|v| format!("{:?}", editor::visible_line_range(&v)))
                    .unwrap_or_default(),
            );
            let styled = big().and_then(|v| anchor(&v));
            check(
                "the lines on screen stay where they were first drawn while their fonts change",
                drawn.is_some() && drawn == styled,
                format!("first drawn {drawn:?}, styled {styled:?}"),
            );
            super::snappy::measured(
                "styling the Markdown scrolled to never holds up input",
                "the scroll to the end",
            );
            close_fence(plain);
        },
    );
}

fn close_fence(plain: Duration) {
    let Some(view) = big() else { return finish(SCENARIO) };
    measure();
    delete(&view, NSRange::new(0, FENCE.len()));
    helpers::wait_within(
        COLOURED,
        || big().is_some_and(|v| settled(&v) && screen_styled(&v, false)),
        move |done| {
            no_longer(
                "deleting the fence again restyles the Markdown in slices as short as a keystroke's",
                done,
                plain,
            );
            finish(SCENARIO);
        },
    );
}
